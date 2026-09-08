// Synthetic lifecycle inputs through generated hooks/plugin and the real native receiver/store.
// Does not prove upstream CLI hook trust or OS toast visibility. No auth or global config changes.
const fs=require('node:fs'), path=require('node:path'), assert=require('node:assert/strict');
const {spawn,spawnSync}=require('node:child_process');
const exe=path.resolve(process.argv[2]||'src-tauri/target/debug/quotabar.exe');
const root=fs.mkdtempSync(path.join(__dirname,'../src-tauri/target/lifecycle-'));
const env={...process.env,APPDATA:path.join(root,'appdata')};
const wait=ms=>new Promise(r=>setTimeout(r,ms));
const snapshots=[],alerts=[]; let latest=[],buffer='',failure;
const watcher=spawn(exe,['task-watch','120'],{env,windowsHide:true,stdio:['ignore','pipe','pipe']});
watcher.on('error',e=>{failure=e;});
watcher.stderr.resume();
watcher.stdout.on('data',chunk=>{
  buffer+=chunk; let i;
  while((i=buffer.indexOf('\n'))>=0){
    const line=buffer.slice(0,i);buffer=buffer.slice(i+1);
    try {const v=JSON.parse(line); latest=v.tasks; snapshots.push(v.tasks);alerts.push(...v.notification_candidates);}
    catch(e){failure=e;}
  }
});
async function until(check,label){const end=Date.now()+20000;while(Date.now()<end){if(failure)throw failure;if(check())return;await wait(50);}throw Error('Timeout: '+label);}
const state=(id,want)=>latest.some(t=>t.session_id===id&&t.state===want);
const count=(id,s)=>alerts.filter(t=>t.session_id===id&&t.state===s).length;
function config(tool){const r=spawnSync(exe,['task-config',tool],{env,windowsHide:true,encoding:'utf8',timeout:20000});assert.ifError(r.error);assert.equal(r.status,0);return JSON.parse(r.stdout).content;}
const configs=Object.fromEntries(['codex','claude'].map(t=>[t,JSON.parse(config(t))]));
function hook(tool,id,event,extra={}){
  const h=configs[tool].hooks[event][0].hooks[0];
  const input=JSON.stringify({session_id:id,hook_event_name:event,cwd:'C:/private/demo',prompt:'SECRET_MUST_NOT_APPEAR',...extra});
  // Match real host semantics: Claude spawns command+args DIRECTLY (no
  // shell); Codex executes the command string through the Windows shell.
  // Codex command_runner.rs (Windows): COMSPEC cmd.exe /d /s /c plus
  // raw_arg(r#""{command_line}""#) — one verbatim argument wrapped in an
  // outer quote pair; windowsVerbatimArguments prevents Node re-quoting.
  const r=h.args
    ? spawnSync(h.command,h.args,{env,windowsHide:true,timeout:20000,encoding:'utf8',input})
    : spawnSync('cmd.exe',['/d','/s','/c',`"${h.commandWindows||h.command}"`],{env,windowsHide:true,timeout:20000,encoding:'utf8',input,windowsVerbatimArguments:true});
  if(h.args){assert.equal(h.command,exe);assert.deepEqual(h.args,['task-event',tool]);}
  else{const cmd=h.commandWindows||h.command;assert.ok(!/powershell/i.test(cmd)&&cmd.startsWith('"')&&cmd.includes('task-event'),'Codex hook must be the quoted native receiver command');}
  assert.ifError(r.error);assert.equal(r.status,0,r.stderr);assert.deepEqual(JSON.parse(r.stdout),{});
}
(async()=>{
  try {
    await until(()=>snapshots.length,'watcher ready');
    const source=config('opencode').replace('windowsHide: true,',`env: {...process.env, APPDATA: ${JSON.stringify(env.APPDATA)}}, windowsHide: true,`);
    // The module exists only in memory; inherited environment values are not serialized.
    const {QuotaBarTasks}=await import('data:text/javascript;base64,'+Buffer.from(source).toString('base64'));
    const plugin=await QuotaBarTasks({directory:'C:/private/demo'});
    const event=(type,props={})=>plugin.event({event:{type,properties:{sessionID:'open-C',...props}}});
    hook('codex','codex-A','UserPromptSubmit');hook('claude','claude-B','UserPromptSubmit');await event('session.status',{status:{type:'busy'}});
    await until(()=>state('codex-A','working')&&state('claude-B','working')&&state('open-C','working'),'three independent working sessions');
    hook('codex','codex-A','PermissionRequest');hook('claude','claude-B','PreToolUse',{tool_name:'AskUserQuestion'});
    await event('permission.asked',{id:'one'});await event('permission.asked',{id:'two'});
    await until(()=>count('codex-A','waiting_approval')===1&&count('claude-B','waiting_input')===1&&count('open-C','waiting_approval')===1,'waiting notifications');
    await event('permission.replied',{requestID:'one'});await event('session.status',{status:{type:'busy'}});
    await wait(2400);assert.ok(state('open-C','waiting_approval'));assert.equal(count('open-C','waiting_approval'),1);
    hook('codex','codex-A','PostToolUse');hook('claude','claude-B','PostToolUse');await event('permission.replied',{requestID:'two'});
    await until(()=>latest.every(t=>t.state==='working')&&latest.length===3,'resume independently');
    hook('codex','codex-A','Stop');hook('codex','codex-A','SessionEnd');
    hook('claude','claude-B','StopFailure');hook('claude','claude-B','Stop');await event('session.idle');
    await until(()=>count('codex-A','ended')===1&&count('claude-B','failed')===1&&count('open-C','ended')===1,'terminal notifications');
    assert.ok(state('claude-B','failed'));assert.equal(count('claude-B','ended'),0);
    hook('codex','codex-A','UserPromptSubmit');await until(()=>state('codex-A','working'),'new turn');hook('codex','codex-A','Stop');
    await until(()=>count('codex-A','ended')===2,'next turn notification');await wait(2200);
    assert.equal(alerts.length,7,'no duplicate notifications');
    assert.ok(!JSON.stringify(snapshots).includes('SECRET_MUST_NOT_APPEAR'));
    const report={passed:true,kind:'synthetic-native-lifecycle',executable:exe,sessionCount:latest.length,notifications:alerts.map(t=>({tool:t.tool,state:t.state})),fixture:root};
    fs.writeFileSync(path.join(root,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report));
  } catch(e){const report={passed:false,kind:'synthetic-native-lifecycle',error:e.message,fixture:root};fs.writeFileSync(path.join(root,'report.json'),JSON.stringify(report,null,2));console.error(JSON.stringify(report));process.exitCode=1;}
  finally {watcher.kill();}
})();
