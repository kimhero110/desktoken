/* Shared browser/CommonJS task presentation. */
(function (root) {
  const labels = {
    working: ['正在工作', 'Working'], waiting_approval: ['等待授权', 'Approval needed'],
    waiting_input: ['等待输入', 'Input needed'], ended: ['本轮结束', 'Turn ended'],
    failed: ['发生错误', 'Error'], interrupted: ['已中断', 'Interrupted'], unknown: ['状态未知', 'Unknown'],
  };
  function taskLabel(state, zh = true) { return (labels[state] || labels.unknown)[zh ? 0 : 1]; }
  function taskColor(state) {
    return state === 'failed' ? '#F85149' : ['waiting_approval', 'waiting_input'].includes(state)
      ? '#D29922' : state === 'working' ? '#58a6ff' : state === 'ended' ? '#3FB950' : '#8B949E';
  }
  function taskSummary(tasks, zh = true) {
    const attention = tasks.find(t => ['waiting_approval', 'waiting_input', 'failed'].includes(t.state));
    return attention ? `${attention.tool} · ${taskLabel(attention.state, zh)}` : '';
  }
  const api = { taskLabel, taskColor, taskSummary };
  Object.assign(root, api);
  if (typeof module !== 'undefined') module.exports = api;
})(globalThis);
