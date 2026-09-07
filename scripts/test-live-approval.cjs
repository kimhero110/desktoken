// Opt-in real permission test; local password-protected OpenCode server, temporary project.
// Requests a harmless echo but REJECTS it after the waiting notification candidate appears.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
const {spawn,spawnSync}=require('node:child_process');
const {approvalPassed}=require('./task-acceptance.cjs');
const repo=path.resolve(__dirname,'..');
const root=fs.mkdtempSync(path.join(repo,'src-tauri/target/live-approval-'));
const exe=path.join(repo,'src-tauri/target/debug/quotabar.exe');
const appdata=path.join(root,'appdata');
const cfg=JSON.parse(spawnSync(exe,['task-config','opencode'],{encoding:'utf8',windowsHide:true}).stdout);
fs.mkdirSync(path.join(root,'.opencode/plugins'),{recursive:true});
fs.writeFileSync(path.join(root,'.opencode/plugins/quotabar-tasks.js'),cfg.content.replace('windowsHide: true,',`env:{...process.env,APPDATA:${JSON.stringify(appdata)}},windowsHide: true,`));
fs.writeFileSync(path.join(root,'opencode.json'),JSON.stringify({permission:{'*':'deny',bash:'ask'}}));
const password=crypto.randomBytes(24).toString('hex');
const binary=path.join(process.env.APPDATA,'npm/node_modules/opencode-ai/bin/opencode.exe');
const server=spawn(binary,['serve','--hostname','127.0.0.1','--port','0'],{cwd:root,windowsHide:true,env:{...process.env,OPENCODE_SERVER_PASSWORD:password,OPENCODE_SERVER_USERNAME:'quotabar-test'},stdio:['ignore','pipe','pipe']});
let url='',serverText='';server.stdout.on('data',v=>{serverText=(serverText+v).slice(-2000);url=serverText.match(/http:\/\/127\.0\.0\.1:\d+/)?.[0]||url;});server.stderr.resume();
const watcher=spawn(exe,['task-watch','120'],{env:{...process.env,APPDATA:appdata},windowsHide:true,stdio:['ignore','pipe','ignore']});
const states=[],notifications=[];let buffer='';
watcher.stdout.on('data',v=>{buffer+=v;let i;while((i=buffer.indexOf('\n'))>=0){const line=buffer.slice(0,i);buffer=buffer.slice(i+1);try{const x=JSON.parse(line);for(const t of x.tasks)if(states.at(-1)!==t.state)states.push(t.state);for(const t of x.notification_candidates)notifications.push(t.state);}catch{}}});
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
async function until(check,ms){const end=Date.now()+ms;while(Date.now()<end){if(await check())return;await sleep(150);}throw Error('Timed out waiting for test condition');}
async function api(route,method='GET',body){const r=await fetch(url+route,{method,headers:{Authorization:'Basic '+Buffer.from('quotabar-test:'+password).toString('base64'),'Content-Type':'application/json'},body:body===undefined?undefined:JSON.stringify(body),signal:AbortSignal.timeout(30000)});if(!r.ok)throw Error(`${method} ${route}: HTTP ${r.status}`);return r.status===204?null:r.json();}
let session,phase='starting';
(async()=>{
  try{
    await until(()=>url,15000);
    phase='server-ready'; console.log(JSON.stringify({phase})); const spec=await api('/doc');
    const paths=Object.keys(spec.paths);
    if(!paths.includes('/permission')||!paths.includes('/permission/{requestID}/reply'))throw Error('Unsupported permission API: '+paths.filter(p=>p.includes('permission')).join(','));
    phase='creating-session'; console.log(JSON.stringify({phase})); session=await api('/session','POST',{title:'QuotaBar permission integration test',permission:[{permission:'*',pattern:'*',action:'deny'},{permission:'bash',pattern:'*',action:'ask'}]});
    phase='session-ready'; console.log(JSON.stringify({phase})); await api(`/session/${session.id}/prompt_async`,'POST',{parts:[{type:'text',text:'Call the bash tool exactly once with echo QUOTABAR_PERMISSION_TEST. Do not read or modify files. If permission is denied, stop and reply DENIED. Do not try another tool.'}]});
    let pending;
    phase='waiting-permission';
    await until(async()=>{const p=await api('/permission');pending=p.find(x=>x.sessionID===session.id);return pending;},45000);
    await until(()=>notifications.includes('waiting_approval'),7000);
    await api(`/permission/${pending.id}/reply`,'POST',{reply:'reject'});
    phase='waiting-end';
    await until(()=>states.includes('ended')||states.includes('failed'),30000);
    await sleep(2500);
    const report={states,notifications,permissionRejected:true,fixture:root};
    report.passed=approvalPassed(report);
    fs.writeFileSync(path.join(root,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report));
    process.exitCode=report.passed?0:2;
  }catch(e){const report={passed:false,phase,error:e.message,states,notifications,fixture:root};fs.writeFileSync(path.join(root,'report.json'),JSON.stringify(report,null,2));console.log(JSON.stringify(report));process.exitCode=2;}
  finally{if(session)await api(`/session/${session.id}/abort`,'POST').catch(()=>{});server.kill();watcher.kill();}
})().catch(()=>{server.kill();watcher.kill();process.exitCode=2;});
