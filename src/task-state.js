/* Shared browser/CommonJS task presentation. */
(function (root) {
  const labels = {
    working: ['正在工作', 'Working'], waiting_approval: ['等待授权', 'Approval needed'],
    waiting_input: ['等待输入', 'Input needed'], ended: ['本轮已停止', 'Turn stopped'],
    failed: ['发生错误', 'Error'], interrupted: ['已中断', 'Interrupted'], unknown: ['状态未知', 'Unknown'],
  };
  // Backend captures only cwd basename and session id — no task title exists.
  const toolNames = {
    codex: ['Codex', 'Codex'], claude: ['Claude Code', 'Claude Code'],
    opencode: ['OpenCode', 'OpenCode'], kimi: ['Kimi Code', 'Kimi Code'],
  };
  function taskLabel(state, zh = true) { return (labels[state] || labels.unknown)[zh ? 0 : 1]; }
  function toolName(tool, zh = true) {
    const known = toolNames[String(tool || '').toLowerCase()];
    if (known) return known[zh ? 0 : 1];
    const raw = String(tool || '');
    return raw ? raw.charAt(0).toUpperCase() + raw.slice(1) : '—';
  }
  function taskColor(state) {
    return state === 'failed' ? '#F85149' : ['waiting_approval', 'waiting_input'].includes(state)
      ? '#D29922' : state === 'working' ? '#58a6ff' : '#8B949E';
  }
  // Shortest session-id prefix that is unique among same-tool sessions (min 8 chars).
  // Peers are filtered by the explicitly passed tool — the same session id may exist
  // under different tools. If no shorter prefix is unique, the full id is returned
  // as a last resort; complete paths are never part of this display path.
  function shortSessionId(tasks, sessionId, tool, minLen = 8) {
    const id = String(sessionId || '');
    if (id.length <= minLen) return id;
    const peers = (Array.isArray(tasks) ? tasks : [])
      .filter(t => t.tool === tool && t.session_id !== id).map(t => String(t.session_id));
    for (let len = minLen; len < id.length; len++) {
      const prefix = id.slice(0, len);
      if (!peers.some(p => p.length >= len && p.slice(0, len) === prefix)) return prefix;
    }
    return id;
  }
  function endedHint(zh = true) {
    return zh ? '仅表示本轮停止，不代表任务目标完成；计划任务后续调度状态未知'
      : 'Only means this turn stopped — not that the task goal is done; future scheduled runs are unknown';
  }
  function taskSummary(tasks, zh = true) {
    const attention = tasks.find(t => ['waiting_approval', 'waiting_input', 'failed'].includes(t.state));
    return attention ? `${toolName(attention.tool, zh)} · ${taskLabel(attention.state, zh)}` : '';
  }
  const api = { taskLabel, toolName, taskColor, taskSummary, shortSessionId, endedHint };
  Object.assign(root, api);
  if (typeof module !== 'undefined') module.exports = api;
})(globalThis);
