// Read-only Codex hooks/list diagnostic. No model requests or trust changes.
const {spawn}=require('node:child_process');
const server=spawn(process.argv[2] || 'codex.exe',['app-server','--stdio'],{windowsHide:true,stdio:['pipe','pipe','pipe']});
const timer=setTimeout(()=>{console.error('hook diagnosis timed out');server.kill();process.exitCode=1;},20000);
server.stderr.resume();let buffer='';
const send=x=>server.stdin.write(JSON.stringify(x)+'\n');
server.stdout.on('data',x=>{buffer+=x;let i;while((i=buffer.indexOf('\n'))>=0){const line=buffer.slice(0,i);buffer=buffer.slice(i+1);let v;try{v=JSON.parse(line);}catch{continue;}
if(v.id===1){if(v.error){console.log(JSON.stringify(v.error));clearTimeout(timer);server.kill();return;}send({method:'initialized',params:{}});send({id:2,method:'hooks/list',params:{cwds:[process.cwd()]}});}
if(v.id===2){console.log(JSON.stringify(v.error||v.result?.data?.map(d=>({cwd:d.cwd,warnings:d.warnings,errors:d.errors,hooks:d.hooks.map(h=>({event:h.eventName,enabled:h.enabled,trust:h.trustStatus,source:h.sourcePath,managed:h.isManaged}))}))));clearTimeout(timer);server.kill();}
}});
server.on('error',e=>{clearTimeout(timer);console.error(e.message);process.exitCode=1;});
send({id:1,method:'initialize',params:{clientInfo:{name:'quotabar_diagnostics',title:'QuotaBar hook diagnosis',version:'1'},capabilities:{experimentalApi:true,requestAttestation:false}}});