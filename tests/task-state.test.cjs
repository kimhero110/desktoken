const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const { taskLabel, taskSummary } = require('../src/task-state.js');

test('unknown is distinct from turn end; approval takes attention', () => {
  assert.equal(taskLabel('anything'), '状态未知');
  assert.equal(taskLabel('ended'), '本轮结束');
  assert.equal(taskSummary([{tool:'codex',state:'working'},{tool:'claude',state:'waiting_approval'}]), 'claude · 等待授权');
  assert.equal(taskSummary([{tool:'codex',state:'ended'}]), '');
});

test('OpenCode plugin forwards only lifecycle metadata in order, never transcript text', async () => {
  const messages = [];
  const source = fs.readFileSync(require.resolve('../integrations/opencode.js'), 'utf8')
    .replace("import { spawnSync } from 'node:child_process';", '')
    .replace('__QUOTABAR_EXE_JSON__', '"test.exe"').replace('export const QuotaBarTasks', 'globalThis.QuotaBarTasks');
  const context = vm.createContext({spawnSync(exe,args,options) {
    assert.equal(exe,'test.exe'); assert.equal(options.windowsHide,true);
    assert.equal(options.timeout,1500);
    messages.push(JSON.parse(options.input));
    return {status:0};
  }});
  vm.runInContext(source,context);
  const plugin = await context.QuotaBarTasks({directory:'C:/demo'});
  for (const [type,properties] of [
    ['message.updated',{sessionID:'s',text:'SECRET'}],
    ['permission.asked',{sessionID:'s',patterns:['SECRET']}],
    ['permission.replied',{sessionID:'s'}],
    ['session.status',{sessionID:'s',status:{type:'busy'}}],
    ['session.idle',{sessionID:'s'}],
  ]) {
    const before = messages.length;
    const pending = plugin.event({event:{type,properties}});
    if(type !== 'message.updated') assert.equal(messages.length,before+1,'lifecycle event must be written before the CLI can exit');
    await pending;
  }
  await new Promise(resolve=>setImmediate(resolve));
  assert.deepEqual(messages.map(m=>m.hook_event_name), ['permission.asked','permission.replied','busy','session.idle']);
  assert.ok(messages.every(m=>m.session_id==='s'));
  assert.ok(!JSON.stringify(messages).includes('SECRET'));
});

test('one resolved approval must not clear another outstanding request in the same session', async () => {
  const messages=[];
  const source=fs.readFileSync(require.resolve('../integrations/opencode.js'),'utf8')
    .replace("import { spawnSync } from 'node:child_process';",'').replace('__QUOTABAR_EXE_JSON__','"test.exe"')
    .replace('export const QuotaBarTasks','globalThis.QuotaBarTasks');
  const context=vm.createContext({spawnSync(_exe,_args,options){messages.push(JSON.parse(options.input));return {status:0};}});
  vm.runInContext(source,context);const plugin=await context.QuotaBarTasks({directory:'demo'});
  const emit=async(type,properties)=>plugin.event({event:{type,properties:{sessionID:'s',...properties}}});
  await emit('permission.asked',{id:'a'});
  await emit('permission.asked',{id:'b'});
  await emit('permission.replied',{requestID:'a'});
  assert.equal(messages.at(-1).hook_event_name,'permission.asked');
  await emit('session.status',{status:{type:'busy'}});
  assert.equal(messages.at(-1).hook_event_name,'permission.asked');
  await emit('permission.replied',{requestID:'b'});
  assert.equal(messages.at(-1).hook_event_name,'permission.replied');
});
