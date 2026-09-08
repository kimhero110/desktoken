const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const { taskLabel, taskColor, taskSummary, toolName, shortSessionId, endedHint } = require('../src/task-state.js');

test('unknown is distinct from turn end; approval takes attention', () => {
  assert.equal(taskLabel('anything'), '状态未知');
  assert.equal(taskLabel('ended'), '本轮已停止');
  assert.equal(taskLabel('ended', false), 'Turn stopped');
  assert.equal(taskSummary([{tool:'codex',state:'working'},{tool:'claude',state:'waiting_approval'}]), 'Claude Code · 等待授权');
  assert.equal(taskSummary([{tool:'codex',state:'ended'}]), '');
});

test('ended renders neutral gray, never success green', () => {
  assert.equal(taskColor('ended'), '#8B949E');
  assert.notEqual(taskColor('ended'), taskColor('working'));
  // waiting states keep the strongest attention color
  assert.equal(taskColor('waiting_approval'), '#D29922');
  assert.equal(taskColor('waiting_input'), '#D29922');
  assert.equal(taskColor('failed'), '#F85149');
});

test('ended hint is honest about scope and scheduler ignorance', () => {
  assert.ok(endedHint().includes('不代表任务目标完成'));
  assert.ok(endedHint().includes('调度状态未知'));
  assert.ok(endedHint(false).includes('not that the task goal is done'));
});

test('toolName shows human names, not raw identifiers', () => {
  assert.equal(toolName('codex'), 'Codex');
  assert.equal(toolName('claude'), 'Claude Code');
  assert.equal(toolName('opencode', false), 'OpenCode');
  assert.equal(toolName('KIMI'), 'Kimi Code');
  assert.equal(toolName('weird-tool'), 'Weird-tool');
  assert.equal(toolName(''), '—');
});

test('shortSessionId returns at least 8 chars and stays 8 when unique', () => {
  const tasks = [
    { tool: 'codex', session_id: 'aaaaaaaa11111111' },
    { tool: 'codex', session_id: 'bbbbbbbb22222222' },
  ];
  assert.equal(shortSessionId(tasks, 'aaaaaaaa11111111', 'codex'), 'aaaaaaaa');
  assert.equal(shortSessionId(tasks, 'shortid', 'codex'), 'shortid');
  assert.equal(shortSessionId([], 'aaaaaaaa11111111', 'codex'), 'aaaaaaaa');
});

test('shortSessionId extends past 8 on same-tool first-8 collision', () => {
  const tasks = [
    { tool: 'codex', session_id: 'abcdefgh0001' },
    { tool: 'codex', session_id: 'abcdefgh9999' },
  ];
  assert.equal(shortSessionId(tasks, 'abcdefgh0001', 'codex'), 'abcdefgh0');
  assert.equal(shortSessionId(tasks, 'abcdefgh9999', 'codex'), 'abcdefgh9');
});

test('shortSessionId does not extend for colliding ids under different tools', () => {
  const tasks = [
    { tool: 'codex', session_id: 'abcdefgh0001' },
    { tool: 'claude', session_id: 'abcdefgh9999' },
  ];
  assert.equal(shortSessionId(tasks, 'abcdefgh0001', 'codex'), 'abcdefgh');
  assert.equal(shortSessionId(tasks, 'abcdefgh9999', 'claude'), 'abcdefgh');
});

test('shortSessionId uses the passed tool even when the same id exists under multiple tools', () => {
  // Regression: inferring the tool from the first id match picked codex's peers
  // for the claude row, hiding a real claude-side collision.
  const tasks = [
    { tool: 'codex', session_id: 'abcdefgh0001' },
    { tool: 'claude', session_id: 'abcdefgh0001' },
    { tool: 'claude', session_id: 'abcdefgh9999' },
  ];
  // codex side: no codex peer collides, 8 chars suffice
  assert.equal(shortSessionId(tasks, 'abcdefgh0001', 'codex'), 'abcdefgh');
  // claude side: same id also exists under claude with a colliding peer, must extend
  assert.equal(shortSessionId(tasks, 'abcdefgh0001', 'claude'), 'abcdefgh0');
  assert.equal(shortSessionId(tasks, 'abcdefgh9999', 'claude'), 'abcdefgh9');
});

test('shortSessionId falls back to the full id when prefixes never diverge within length', () => {
  const tasks = [
    { tool: 'codex', session_id: 'abcdefgh' },
    { tool: 'codex', session_id: 'abcdefgh' },
  ];
  // same session id twice: peer filter excludes itself, so 8-char prefix suffices
  assert.equal(shortSessionId(tasks, 'abcdefgh', 'codex'), 'abcdefgh');
  assert.equal(shortSessionId([{tool:'codex',session_id:'abcdefgh1'},{tool:'codex',session_id:'abcdefgh'}], 'abcdefgh', 'codex'), 'abcdefgh');
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
