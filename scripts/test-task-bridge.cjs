// Run with a built Windows executable, including release (GUI subsystem).
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const assert = require('node:assert/strict');
const exe = path.resolve(process.argv[2] || 'src-tauri/target/debug/quotabar.exe');
const root = fs.mkdtempSync(path.join(os.tmpdir(), 'quotabar-bridge-'));
assert.ok(path.resolve(root).startsWith(path.resolve(os.tmpdir()) + path.sep));
const dir = path.join(root, 'quotabar', 'task-events');
const env = { ...process.env, APPDATA: root }; // only this child, never the user's settings
function check(program, args, input, expected) {
  const result = spawnSync(program, args, { input, encoding:'utf8', env, windowsHide:true, timeout:20000 });
  assert.ifError(result.error); assert.equal(result.status,0,result.stderr);
  assert.deepEqual(JSON.parse(result.stdout.trim()),{});
  const files = fs.existsSync(dir) ? fs.readdirSync(dir).filter(f=>f.endsWith('.json')) : [];
  assert.equal(files.length,expected ? 1 : 0);
  if(expected) {
    const raw = fs.readFileSync(path.join(dir,files[0]),'utf8');
    assert.equal(JSON.parse(raw).state,expected); assert.ok(!raw.includes('SECRET'));
    fs.unlinkSync(path.join(dir,files[0]));
  }
}
try {
  const payload = JSON.stringify({session_id:'smoke',cwd:'C:/private/demo',hook_event_name:'PermissionRequest',prompt:'SECRET'});
  check(exe,['task-event','codex'],payload,'waiting_approval');
  const script = `$OutputEncoding = [Console]::InputEncoding = [Text.UTF8Encoding]::new($false); [Console]::In.ReadToEnd() | & '${exe.replaceAll("'", "''")}' task-event claude`;
  check('powershell.exe',['-NoLogo','-NoProfile','-NonInteractive','-EncodedCommand',Buffer.from(script,'utf16le').toString('base64')],payload,'waiting_approval');
  check(exe,['task-event','codex'],'{invalid',null);
  check(exe,['task-event','codex'],'x'.repeat(1048577),null);
  console.log('PASS: direct and generated PowerShell hooks, passive malformed/oversize input; isolated data only');
} finally { fs.rmSync(root,{recursive:true,force:true}); }
