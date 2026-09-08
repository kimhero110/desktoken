const { test, mock } = require('node:test');
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
// Requiring must never spawn anything (main guarded by require.main).
const { diagnose, summarize, FAILURE_CATEGORIES, DEFAULT_TIMEOUT_MS } = require('../scripts/diagnose-codex-hooks.cjs');

const ALLOWED_METHODS = ['initialize', 'initialized', 'hooks/list'];

function fakeChild() {
  const child = new EventEmitter();
  child.writes = [];
  child.stdin = new EventEmitter();
  child.stdin.write = (raw) => { child.writes.push(JSON.parse(raw)); };
  child.stdout = new EventEmitter();
  child.stderr = { resume() {} };
  child.killCalls = 0;
  child.kill = () => { child.killCalls++; };
  return child;
}

function emitLine(child, message) {
  child.stdout.emit('data', Buffer.from(JSON.stringify(message) + '\n'));
}

function hookEntry(overrides = {}) {
  return {
    cwd: 'C:\\Users\\secret\\proj',
    warnings: ['warning text secret'],
    errors: ['error text secret'],
    hooks: [],
    ...overrides,
  };
}

test('module exports pure helpers and does not spawn on require', () => {
  assert.equal(typeof diagnose, 'function');
  assert.equal(typeof summarize, 'function');
  assert.deepEqual(FAILURE_CATEGORIES, ['rpc_error', 'spawn_error', 'timeout', 'invalid_response']);
  assert.equal(DEFAULT_TIMEOUT_MS, 20000);
});

test('actual observed state: 8 enabled untrusted hooks -> needs_trust, exit-safe, sanitized', async () => {
  const child = fakeChild();
  const OBSERVED_EVENTS = ['preToolUse', 'permissionRequest', 'postToolUse', 'sessionStart', 'sessionEnd', 'userPromptSubmit', 'stop', 'interrupt'];
  const hooks = OBSERVED_EVENTS.map((eventName) => ({
    eventName,
    enabled: true,
    trustStatus: 'untrusted',
    sourcePath: 'C:\\Users\\secret\\.codex\\hooks.json',
    isManaged: false,
  }));
  const pending = diagnose({ spawnImpl: () => child });
  const line1 = JSON.stringify({ id: 1, result: { ok: true } }) + '\n';
  child.stdout.emit('data', Buffer.from(line1.slice(0, 5)));
  child.stdout.emit('data', Buffer.from(line1.slice(5)));
  emitLine(child, { id: 2, result: { data: [hookEntry({ hooks })] } });
  const summary = await pending;
  assert.equal(summary.ok, true);
  assert.equal(summary.category, 'needs_trust');
  assert.deepEqual(summary.counts, { directories: 1, hooks: 8, enabled: 8, enabledUntrusted: 8, managed: 0 });
  assert.equal(summary.events.length, 8);
  assert.deepEqual(summary.events[0], { event: 'preToolUse', enabled: true, trust: 'untrusted', managed: false });
  assert.deepEqual(summary.events.map((e) => e.event), OBSERVED_EVENTS);
  assert.equal(summary.desktopConnectionVerified, false);
  assert.ok(!FAILURE_CATEGORIES.includes(summary.category), 'needs_trust must exit zero');
  assert.ok(child.writes.every((m) => ALLOWED_METHODS.includes(m.method)), 'no model or thread calls');
  assert.equal(child.killCalls, 1, 'owned child killed exactly once');
});

test('all enabled hooks trusted -> ready, but desktop still unverified', async () => {
  const child = fakeChild();
  const hooks = [
    { eventName: 'stop', enabled: true, trustStatus: 'trusted', isManaged: true },
    { eventName: 'userPromptSubmit', enabled: true, trustStatus: 'trusted', isManaged: false },
  ];
  const pending = diagnose({ spawnImpl: () => child });
  emitLine(child, { id: 1, result: {} });
  emitLine(child, { id: 2, result: { data: [hookEntry({ hooks })] } });
  const summary = await pending;
  assert.equal(summary.ok, true);
  assert.equal(summary.category, 'ready');
  assert.deepEqual(summary.counts, { directories: 1, hooks: 2, enabled: 2, enabledUntrusted: 0, managed: 1 });
  assert.equal(summary.desktopConnectionVerified, false);
});

test('hooks present but none enabled -> disabled; empty list -> no_hooks', async () => {
  const child = fakeChild();
  const pending = diagnose({ spawnImpl: () => child });
  emitLine(child, { id: 1, result: {} });
  emitLine(child, { id: 2, result: { data: [hookEntry({ hooks: [{ eventName: 'stop', enabled: false, trustStatus: 'trusted', isManaged: false }] })] } });
  const disabled = await pending;
  assert.equal(disabled.category, 'disabled');
  assert.equal(disabled.ok, true);

  const child2 = fakeChild();
  const pending2 = diagnose({ spawnImpl: () => child2 });
  emitLine(child2, { id: 1, result: {} });
  emitLine(child2, { id: 2, result: { data: [] } });
  const noHooks = await pending2;
  assert.equal(noHooks.category, 'no_hooks');
  assert.deepEqual(noHooks.counts, { directories: 0, hooks: 0, enabled: 0, enabledUntrusted: 0, managed: 0 });
});

