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
const env = { ...process.env, APPDATA: root }; // only these children, never the user's settings
function check(program, args, input, expected, opts = {}) {
  const result = spawnSync(program, args, { input, encoding:'utf8', env, windowsHide:true, timeout:20000, ...opts });
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
  // Generated hooks through the REAL host semantics, with a receiver path
  // containing spaces AND non-ASCII characters. The config is produced by the
  // receiver copy itself (task-config), so the strings under test are exactly
  // what the product generates for that path.
  const spaced = fs.mkdtempSync(path.join(root, '目录 bridge 拷 贝-'));
  const receiverCopy = path.join(spaced, 'QuotaBar 拷 贝.exe');
  fs.copyFileSync(exe, receiverCopy);
  const gen = tool => {
    const r = spawnSync(receiverCopy,['task-config',tool],{env,windowsHide:true,encoding:'utf8',timeout:20000});
    assert.ifError(r.error); assert.equal(r.status,0,r.stderr);
    return JSON.parse(JSON.parse(r.stdout).content).hooks;
  };
  // Claude official semantics: command + args are spawned DIRECTLY, no shell.
  const ch = gen('claude').PermissionRequest[0].hooks[0];
  assert.ok(!/powershell/i.test(ch.command) && ch.command === receiverCopy,'Claude command must be the raw executable');
  assert.deepEqual(ch.args,['task-event','claude']);
  check(receiverCopy,ch.args,payload,'waiting_approval');
  // Codex: command string executed through the Windows shell (cmd semantics).
  const xh = gen('codex').PermissionRequest[0].hooks[0];
  const cmd = xh.commandWindows || xh.command;
  assert.ok(!/powershell/i.test(cmd) && !cmd.includes('-EncodedCommand'),'generated Codex hook must not use PowerShell');
  assert.ok(cmd.startsWith('"') && cmd.includes('task-event'),'Codex hook invokes the quoted native receiver');
  // Codex command_runner.rs (Windows): COMSPEC cmd.exe /d /s /c plus
  // raw_arg(r#""{command_line}""#) — one verbatim argument wrapped in an
  // outer quote pair. windowsVerbatimArguments keeps Node from re-quoting
  // (its default escaping backslash-quotes embedded quotes, breaking cmd).
  check('cmd.exe',['/d','/s','/c',`"${cmd}"`],payload,'waiting_approval',{windowsVerbatimArguments:true});
  check(exe,['task-event','codex'],'{invalid',null);
  check(exe,['task-event','codex'],'x'.repeat(1048577),null);
  console.log('PASS: direct receiver, Claude direct-spawn and Codex shell hooks (spaces+Unicode path), passive malformed/oversize input; isolated data only');
} finally { fs.rmSync(root,{recursive:true,force:true}); }
