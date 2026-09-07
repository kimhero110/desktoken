// OpenCode V1 local plugin. The settings screen substitutes the executable path.
import { spawnSync } from 'node:child_process';
const executable = __QUOTABAR_EXE_JSON__;

export const QuotaBarTasks = async ({ directory }) => {
  // A session may wait on several parallel tools. Resolve by request ID, not by
  // the last event alone; one reply/busy event must not hide another prompt.
  const pending = new Map();
  const waitingName = session => {
    const entry = pending.get(session);
    if (entry?.overflow) return 'unknown';
    const kinds = [...(entry?.requests.values() || [])];
    return kinds.includes('permission.asked') ? 'permission.asked' : kinds.includes('question.asked') ? 'question.asked' : null;
  };
  // The CLI can exit immediately after session.idle. Finish each small lifecycle
  // write before returning; an asynchronous queue loses the final event on exit.
  const send = (session, name) => {
    if (typeof session !== 'string' || !session || session.length > 256) return;
    try {
      spawnSync(executable, ['task-event', 'opencode'], {
        windowsHide: true, stdio: ['pipe', 'ignore', 'ignore'], timeout: 1500,
        input: JSON.stringify({ session_id: session, hook_event_name: name, cwd: directory }),
      });
    } catch { /* Best effort; never return an approval decision. */ }
  };
  return {
    event: async ({ event }) => {
      const p = event.properties || {};
      const id = p.sessionID || p.info?.id;
      if (typeof id !== 'string' || !id || id.length > 256) return;
      if (['permission.asked', 'question.asked'].includes(event.type) && typeof p.id === 'string' && p.id.length <= 256) {
        if (!pending.has(id)) {
          if (pending.size >= 100) pending.delete(pending.keys().next().value);
          pending.set(id, {requests: new Map(), overflow: false});
        }
        const entry = pending.get(id);
        if (entry.requests.size >= 64 && !entry.requests.has(p.id)) entry.overflow = true;
        else entry.requests.set(p.id, event.type);
        send(id, waitingName(id)); return;
      }
      if (['permission.replied', 'question.replied', 'question.rejected'].includes(event.type)) {
        const entry = pending.get(id);
        entry?.requests.delete(p.requestID);
        const waiting = waitingName(id);
        if (!waiting) pending.delete(id);
        send(id, waiting || event.type); return;
      }
      if (['session.idle', 'session.error', 'session.deleted'].includes(event.type)) pending.delete(id);
      if (event.type === 'session.status') {
        if (['busy', 'retry'].includes(p.status?.type)) send(id, waitingName(id) || p.status.type);
        // Idle is handled by session.idle; never overwrite an error with status.idle.
      } else if (['session.idle', 'session.error', 'session.deleted', 'permission.asked',
        'permission.replied', 'question.asked', 'question.replied', 'question.rejected'].includes(event.type)) {
        send(id, event.type);
      }
    },
  };
};