test('malformed result shapes -> invalid_response, nonzero exit category', () => {
  const bad = [
    undefined,
    null,
    'string',
    { result: {} },
    { result: { data: 'nope' } },
    { result: { data: [{ hooks: 'nope' }] } },
    { result: { data: [{ hooks: [{ enabled: true, trustStatus: 'untrusted' }] }] } },
    { result: { data: [{ hooks: [{ eventName: 'stop', enabled: 'yes', trustStatus: 'untrusted' }] }] } },
    { result: { data: [{ hooks: [{ eventName: 'stop', enabled: true, trustStatus: 7 }] }] } },
    { result: { data: [{ hooks: [{ eventName: 'C:\\evil\\path', enabled: true, trustStatus: 'untrusted' }] }] } },
    { result: { data: [null] } },
    { result: { data: [{ hooks: [null] }] } },
  ];
  for (const response of bad) {
    const summary = summarize(response);
    assert.equal(summary.ok, false);
    assert.equal(summary.category, 'invalid_response');
    assert.ok(FAILURE_CATEGORIES.includes(summary.category));
    assert.equal(summary.desktopConnectionVerified, false);
  }
});

test('redaction: summary never contains paths, warnings, errors, or commands', () => {
  const response = {
    id: 2,
    result: {
      data: [{
        cwd: 'C:\\Users\\secret\\proj',
        warnings: ['warning text secret'],
        errors: ['error text secret'],
        hooks: [{
          eventName: 'preToolUse',
          enabled: true,
          trustStatus: 'untrusted',
          sourcePath: 'C:\\Users\\secret\\.codex\\hooks.json',
          command: 'curl http://evil.example',
          transcript: 'user prompt secret',
          isManaged: false,
        }],
      }],
    },
  };
  const text = JSON.stringify(summarize(response));
  for (const leak of ['secret', 'cwd', 'warnings', 'errors', 'sourcePath', 'command', 'transcript', 'curl', 'evil']) {
    assert.ok(!text.includes(leak), `summary leaked "${leak}"`);
  }
});

test('RPC errors on initialize or hooks/list -> rpc_error', async () => {
  const child = fakeChild();
  const pending = diagnose({ spawnImpl: () => child });
  emitLine(child, { id: 1, error: { code: -32000, message: 'boom secret' } });
  const summary = await pending;
  assert.equal(summary.ok, false);
  assert.equal(summary.category, 'rpc_error');
  assert.ok(FAILURE_CATEGORIES.includes(summary.category));
  assert.ok(!JSON.stringify(summary).includes('boom'));

  const child2 = fakeChild();
  const pending2 = diagnose({ spawnImpl: () => child2 });
  emitLine(child2, { id: 1, result: {} });
  emitLine(child2, { id: 2, error: { code: -32000, message: 'boom secret' } });
  const summary2 = await pending2;
  assert.equal(summary2.category, 'rpc_error');
});

test('synchronous spawn throw (WindowsApps EPERM style) -> spawn_error', async () => {
  const summary = await diagnose({ spawnImpl: () => { const e = new Error('EPERM secret'); e.code = 'EPERM'; throw e; } });
  assert.equal(summary.ok, false);
  assert.equal(summary.category, 'spawn_error');
  assert.ok(!JSON.stringify(summary).includes('EPERM'));
});

test('asynchronous spawn error then close -> spawn_error, finished once, owned child killed once', async () => {
  const child = fakeChild();
  const pending = diagnose({ spawnImpl: () => child });
  child.emit('error', Object.assign(new Error('EPERM secret'), { code: 'EPERM' }));
  child.emit('close');
  child.emit('error', new Error('late'));
  const summary = await pending;
  assert.equal(summary.category, 'spawn_error');
  assert.equal(child.killCalls, 1);
});

test('premature close before any response -> spawn_error', async () => {
  const child = fakeChild();
  const pending = diagnose({ spawnImpl: () => child });
  child.emit('close');
  const summary = await pending;
  assert.equal(summary.category, 'spawn_error');
});

