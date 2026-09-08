(function (global) {
  'use strict';

  var CLOCK_BASE = Date.UTC(2026, 8, 8, 10, 0, 0);

  var QUOTA_SAMPLES = [
    { pct: 72, used: '$42.5', total: '$60.0' },
    { pct: 65, used: '$39.0', total: '$60.0' },
    { pct: 81, used: '$48.6', total: '$60.0' }
  ];

  var CUSTOM_DEFAULTS = {
    name: '示例自定义来源',
    poll: 30,
    endpoint: 'https://quota.example.invalid/api/v1',
    header: 'Authorization',
    prefix: 'Bearer ',
    window: '月额度 | data.used | data.limit | data.reset',
    invert: false
  };

  var TEMPLATES = {
    cn: {
      key: 'cn',
      name: 'Moonshot 国内（模板示例）',
      values: {
        name: 'Moonshot 国内（示例）',
        poll: 15,
        endpoint: 'https://cn.example.invalid/v1/quota',
        header: 'Authorization',
        prefix: 'Bearer ',
        window: '月额度 | data.used | data.limit',
        invert: false
      }
    },
    intl: {
      key: 'intl',
      name: 'Moonshot 国际（模板示例）',
      values: {
        name: 'Moonshot 国际（示例）',
        poll: 15,
        endpoint: 'https://intl.example.invalid/v1/quota',
        header: 'Authorization',
        prefix: 'Bearer ',
        window: '月额度 | data.used | data.limit | data.reset',
        invert: false
      }
    }
  };

  var TOOL_CONFIGS = {
    codex: {
      name: 'Codex CLI',
      path: 'C:\\Users\\示例用户\\DemoOnly\\.codex\\config.toml（虚构路径）',
      content: [
        '# 示例配置（虚构 · 演示，永不写入磁盘）',
        'model = "demo-model"',
        '',
        '[model_providers.quotabar_demo]',
        'name = "QuotaBar Demo"',
        'base_url = "https://demo.invalid/api"',
        'api_key_env = "QUOTABAR_DEMO_KEY"'
      ].join('\n')
    },
    claude: {
      name: 'Claude Code',
      path: 'C:\\Users\\示例用户\\DemoOnly\\.claude\\settings.json（虚构路径）',
      content: [
        '// 示例配置（虚构 · 演示，永不写入磁盘）',
        '{',
        '  "demo": true,',
        '  "apiKeyHelper": "quotabar-demo-helper（虚构）",',
        '  "primaryProvider": "demo-provider"',
        '}'
      ].join('\n')
    },
    opencode: {
      name: 'OpenCode V1',
      path: 'C:\\Users\\示例用户\\DemoOnly\\.config\\opencode\\opencode.json（虚构路径）',
      content: [
        '// 示例配置（虚构 · 演示，永不写入磁盘）',
        '{',
        '  "demo": true,',
        '  "provider": {',
        '    "demo": { "npm": "demo-provider", "api": "https://demo.invalid" }',
        '  }',
        '}'
      ].join('\n')
    }
  };

  var DIAG_SAMPLE = [
    '[诊断预览 · 固定脱敏样例 · 演示]',
    '应用版本: 0.0.0-prototype（示例）',
    '目标窗口: ChatGPTDesktopWnd（示例类名，已脱敏）',
    '进程路径: C:\\Users\\***\\***（已脱敏）',
    '凭据: <已脱敏>（仅示例掩码）',
    '网络: 未发起任何请求（演示）',
    '时钟: 合成演示时钟，非系统时钟'
  ].join('\n');

  var TASK_TEXT = {
    unknown: '未知（演示）',
    working: '工作中（演示）',
    needs_attention: '等待确认（演示）',
    ended: '已结束（演示）',
    cancelled: '已取消（演示）',
    failed_demo: '失败（演示）'
  };

  var TASK_DOT = {
    unknown: 'dot-unknown',
    working: 'dot-ok',
    needs_attention: 'dot-warn',
    ended: 'dot-unknown',
    cancelled: 'dot-unknown',
    failed_demo: 'dot-warn'
  };

  var QuotaBarDemoLogic = {
    reduceTaskEvents: function (taskEvents) {
      var order = [];
      var map = {};
      taskEvents.forEach(function (ev) {
        var key = ev.taskId + '#' + ev.generation;
        var cur = map[key];
        if (!cur) {
          cur = { id: ev.taskId, taskId: ev.taskId, generation: ev.generation, label: ev.label || ev.taskId, state: 'unknown', attention: null, note: null };
          map[key] = cur;
          order.push(cur);
        }
        if (ev.type === 'start') { cur.state = 'working'; }
        else if (ev.type === 'await_auth') { cur.state = 'needs_attention'; cur.attention = '授权'; }
        else if (ev.type === 'await_input') { cur.state = 'needs_attention'; cur.attention = '输入'; }
        else if (ev.type === 'cancel') { if (cur.state === 'working') { cur.state = 'cancelled'; cur.note = '已取消'; } }
        else if (ev.type === 'end') {
          if (cur.state === 'cancelled') { cur.note = '已取消 · 结束事件晚到'; }
          cur.state = 'ended';
        }
        else if (ev.type === 'error') { cur.state = 'failed_demo'; }
      });
      return order;
    },
    splitCurrentAndHistory: function (tasks) {
      var latest = {};
      tasks.forEach(function (t) {
        var key = t.taskId;
        if (!latest[key] || t.generation > latest[key].generation) { latest[key] = t; }
      });
      var current = [];
      var history = [];
      tasks.forEach(function (t) {
        if (latest[t.taskId] === t) { current.push(t); }
        else { history.push(t); }
      });
      return { current: current, history: history };
    },
    currentTasks: function (tasks) {
      return QuotaBarDemoLogic.splitCurrentAndHistory(tasks).current;
    },
    historyTasks: function (tasks) {
      return QuotaBarDemoLogic.splitCurrentAndHistory(tasks).history;
    },
    summarizeTasks: function (tasks) {
      var s = { total: tasks.length, working: 0, attention: 0, ended: 0, cancelled: 0, failed: 0, unknown: 0 };
      tasks.forEach(function (t) {
        if (t.state === 'working') { s.working += 1; }
        else if (t.state === 'needs_attention') { s.attention += 1; }
        else if (t.state === 'ended') { s.ended += 1; }
        else if (t.state === 'cancelled') { s.cancelled += 1; }
        else if (t.state === 'failed_demo') { s.failed += 1; }
        else { s.unknown += 1; }
      });
      return s;
    },
    workSummaryLabel: function (tasks) {
      var s = QuotaBarDemoLogic.summarizeTasks(tasks);
      if (s.total === 0 || (s.working + s.attention + s.ended + s.cancelled + s.failed) === 0) {
        return '任务状态：未知（演示）';
      }
      var parts = [];
      if (s.attention) { parts.push('等待确认 ' + s.attention); }
      if (s.working) { parts.push('工作中 ' + s.working); }
      if (s.ended) { parts.push('已结束 ' + s.ended); }
      if (s.cancelled) { parts.push('已取消 ' + s.cancelled); }
      if (s.failed) { parts.push('失败 ' + s.failed); }
      return parts.join(' · ') + '（演示）';
    }
  };

  var SCENARIOS = [
    {
      id: 'S01', name: '默认未验证',
      desc: '初始状态：应用能力未验证，无事件，提醒开启，无暂停。',
      expect: '悬浮条/面板显示“未验证（演示）”，额度摘要为“未知（连接未验证）”，最近事件为空态，无假时间。',
      apply: { connection: 'unverified', tasks: [], events: [], notifyEnabled: true, lastSubmit: 'none', pause: null, toast: null, clockOffset: 0 }
    },
    {
      id: 'S02', name: '应用缺失',
      desc: '未检测到 ChatGPT 桌面应用（示例）。',
      expect: '连接边界显示“应用缺失（示例）”，额度摘要为未知；打开按钮显示演示反馈，不激活任何窗口。',
      apply: { connection: 'absent', tasks: [], events: [], notifyEnabled: true, lastSubmit: 'none', pause: null, toast: null, clockOffset: 0 }
    },
    {
      id: 'S03', name: '身份冲突',
      desc: '检测到多个候选应用身份（示例），需人工确认。',
      expect: '连接边界显示“身份冲突（示例）”，额度摘要为未知，不显示任何已连接状态。',
      apply: {
        connection: 'conflict', tasks: [], notifyEnabled: true, lastSubmit: 'none', pause: null, toast: null, clockOffset: 70000,
        events: [{ t: '10:01:10', tag: '连接', label: '检测到多个候选身份（示例）· 需人工确认' }]
      }
    },
    {
      id: 'S04', name: '演示工作',
      desc: '可观察（演示）连接下，示例任务 SAMPLE-B 工作中。',
      expect: '额度摘要显示示例数值；工作摘要显示“工作中（演示）”；面板含任务开始示例事件。',
      apply: {
        connection: 'observable_demo', notifyEnabled: true, lastSubmit: 'none', pause: null, toast: null, clockOffset: 65000,
        tasks: [{ id: 'SAMPLE-B', label: '示例任务 B', state: 'working' }],
        events: [{ t: '10:01:05', tag: '任务', label: '任务开始 · SAMPLE-B（示例）' }]
      }
    },
    {
      id: 'S05', name: '授权等待',
      desc: '示例任务 SAMPLE-A 等待授权确认（needs_attention · 授权）。',
      expect: '出现演示通知（非系统通知）；点击“查看”（1 次操作）打开面板，任务状态清晰标“等待授权”。',
      apply: {
        connection: 'observable_demo', notifyEnabled: true, lastSubmit: 'none', pause: null, clockOffset: 180000,
        tasks: [{ id: 'SAMPLE-A', label: '示例任务 A', state: 'needs_attention', attention: '授权' }],
        events: [{ t: '10:03:00', tag: '任务', label: '等待授权 · SAMPLE-A（示例）' }],
        toast: { title: '等待授权 · SAMPLE-A（示例）', body: '演示通知：任务等待授权确认。非系统通知，点击“查看”打开面板。' }
      }
    },
    {
      id: 'S06', name: '输入等待',
      desc: '示例任务 SAMPLE-A 等待用户输入（needs_attention · 输入）。',
      expect: '同授权等待：演示通知可“查看”打开面板，任务状态标“等待输入”。',
      apply: {
        connection: 'observable_demo', notifyEnabled: true, lastSubmit: 'none', pause: null, clockOffset: 180000,
        tasks: [{ id: 'SAMPLE-A', label: '示例任务 A', state: 'needs_attention', attention: '输入' }],
        events: [{ t: '10:03:00', tag: '任务', label: '等待输入 · SAMPLE-A（示例）' }],
        toast: { title: '等待输入 · SAMPLE-A（示例）', body: '演示通知：任务等待用户输入。非系统通知，点击“查看”打开面板。' }
      }
    },
    {
      id: 'S07', name: '本轮结束',
      desc: '示例任务 SAMPLE-A 本轮已结束。',
      expect: '演示通知提示“本轮已结束”；面板任务状态为“已结束”，最近事件含开始与结束两条示例。',
      apply: {
        connection: 'observable_demo', notifyEnabled: true, lastSubmit: 'none', pause: null, clockOffset: 300000,
        tasks: [{ id: 'SAMPLE-A', label: '示例任务 A', state: 'ended' }],
        events: [
          { t: '10:01:05', tag: '任务', label: '任务开始 · SAMPLE-A（示例）' },
          { t: '10:05:00', tag: '任务', label: '本轮结束 · SAMPLE-A（示例）' }
        ],
        toast: { title: '本轮已结束 · SAMPLE-A（示例）', body: '演示通知：本轮对话已结束。非系统通知，点击“查看”打开面板。' }
      }
    },
    {
      id: 'S08', name: '断连',
      desc: '连接断开（示例）；断连不推断任务失败或完成，任务状态保持未知，历史仅作保留。',
      expect: '任务列表显示“未知（演示）”而非失败；额度摘要仍来自独立额度来源（示例）；连接不为绿色。',
      apply: {
        connection: 'disconnected', notifyEnabled: true, lastSubmit: 'none', pause: null, toast: null, clockOffset: 120000,
        tasks: [{ id: 'SAMPLE-A', label: '示例任务 A', state: 'unknown', note: '断连期间状态未知（示例）· 历史仅作保留，不推断失败/完成' }],
        events: [{ t: '10:02:00', tag: '连接', label: '连接断开（示例）· 任务状态置为未知，不推断失败/完成' }]
      }
    },
    {
      id: 'S09', name: '通知暂停',
      desc: '提醒暂停中（直到恢复，示例）；历史事件保留。',
      expect: '悬浮条与面板显示暂停标记；最近事件不因暂停被删除；测试通知不产生演示提交事件。',
      apply: {
        connection: 'unverified', tasks: [], notifyEnabled: true, lastSubmit: 'none', clockOffset: 90000,
        pause: { active: true, mode: 'resume' },
        events: [{ t: '10:01:30', tag: '演示', label: '示例事件保留（示例）· 暂停不删除事件' }],
        toast: null
      }
    },
    {
      id: 'S10', name: '系统可见性未确认',
      desc: '系统通知关闭/勿扰（示例）：提交结果为“可见性未确认”。',
      expect: '提交结果标“可见性未确认”；连接状态不变红、不变绿；事件留存。',
      apply: {
        connection: 'unverified', tasks: [], notifyEnabled: true, lastSubmit: 'visibility_unconfirmed', pause: null, toast: null, clockOffset: 140000,
        events: [{ t: '10:02:20', tag: '通知', label: '提交结果：可见性未确认（系统勿扰/关闭 · 示例）' }]
      }
    },
    {
      id: 'S11', name: '提交失败',
      desc: '示例 API 错误：通知提交失败（示例）。',
      expect: '提交结果标“提交失败”；连接状态不随之改变；事件留存。',
      apply: {
        connection: 'unverified', tasks: [], notifyEnabled: true, lastSubmit: 'submit_failed', pause: null, toast: null, clockOffset: 140000,
        events: [{ t: '10:02:20', tag: '通知', label: '提交失败：示例 API 错误（演示）' }]
      }
    },
    {
      id: 'S12', name: 'A/B 交错及旧代晚到',
      desc: '事件回放：SAMPLE-A 结束而 SAMPLE-B 仍工作；同一任务 SAMPLE-C 的 G1 取消后 G2 开始，旧代 G1 结束事件晚到。',
      expect: '由纯 reducer（taskId+generation 键控）回放、仅取每任务最新代为当前任务：当前 3 项 = A 已结束、B 工作中、C(G2) 工作中，摘要“工作中 2 · 已结束 1（演示）”；旧代 G1（已取消 · 结束事件晚到）仅入历史，不进当前任务列表，也不改变 G2。',
      apply: {
        connection: 'observable_demo', notifyEnabled: true, lastSubmit: 'none', pause: null, toast: null, clockOffset: 410000,
        taskEvents: [
          { taskId: 'SAMPLE-A', label: '示例任务 A', generation: 1, type: 'start' },
          { taskId: 'SAMPLE-B', label: '示例任务 B', generation: 1, type: 'start' },
          { taskId: 'SAMPLE-A', label: '示例任务 A', generation: 1, type: 'end' },
          { taskId: 'SAMPLE-C', label: '示例任务 C', generation: 1, type: 'start' },
          { taskId: 'SAMPLE-C', label: '示例任务 C', generation: 1, type: 'cancel' },
          { taskId: 'SAMPLE-C', label: '示例任务 C', generation: 2, type: 'start' },
          { taskId: 'SAMPLE-C', label: '示例任务 C', generation: 1, type: 'end' }
        ],
        tasks: null,
        events: [
          { t: '10:01:05', tag: '任务', label: '任务开始 · SAMPLE-A（示例）' },
          { t: '10:02:30', tag: '任务', label: '任务开始 · SAMPLE-B（示例）' },
          { t: '10:04:10', tag: '任务', label: '本轮结束 · SAMPLE-A（示例）· B 仍进行' },
          { t: '10:05:00', tag: '任务', label: '任务开始 · SAMPLE-C G1（示例）' },
          { t: '10:05:10', tag: '任务', label: '轮次取消 · SAMPLE-C G1（示例）' },
          { t: '10:05:20', tag: '任务', label: '轮次开始 · SAMPLE-C G2（示例）' },
          { t: '10:06:40', tag: '任务', label: '旧代结束到达 · SAMPLE-C G1（示例）· G2 仍进行' }
        ]
      }
    }
  ];

  var CONNECTION_TEXT = {
    unverified: '未验证（演示）· 未连接实际应用',
    absent: '应用缺失（示例）',
    conflict: '身份冲突（示例）',
    observable_demo: '可观察（演示）',
    disconnected: '已断连（示例）',
    unsupported_demo: '未支持（示例 · 该来源不支持演示观察）'
  };

  var CONNECTION_DOT = {
    unverified: 'dot-unknown',
    absent: 'dot-dim',
    conflict: 'dot-conflict',
    observable_demo: 'dot-ok',
    disconnected: 'dot-half',
    unsupported_demo: 'dot-half'
  };

  var SUBMIT_TEXT = {
    none: '尚未提交（演示）',
    submitted_demo: '已提交（演示）',
    visibility_unconfirmed: '可见性未确认（演示）',
    submit_failed: '提交失败（演示）'
  };

  global.WP003 = {
    CLOCK_BASE: CLOCK_BASE,
    QUOTA_SAMPLES: QUOTA_SAMPLES,
    CUSTOM_DEFAULTS: CUSTOM_DEFAULTS,
    TEMPLATES: TEMPLATES,
    TOOL_CONFIGS: TOOL_CONFIGS,
    DIAG_SAMPLE: DIAG_SAMPLE,
    SCENARIOS: SCENARIOS,
    CONNECTION_TEXT: CONNECTION_TEXT,
    CONNECTION_DOT: CONNECTION_DOT,
    TASK_TEXT: TASK_TEXT,
    TASK_DOT: TASK_DOT,
    SUBMIT_TEXT: SUBMIT_TEXT,
    QuotaBarDemoLogic: QuotaBarDemoLogic
  };
})(typeof window !== 'undefined' ? window : globalThis);
