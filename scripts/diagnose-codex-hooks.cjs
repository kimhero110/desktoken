// Read-only, bounded Codex hooks/list diagnostic. No model requests, no thread
// calls, no trust changes, no settings writes. Output is a sanitized JSON
// summary only: no cwd, source paths, warnings, error text, commands, or
// transcripts. This script can be required without spawning anything.
const { spawn: defaultSpawn } = require('node:child_process');

const DEFAULT_TIMEOUT_MS = 20000;
const MAX_LINE_BUFFER = 1024 * 1024;
const FAILURE_CATEGORIES = ['rpc_error', 'spawn_error', 'timeout', 'invalid_response'];
// Official locally generated HookTrustStatus enum: managed|untrusted|trusted|modified.
const TRUST_STATUSES = ['managed', 'untrusted', 'trusted', 'modified'];
// Exact generated HookEventName enum (lower camelCase) from hooks/list.
const EVENT_NAMES = [
  'preToolUse', 'permissionRequest', 'postToolUse', 'preCompact', 'postCompact',
  'sessionStart', 'sessionEnd', 'userPromptSubmit', 'subagentStart',
  'subagentStop', 'stop', 'interrupt',
];

function isPlainObject(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function failureSummary(category) {
  return { ok: false, category, counts: null, events: [], desktopConnectionVerified: false };
}

// Pure reducer: maps a parsed hooks/list JSON-RPC response to the sanitized
// summary schema. Robustly validates the result shape; never throws.
function summarize(response) {
  if (!isPlainObject(response)) return failureSummary('invalid_response');
  if (response.error !== undefined && response.error !== null) return failureSummary('rpc_error');
  const data = isPlainObject(response.result) ? response.result.data : undefined;
  if (!Array.isArray(data)) return failureSummary('invalid_response');
  const events = [];
  let hooks = 0;
  let enabled = 0;
  let enabledUntrusted = 0;
  let managed = 0;
  for (const entry of data) {
    if (!isPlainObject(entry) || !Array.isArray(entry.hooks)) return failureSummary('invalid_response');
    for (const hook of entry.hooks) {
      if (!isPlainObject(hook)) return failureSummary('invalid_response');
      const eventName = hook.eventName;
      const trust = hook.trustStatus;
      if (typeof eventName !== 'string' || !EVENT_NAMES.includes(eventName)) return failureSummary('invalid_response');
      if (typeof trust !== 'string' || !TRUST_STATUSES.includes(trust)) return failureSummary('invalid_response');
      if (typeof hook.enabled !== 'boolean') return failureSummary('invalid_response');
      if (hook.isManaged !== undefined && typeof hook.isManaged !== 'boolean') return failureSummary('invalid_response');
      const isEnabled = hook.enabled;
      const isManaged = hook.isManaged === true;
      if (trust === 'managed' && !isManaged) return failureSummary('invalid_response');
      hooks++;
      if (isEnabled) {
        enabled++;
        // Eligible for execution: explicitly trusted, or managed hooks flagged
        // isManaged. Managed hooks are never labeled untrusted; untrusted and
        // modified (changed since trust) enabled hooks require a trust step.
        const eligible = trust === 'trusted' || (trust === 'managed' && isManaged);
        if (!eligible) enabledUntrusted++;
      }
      if (isManaged) managed++;
      events.push({ event: eventName, enabled: isEnabled, trust, managed: isManaged });
    }
  }
  const category = hooks === 0 ? 'no_hooks' : enabled === 0 ? 'disabled' : enabledUntrusted > 0 ? 'needs_trust' : 'ready';
  return {
    ok: true,
    category,
    counts: { directories: data.length, hooks, enabled, enabledUntrusted, managed },
    events,
    // Never claim the ChatGPT desktop app is connected, even when ready.
    desktopConnectionVerified: false,
  };
}

// Runs one bounded app-server session: initialize -> initialized ->
// hooks/list, then resolves with the sanitized summary. The spawn
// implementation is injectable so failure paths can be tested without any
// real child process or network. The promise never rejects.
function diagnose({ executable = 'codex.exe', spawnImpl = defaultSpawn, timeoutMs = DEFAULT_TIMEOUT_MS, cwd = process.cwd() } = {}) {
  return new Promise((resolve) => {
    let child = null;
    let settled = false;
    let timer = null;
    let buffer = '';
    const finish = (summary) => {
      if (settled) return;
      settled = true;
      if (timer !== null) {
        clearTimeout(timer);
        timer = null;
      }
      // Kill only the child we own, at most once, and only if killable.
      if (child !== null && typeof child.kill === 'function') {
        try { child.kill(); } catch {}
      }
      resolve(summary);
    };
    let server;
    try {
      server = spawnImpl(executable, ['app-server', '--stdio'], { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
    } catch {
      // Synchronous spawn failure (for example WindowsApps runtime EPERM).
      resolve(failureSummary('spawn_error'));
      return;
    }
    if (!server || typeof server.on !== 'function' || !server.stdin || !server.stdout) {
      finish(failureSummary('spawn_error'));
      return;
    }
    child = server;
    timer = setTimeout(() => finish(failureSummary('timeout')), timeoutMs);
    server.on('error', () => finish(failureSummary('spawn_error')));
    // Broken stdin pipe (EPIPE when the child exits early) must not become an
    // unhandled stream error; treat it as a sanitized spawn failure.
    if (typeof server.stdin.on === 'function') server.stdin.on('error', () => finish(failureSummary('spawn_error')));
    // Process exited before the protocol completed (for example EPERM close).
    server.on('close', () => finish(failureSummary('spawn_error')));
    if (server.stderr && typeof server.stderr.resume === 'function') server.stderr.resume();
    const send = (message) => {
      try { server.stdin.write(JSON.stringify(message) + '\n'); } catch {}
    };
    server.stdout.on('data', (chunk) => {
      if (settled) return;
      buffer += typeof chunk === 'string' ? chunk : String(chunk);
      if (buffer.length > MAX_LINE_BUFFER) {
        finish(failureSummary('invalid_response'));
        return;
      }
      let newline;
      while ((newline = buffer.indexOf('\n')) >= 0) {
        const line = buffer.slice(0, newline);
        buffer = buffer.slice(newline + 1);
        let message;
        try { message = JSON.parse(line); } catch { continue; }
        if (!isPlainObject(message)) continue;
        if (message.id === 1) {
          if (message.error) {
            finish(failureSummary('rpc_error'));
            return;
          }
          send({ method: 'initialized', params: {} });
          send({ id: 2, method: 'hooks/list', params: { cwds: [cwd] } });
        } else if (message.id === 2) {
          try {
            finish(summarize(message));
          } catch {
            finish(failureSummary('invalid_response'));
          }
          return;
        }
      }
    });
    send({
      id: 1,
      method: 'initialize',
      params: {
        clientInfo: { name: 'quotabar_diagnostics', title: 'QuotaBar hook diagnosis', version: '1' },
        capabilities: { experimentalApi: true, requestAttestation: false },
      },
    });
  });
}

function main() {
  const executable = process.argv[2] || 'codex.exe';
  diagnose({ executable }).then((summary) => {
    process.stdout.write(JSON.stringify(summary) + '\n');
    if (FAILURE_CATEGORIES.includes(summary.category)) process.exitCode = 1;
  });
}

if (require.main === module) main();

module.exports = { diagnose, summarize, failureSummary, FAILURE_CATEGORIES, DEFAULT_TIMEOUT_MS };