test('managed hooks are eligible only with isManaged; never labeled untrusted', async () => {
  const child = fakeChild();
  const pending = diagnose({ spawnImpl: () => child });
  emitLine(child, { id: 1, result: {} });
  emitLine(child, { id: 2, result: { data: [hookEntry({ hooks: [
    { eventName: 'stop', enabled: true, trustStatus: 'managed', isManaged: true },
    { eventName: 'preToolUse', enabled: true, trustStatus: 'trusted', isManaged: false },
  ] })] } });
  const summary = await pending;
  assert.equal(summary.category, 'ready');
  assert.deepEqual(summary.counts, { directories: 1, hooks: 2, enabled: 2, enabledUntrusted: 0, managed: 1 });
  assert.equal(summary.events[0].trust, 'managed');
  assert.equal(summary.events[0].managed, true);

  const child2 = fakeChild();
  const pending2 = diagnose({ spawnImpl: () => child2 });
  emitLine(child2, { id: 1, result: {} });
  emitLine(child2, { id: 2, result: { data: [hookEntry({ hooks: [
    { eventName: 'stop', enabled: true, trustStatus: 'managed' },
  ] })] } });
  const inconsistent = await pending2;
  assert.equal(inconsistent.ok, false);
  assert.equal(inconsistent.category, 'invalid_response');
});

test('modified enabled hooks still require a trust step', async () => {
  const summary = summarize({ result: { data: [{ hooks: [
    { eventName: 'stop', enabled: true, trustStatus: 'modified', isManaged: false },
  ] }] } });
  assert.equal(summary.ok, true);
  assert.equal(summary.category, 'needs_trust');
  assert.equal(summary.counts.enabledUntrusted, 1);
});

test('unknown trust status or event name -> invalid_response, never needs_trust', () => {
  const bad = [
    { result: { data: [{ hooks: [{ eventName: 'stop', enabled: true }] }] } },
    { result: { data: [{ hooks: [{ eventName: 'stop', enabled: true, trustStatus: 'unknown' }] }] } },
    { result: { data: [{ hooks: [{ eventName: 'stop', enabled: true, trustStatus: 'trusted/../../etc' }] }] } },
    { result: { data: [{ hooks: [{ eventName: 'WeirdEvent', enabled: true, trustStatus: 'trusted' }] }] } },
    { result: { data: [{ hooks: [{ eventName: 'Stop', enabled: true, trustStatus: 'trusted' }] }] } },
    { result: { data: [{ hooks: [{ eventName: 'UserPromptSubmit', enabled: true, trustStatus: 'trusted' }] }] } },
  ];
  for (const response of bad) {
    const summary = summarize(response);
    assert.equal(summary.ok, false);
    assert.equal(summary.category, 'invalid_response');
    assert.ok(FAILURE_CATEGORIES.includes(summary.category));
  }
});

test('stdin error / EPIPE is a sanitized spawn_error, not an unhandled stream error', async () => {
  const child = fakeChild();
  const pending = diagnose({ spawnImpl: () => child });
  child.stdin.emit('error', Object.assign(new Error('EPIPE secret'), { code: 'EPIPE' }));
  const summary = await pending;
  assert.equal(summary.ok, false);
  assert.equal(summary.category, 'spawn_error');
  assert.ok(!JSON.stringify(summary).includes('EPIPE'));
  assert.equal(child.killCalls, 1);
});

test('timeout fires once, cleans up timer, kills only the owned child', async () => {
  mock.timers.enable({ apis: ['setTimeout'] });
  try {
    const child = fakeChild();
    const pending = diagnose({ spawnImpl: () => child, timeoutMs: 1000 });
    mock.timers.tick(999);
    assert.equal(child.killCalls, 0);
    mock.timers.tick(2);
    const summary = await pending;
    assert.equal(summary.ok, false);
    assert.equal(summary.category, 'timeout');
    assert.ok(FAILURE_CATEGORIES.includes(summary.category));
    assert.equal(child.killCalls, 1);
    mock.timers.tick(5000);
    assert.equal(child.killCalls, 1, 'cleanup runs exactly once');
  } finally {
    mock.timers.reset();
  }
});

test('oversized response buffer is bounded -> invalid_response', async () => {
  const child = fakeChild();
  const pending = diagnose({ spawnImpl: () => child });
  child.stdout.emit('data', Buffer.alloc(1024 * 1024 + 1, 0x61));
  const summary = await pending;
  assert.equal(summary.category, 'invalid_response');
});

test('non-JSON and notification lines are ignored until a real response arrives', async () => {
  const child = fakeChild();
  const pending = diagnose({ spawnImpl: () => child });
  child.stdout.emit('data', Buffer.from('not json\n'));
  emitLine(child, { method: 'log/updated', params: { text: 'noise' } });
  emitLine(child, { id: 1, result: {} });
  emitLine(child, { id: 2, result: { data: [{ hooks: [{ eventName: 'stop', enabled: true, trustStatus: 'trusted', isManaged: false }] }] } });
  const summary = await pending;
  assert.equal(summary.category, 'ready');
});
