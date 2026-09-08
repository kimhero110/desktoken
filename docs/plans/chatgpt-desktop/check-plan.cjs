// Structural plan checks only. Passing does not prove desktop support or usability.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict');
const root=__dirname;
const plan=fs.readFileSync(path.join(root,process.argv[2]||'PLAN-v2.md'),'utf8');
const ui=fs.readFileSync(path.join(root,'UI-MAP.md'),'utf8');
const checks=[];
for(let n=1;n<=12;n++) {
  const id=String(n).padStart(2,'0');
  assert.ok(plan.includes('CG-'+id)&&plan.includes('AT-'+id),'Missing requirement/test reference '+id);
  checks.push({id:'MAP-'+id,status:'passed'});
}
for(const id of ['U1','U2','U3','U4','U5'])assert.ok(plan.includes(id),'Missing workflow '+id);
for(const name of ['立即刷新','迷你模式','复制诊断','检查更新','GitHub','赞助','开机启动','平台凭据','自定义监视','安装/移除'])assert.ok(ui.includes(name),'Missing migration '+name);
assert.ok(plan.includes('本轮只授权P/R'));
console.log(JSON.stringify({kind:'plan-structure-only',checks,workflowReferences:5,migrationGroups:10,productTests:'not_run',passed:true},null,2));
