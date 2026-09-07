// Opt-in live CLI test. Uses the user's existing auth, but only a temporary project plugin/settings.
// No global config edits; child agent may make one small model request.
const fs = require('node:fs');
const path = require('node:path');
const {spawn, spawnSync} = require('node:child_process');
const tool = process.argv[2];
if (!['claude','opencode'].includes(tool)) throw Error('Usage: node scripts/test-live-tasks.cjs claude|opencode');
const repo = path.resolve(__dirname,'..');
const root = fs.mkdtempSync(path.join(repo,'src-tauri/target/live-tasks-'));
const exe = path.join(repo,'src-tauri/target/debug/quotabar.exe');
const appdata = path.join(root,'appdata');
const result = spawnSync(exe,['task-config',tool],{encoding:'utf8',windowsHide:true});
if(result.status!==0) throw Error('Cannot generate configuration');
const config = JSON.parse(result.stdout);
let args, binary;
if(tool==='opencode') {
  const pluginDir = path.join(root,'.opencode/plugins'); fs.mkdirSync(pluginDir,{recursive:true});
  const trace = path.join(root,'lifecycle.jsonl');
  let source = config.content.replace('windowsHide: true,',`env: { ...process.env, APPDATA: ${JSON.stringify(appdata)} }, windowsHide: true,`);
  source = "import { appendFileSync } from 'node:fs';\n" + source.replace('event: async ({ event }) => {', `event: async ({ event }) => { if (/^(session|permission|question)\\./.test(event.type)) appendFileSync(${JSON.stringify(trace)}, JSON.stringify({type:event.type,keys:Object.keys(event.properties || {}),status:event.properties?.status?.type})+'\\n');`);
  fs.writeFileSync(path.join(pluginDir,'quotabar-tasks.js'),source);
  binary = path.join(process.env.APPDATA,'npm/node_modules/opencode-ai/bin/opencode.exe');
  args = ['run','--format','json','--title','QuotaBar local integration test','Reply with exactly OK. Do not use any tools, read files, or change files.'];
} else {
  const cfg = JSON.parse(config.content);
  for(const groups of Object.values(cfg.hooks)) for(const group of groups) for(const hook of group.hooks) {
    const prefix = hook.command.slice(0,hook.command.lastIndexOf(' ')+1);
    const source = Buffer.from(hook.command.slice(prefix.length),'base64').toString('utf16le');
    hook.command = prefix + Buffer.from(`$env:APPDATA = '${appdata.replaceAll("'","''")}'; ${source}`,'utf16le').toString('base64');
  }
  fs.writeFileSync(path.join(root,'settings.json'),JSON.stringify(cfg));
  binary = path.join(process.env.APPDATA,'npm/node_modules/@anthropic-ai/claude-code/bin/claude.exe');
  args = ['-p','--setting-sources','','--settings',path.join(root,'settings.json'),'--tools',''];
}
const watcher = spawn(exe,['task-watch','60'],{env:{...process.env,APPDATA:appdata},windowsHide:true,stdio:['ignore','pipe','ignore']});
const states = new Set(), notifications = new Set(); let buffer='';
watcher.stdout.on('data',chunk=>{
  buffer+=chunk;
  let index;
  while((index=buffer.indexOf('\n'))>=0) {
    const line=buffer.slice(0,index);buffer=buffer.slice(index+1);
    try {const v=JSON.parse(line);for(const t of v.tasks) states.add(t.state);for(const t of v.notification_candidates) notifications.add(t.state);}catch{}
  }
});
setTimeout(()=>{
  const child = spawn(binary,args,{cwd:root,windowsHide:true,stdio:['pipe','pipe','pipe']});
  child.stdin.on('error',()=>{});
  child.stdin.end(tool==='claude' ? 'Reply with exactly OK.' : '');
  let stderrPresent=false, diagnostic=''; child.stdout.resume(); child.stderr.on('data',chunk=>{stderrPresent=true;diagnostic=(diagnostic+chunk).slice(-2000);});
  let timedOut=false;
  const timer=setTimeout(()=>{timedOut=true;child.kill();},45000);
  child.on('error',error=>console.log(JSON.stringify({tool,launchError:error.code})));
  child.on('close',code=>{
    clearTimeout(timer);
    setTimeout(()=>{
      watcher.kill();
      const errorHint = code ? diagnostic.replace(/(?:sk-|Bearer\s+)[A-Za-z0-9_.-]+/g,'[REDACTED]').replace(/https?:\/\/\S+/g,'[URL]').replace(/-EncodedCommand\s+[A-Za-z0-9+/=]+/g,'-EncodedCommand [HOOK]').slice(-1000) : undefined;
      const report={tool,code,timedOut,stderrPresent,errorHint,states:[...states],notificationCandidates:[...notifications],fixture:root};
      report.passed = code===0 && !timedOut && states.has('ended') && notifications.has('ended');
      process.exitCode = report.passed ? 0 : 2;
      fs.writeFileSync(path.join(root,'report.json'),JSON.stringify(report,null,2));
      console.log(JSON.stringify(report));
    },3000);
  });
},400);
