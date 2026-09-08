(function () {
  'use strict';

  var C = window.WP003;

  function $(id) { return document.getElementById(id); }

  var els = {};
  var clockOffset = 0;
  var productActions = 0;
  var menuState = null;
  var toastProp = null;

  var state = {
    scenarioId: 'S01',
    view: 'settings',
    panelSection: 'main',
    settingsTab: 'conn',
    displayKind: 'quota',
    miniMode: false,
    connectionState: 'unverified',
    tasks: [],
    events: [],
    notifyEnabled: true,
    lastSubmit: 'none',
    pause: null,
    quotaIndex: 0,
    refreshCooldownUntil: 0,
    drafts: {
      custom: Object.assign({}, C.CUSTOM_DEFAULTS),
      cred: { mask: null, saved: false, verified: false },
      sources: { official: true, api: true, apiInstance: true },
      templates: { cn: { saved: false, verified: false, deleted: false }, intl: { saved: false, verified: false, deleted: false } }
    },
    selectedTool: null,
    tool: { generated: false, installed: false },
    opacity: 0.90,
    width: 300,
    autoStart: false,
    exited: false
  };

  function now() { return C.CLOCK_BASE + clockOffset; }

  function pad(n) { return n < 10 ? '0' + n : '' + n; }

  function fmtClock(ms) {
    var d = new Date(ms);
    return pad(d.getUTCHours()) + ':' + pad(d.getUTCMinutes()) + ':' + pad(d.getUTCSeconds());
  }

  function fmtCountdown(ms) {
    var s = Math.max(0, Math.floor(ms / 1000));
    return pad(Math.floor(s / 60)) + ':' + pad(s % 60);
  }

  function el(id) {
    if (!els[id]) { els[id] = $(id); }
    return els[id];
  }

  function setText(id, text) { el(id).textContent = text; }

  function setChip(id, text, cls) {
    var node = el(id);
    node.textContent = text;
    node.className = 'chip' + (cls ? ' ' + cls : '');
  }

  function bumpAction() {
    productActions += 1;
    setText('actionCount', String(productActions));
  }

  function appendEvent(tag, label) {
    state.events.unshift({ t: fmtClock(now()), tag: tag, label: label });
    if (state.events.length > 100) { state.events.length = 100; }
  }

  function quotaAvailable() {
    return !!(state.drafts.sources.official || state.drafts.sources.api);
  }

  function quotaSample() { return C.QUOTA_SAMPLES[state.quotaIndex % C.QUOTA_SAMPLES.length]; }

  function quotaText() {
    if (!quotaAvailable()) { return '未知（示例额度来源已停用）'; }
    var q = quotaSample();
    return q.pct + '% · ' + q.used + ' / ' + q.total + '（示例数据）';
  }

  function taskAggregate() {
    return C.QuotaBarDemoLogic.summarizeTasks(state.tasks);
  }

  function workText() {
    return C.QuotaBarDemoLogic.workSummaryLabel(state.tasks);
  }

  function barSummaryText() {
    if (state.displayKind === 'quota') {
      if (state.miniMode) {
        if (!quotaAvailable()) { return '额度未知（例）'; }
        return quotaSample().pct + '%（例）';
      }
      return quotaText();
    }
    if (state.miniMode) {
      var a = taskAggregate();
      if (a.total === 0 || (a.working + a.attention + a.ended + a.cancelled + a.failed) === 0) { return '任务未知（例）'; }
      var parts = [];
      if (a.attention) { parts.push('等' + a.attention); }
      if (a.working) { parts.push('工' + a.working); }
      if (a.ended) { parts.push('结' + a.ended); }
      if (a.cancelled) { parts.push('消' + a.cancelled); }
      if (a.failed) { parts.push('失' + a.failed); }
      return parts.join('·') + '（例）';
    }
    return workText();
  }

  function connChipText() { return C.CONNECTION_TEXT[state.connectionState]; }

  function pauseChipText() {
    if (!state.pause || !state.pause.active) { return '未暂停（演示）'; }
    if (state.pause.mode === 'resume') { return '暂停中 · 直到恢复（演示）'; }
    return '暂停中 · 剩余 ' + fmtCountdown(state.pause.until - now()) + '（演示）';
  }

  function renderBar() {
    var bar = el('hoverBar');
    bar.hidden = state.exited;
    bar.setAttribute('aria-expanded', String(state.view === 'panel' || !!menuState));
    el('barDot').className = 'dot ' + C.CONNECTION_DOT[state.connectionState];
    setText('barText', barSummaryText());
    var p = el('barPause');
    if (state.pause && state.pause.active) {
      p.hidden = false;
      p.textContent = state.pause.mode === 'resume' ? '暂停·直到恢复' : '暂停 ' + fmtCountdown(state.pause.until - now());
    } else { p.hidden = true; }
    el('barNotifOff').hidden = state.notifyEnabled;
    bar.classList.toggle('mini', state.miniMode);
    bar.style.opacity = String(state.opacity);
    bar.style.width = state.miniMode ? 'auto' : state.width + 'px';
  }

  function renderEventList(listNode, events) {
    listNode.textContent = '';
    if (events.length === 0) {
      var li = document.createElement('li');
      li.className = 'empty';
      li.textContent = '暂无演示事件（空态：不填充虚构时间）';
      listNode.appendChild(li);
      return;
    }
    events.forEach(function (ev) {
      var item = document.createElement('li');
      var t = document.createElement('span');
      t.className = 'e-time'; t.textContent = ev.t;
      var tag = document.createElement('span');
      tag.className = 'e-tag chip'; tag.textContent = ev.tag;
      var label = document.createElement('span');
      label.className = 'e-label'; label.textContent = ev.label;
      item.appendChild(t); item.appendChild(tag); item.appendChild(label);
      listNode.appendChild(item);
    });
  }

  function renderPanel() {
    var panel = el('panel');
    panel.hidden = !(state.view === 'panel' && !state.exited);
    if (panel.hidden) { return; }
    setChip('connChip', '连接边界：' + connChipText(), state.connectionState === 'observable_demo' ? 'demo-ok' : '');
    el('panelMain').hidden = state.panelSection !== 'main';
    el('panelRecords').hidden = state.panelSection !== 'records';
    setText('quotaTitle', state.displayKind === 'quota' ? '额度摘要（示例）' : '工作摘要（示例）');
    setText('quotaLine', state.displayKind === 'quota' ? quotaText() : workText());
    var tl = el('taskList');
    tl.hidden = state.displayKind !== 'work';
    tl.textContent = '';
    state.tasks.forEach(function (t) {
      var li = document.createElement('li');
      var dot = document.createElement('span');
      dot.className = 'dot ' + C.TASK_DOT[t.state];
      dot.setAttribute('aria-hidden', 'true');
      var id = document.createElement('span');
      id.className = 't-id'; id.textContent = t.id;
      var st = document.createElement('span');
      st.className = 'chip';
      st.textContent = t.state === 'needs_attention' && t.attention ? '等待' + t.attention + '（演示）' : C.TASK_TEXT[t.state];
      li.appendChild(dot); li.appendChild(id); li.appendChild(st);
      if (t.note) {
        var note = document.createElement('span');
        note.textContent = t.note;
        li.appendChild(note);
      }
      tl.appendChild(li);
    });
    var cooling = state.refreshCooldownUntil > now();
    var btnR = el('btnRefresh');
    btnR.disabled = cooling;
    btnR.textContent = cooling ? '冷却 ' + fmtCountdown(state.refreshCooldownUntil - now()) + '（演示）' : '刷新';
    el('btnPanelPause').textContent = state.pause && state.pause.active ? '恢复提醒' : '暂停提醒 ▾';
    renderEventList(el('recentList'), state.events.slice(0, 5));
    setText('recCount', state.events.length + '/100');
    renderEventList(el('recordList'), state.events);
  }

  function renderSettingsStatus() {
    var conn = connChipText();
    setText('connStatusMain', '状态：' + conn);
    el('connDot').className = 'dot ' + C.CONNECTION_DOT[state.connectionState];
    setChip('chipConn', conn, state.connectionState === 'observable_demo' ? 'demo-ok' : '');
    setChip('chipNotif', state.notifyEnabled ? '开启（演示）' : '已关闭（演示）', state.notifyEnabled ? 'demo-ok' : 'chip-off');
    setChip('chipPause', pauseChipText(), state.pause && state.pause.active ? 'chip-pause' : '');
    var subCls = state.lastSubmit === 'submit_failed' || state.lastSubmit === 'visibility_unconfirmed' ? 'demo-warn' : '';
    setChip('chipSubmit', C.SUBMIT_TEXT[state.lastSubmit], subCls);
    setChip('pauseStateChip', pauseChipText(), state.pause && state.pause.active ? 'chip-pause' : '');
    el('btnUnpause').hidden = !(state.pause && state.pause.active);
    setText('opacityOut', state.opacity.toFixed(2));
    el('miniToggle').checked = state.miniMode;
    setChip('miniChip', state.miniMode ? '开（演示）' : '关（演示）');
    setChip('autoStartChip', state.autoStart ? '开（仅演示）' : '关（仅演示）');
    setChip('notifMasterChip', state.notifyEnabled ? '开启（演示）' : '已关闭（演示）');
  }

  function renderSceneInfo() {
    var sc = C.SCENARIOS.filter(function (s) { return s.id === state.scenarioId; })[0];
    setText('sceneName', sc.id + ' · ' + sc.name);
    setText('sceneDesc', '说明：' + sc.desc);
    setText('sceneExpect', '预期：' + sc.expect);
  }

  function renderToolPanel() {
    el('btnGenConfig').disabled = !state.selectedTool;
    var ready = state.selectedTool && state.tool.generated;
    el('btnInstall').disabled = !ready || state.tool.installed;
    el('btnRemove').disabled = !ready || !state.tool.installed;
    el('configPreview').hidden = !ready;
    if (ready) {
      var cfg = C.TOOL_CONFIGS[state.selectedTool];
      setText('cfgPath', cfg.path);
      setText('cfgPre', cfg.content);
    }
  }

  function renderTemplates() {
    var host = el('tplList');
    host.textContent = '';
    Object.keys(C.TEMPLATES).forEach(function (key) {
      var tpl = C.TEMPLATES[key];
      var st = state.drafts.templates[key];
      var row = document.createElement('div');
      row.className = 'tpl-row';
      if (st.deleted) {
        var gone = document.createElement('span');
        gone.className = 'tpl-name';
        gone.textContent = tpl.name + ' · 已删除（演示，重置演示可恢复）';
        row.appendChild(gone);
        host.appendChild(row);
        return;
      }
      var name = document.createElement('span');
      name.className = 'tpl-name';
      name.textContent = tpl.name;
      row.appendChild(name);
      var chip = document.createElement('span');
      chip.className = 'chip';
      chip.textContent = st.verified ? '已验证（演示）' : st.saved ? '已保存（演示）' : '未保存';
      row.appendChild(chip);
      [['填入', 'fill'], ['保存', 'save'], ['保存验证', 'saveVerify'], ['删除', 'delete']].forEach(function (pair) {
        var b = document.createElement('button');
        b.type = 'button';
        b.className = 'btn';
        b.textContent = pair[0] === '保存验证' ? '保存验证' : pair[0] + '（演示）';
        b.setAttribute('data-tpl', key);
        b.setAttribute('data-act', pair[1]);
        row.appendChild(b);
      });
      host.appendChild(row);
    });
  }

  function renderAll() {
    renderBar();
    renderPanel();
    renderSettingsStatus();
    renderToolPanel();
    renderSceneInfo();
    var win = el('settingsWin');
    win.hidden = !(state.view === 'settings' && !state.exited);
  }

  function setView(view) {
    state.view = view;
    if (view !== 'panel') { state.panelSection = 'main'; }
    closeMenu(true);
    closePausePop(true);
    renderAll();
  }

  function togglePanel() {
    if (state.exited) { return; }
    setView(state.view === 'panel' ? 'bar' : 'panel');
    if (state.view === 'bar') { el('hoverBar').focus(); }
  }

  function openSettings(tab) {
    if (state.exited) { return; }
    state.view = 'settings';
    if (tab) { selectTab(tab, false); }
    closeMenu(true);
    closePausePop(true);
    renderAll();
    var current = el('tab-' + state.settingsTab);
    current.focus();
  }

  function closeSettings(returnFocus) {
    state.view = 'bar';
    state.panelSection = 'main';
    el('guideOverlay').hidden = true;
    el('aboutOverlay').hidden = true;
    renderAll();
    if (returnFocus) { el('hoverBar').focus(); }
  }

  function selectTab(tab, moveFocus) {
    state.settingsTab = tab;
    ['conn', 'remind', 'appearance'].forEach(function (name) {
      var btn = el('tab-' + name);
      var selected = name === tab;
      btn.setAttribute('aria-selected', String(selected));
      btn.tabIndex = selected ? 0 : -1;
      el('page-' + name).hidden = !selected;
    });
    if (moveFocus) { el('tab-' + tab).focus(); }
  }

  function openRecords() {
    state.panelSection = 'records';
    renderPanel();
    el('btnRecordsBack').focus();
  }

  function closeRecords() {
    state.panelSection = 'main';
    renderPanel();
    el('btnAllRecords').focus();
  }

  function showFeedback(text) {
    setText('panelFeedback', text);
  }

  function doRefresh() {
    if (state.refreshCooldownUntil > now()) { return; }
    state.quotaIndex += 1;
    state.refreshCooldownUntil = now() + 30000;
    if (quotaAvailable()) {
      appendEvent('额度', '刷新额度 · 示例数据（演示）· 未发起真实请求');
      showFeedback('已刷新（示例数据 · 演示）');
    } else {
      appendEvent('额度', '刷新额度 · 示例额度来源已停用，摘要保持未知（演示）');
      showFeedback('示例额度来源已停用：摘要保持未知（演示）');
    }
    renderPanel();
  }

  function openApp() {
    if (state.connectionState === 'absent') {
      appendEvent('演示', '打开 ChatGPT（演示）· 应用缺失场景仅提示');
      showFeedback('演示：此处将提示“未检测到应用（示例）”，不激活任何窗口');
    } else {
      appendEvent('演示', '打开 ChatGPT（演示）');
      showFeedback('演示：此处将返回应用（不激活任何窗口）');
    }
    renderPanel();
  }

  function setPause(mode) {
    if (mode === 'resume') {
      state.pause = null;
      appendEvent('提醒', '恢复提醒（演示）');
    } else if (mode === 'resume-until') {
      state.pause = { active: true, mode: 'resume', until: null };
      appendEvent('提醒', '提醒暂停：直到恢复（演示时钟）');
    } else {
      var ms = mode === '15m' ? 15 * 60000 : 60 * 60000;
      state.pause = { active: true, mode: mode, until: now() + ms };
      appendEvent('提醒', '提醒暂停：' + (mode === '15m' ? '15 分钟' : '1 小时') + '（演示时钟）');
    }
    renderAll();
    var pop = el('pausePop');
    var trigger = el('btnPanelPause');
    if (pop.hidden && trigger.offsetParent && (document.activeElement === document.body || pop.contains(document.activeElement))) {
      trigger.focus();
    }
  }

  function testNotification() {
    var out = el('testNotifResult');
    if (!state.notifyEnabled) {
      out.textContent = '提醒总开关已关闭：未产生演示提交事件（演示）。';
      out.className = 'result';
      return;
    }
    if (state.pause && state.pause.active) {
      out.textContent = '提醒暂停中：未产生演示提交事件（演示）。';
      out.className = 'result';
      return;
    }
    if (state.lastSubmit === 'visibility_unconfirmed') {
      appendEvent('通知', '测试通知 · 可见性未确认（系统勿扰/关闭 · 示例）');
      out.textContent = '已提交但系统可见性未确认（示例）；未显示演示通知；连接状态不变。';
      out.className = 'result err';
    } else if (state.lastSubmit === 'submit_failed') {
      appendEvent('通知', '测试通知 · 提交失败（示例 API 错误 · 演示）');
      out.textContent = '提交失败：示例 API 错误（演示）；连接状态不变。';
      out.className = 'result err';
    } else {
      state.lastSubmit = 'submitted_demo';
      appendEvent('通知', '测试通知 · 已提交系统通知（演示）');
      showToast({ title: '测试通知（演示）', body: '这是一条演示通知，非系统通知。' });
      out.textContent = '已提交系统通知并显示演示通知（示例）。';
      out.className = 'result';
    }
    renderSettingsStatus();
    renderPanel();
  }

  function showToast(prop) {
    toastProp = prop;
    setText('toastTitle', prop.title);
    setText('toastBody', prop.body);
    el('demoToast').hidden = false;
  }

  function hideToast() {
    toastProp = null;
    el('demoToast').hidden = true;
  }

  function applyScenario(id) {
    var sc = C.SCENARIOS.filter(function (s) { return s.id === id; })[0];
    var a = sc.apply;
    state.scenarioId = id;
    state.connectionState = a.connection;
    if (a.taskEvents) {
      state.tasks = C.QuotaBarDemoLogic.currentTasks(C.QuotaBarDemoLogic.reduceTaskEvents(a.taskEvents));
    } else {
      state.tasks = (a.tasks || []).map(function (t) { return Object.assign({}, t); });
    }
    state.events = a.events.slice().reverse().map(function (e) { return Object.assign({}, e); });
    state.notifyEnabled = a.notifyEnabled;
    el('notifMaster').checked = a.notifyEnabled;
    state.lastSubmit = a.lastSubmit;
    state.pause = a.pause ? { active: true, mode: a.pause.mode, until: a.pause.mode === 'resume' ? null : now() } : null;
    if (a.pause && a.pause.mode !== 'resume') { state.pause.until = C.CLOCK_BASE + a.clockOffset + (a.pause.mode === '15m' ? 900000 : 3600000); }
    clockOffset = a.clockOffset;
    state.quotaIndex = 0;
    state.refreshCooldownUntil = 0;
    hideToast();
    if (a.toast) { showToast(a.toast); }
    el('testNotifResult').textContent = '';
    setText('actionCount', String(productActions));
    renderAll();
  }

  function closeMenu(silent) {
    if (!menuState) { return; }
    var node = menuState.node;
    var trigger = el('hoverBar');
    menuState = null;
    node.parentNode.removeChild(node);
    el('hoverBar').setAttribute('aria-expanded', String(state.view === 'panel'));
    if (!silent) { trigger.focus(); }
  }

  function openMenu() {
    if (state.exited || menuState) { return; }
    var desktop = el('fakeDesktop');
    var bar = el('hoverBar');
    var menu = document.createElement('div');
    menu.className = 'ctx-menu';
    menu.setAttribute('role', 'menu');
    menu.setAttribute('aria-label', '悬浮条菜单（演示）');

    var btnDisplay = mkMenuItem('display', '显示模式', true);
    menu.appendChild(btnDisplay);
    var subDisplay = document.createElement('div');
    subDisplay.className = 'ctx-sub';
    subDisplay.setAttribute('role', 'menu');
    subDisplay.setAttribute('aria-label', '显示模式（演示）');
    subDisplay.hidden = true;
    var optQuota = mkCheckItem('quota', '额度摘要', state.displayKind === 'quota', 'menuitemradio');
    var optWork = mkCheckItem('work', '工作摘要', state.displayKind === 'work', 'menuitemradio');
    var optMini = mkCheckItem('mini', '迷你显示（尺寸开关）', state.miniMode, 'menuitemcheckbox');
    [optQuota, optWork, optMini].forEach(function (b) { subDisplay.appendChild(b); });
    menu.appendChild(subDisplay);

    var paused = state.pause && state.pause.active;
    var btnPause;
    if (paused) {
      btnPause = mkMenuItem('resume', '恢复提醒', false);
    } else {
      btnPause = mkMenuItem('pause', '暂停提醒', true);
    }
    menu.appendChild(btnPause);
    if (!paused) {
      var subPause = document.createElement('div');
      subPause.className = 'ctx-sub';
      subPause.setAttribute('role', 'menu');
      subPause.setAttribute('aria-label', '暂停时长（演示）');
      subPause.hidden = true;
      subPause.appendChild(mkCheckItem('15m', '15 分钟（演示）', false, 'menuitem'));
      subPause.appendChild(mkCheckItem('1h', '1 小时（演示）', false, 'menuitem'));
      subPause.appendChild(mkCheckItem('resume-until', '直到恢复（演示）', false, 'menuitem'));
      menu.appendChild(subPause);
    }

    menu.appendChild(mkMenuItem('settings', '设置', false));
    menu.appendChild(mkMenuItem('exit', '退出演示', false));

    desktop.appendChild(menu);
    menu.addEventListener('click', function (e) {
      var btn = e.target.closest('button');
      if (btn) { menuActivate(btn); }
    });
    var bRect = bar.getBoundingClientRect();
    var dRect = desktop.getBoundingClientRect();
    var left = Math.max(8, Math.min(bRect.left - dRect.left, dRect.width - menu.offsetWidth - 8));
    var top = bRect.bottom - dRect.top + 6;
    menu.style.left = left + 'px';
    menu.style.top = top + 'px';

    menuState = { node: menu, layer: 'main' };
    bar.setAttribute('aria-expanded', 'true');
    btnDisplay.focus();
  }

  function mkMenuItem(act, text, hasSub) {
    var b = document.createElement('button');
    b.type = 'button';
    b.setAttribute('role', 'menuitem');
    b.setAttribute('data-act', act);
    if (hasSub) {
      b.setAttribute('aria-haspopup', 'menu');
      b.setAttribute('aria-expanded', 'false');
      var arrow = document.createElement('span');
      arrow.className = 'ctx-arrow';
      arrow.setAttribute('aria-hidden', 'true');
      arrow.textContent = '▸';
      b.appendChild(document.createTextNode(text));
      b.appendChild(arrow);
    } else {
      b.textContent = text;
    }
    return b;
  }

  function mkCheckItem(value, text, checked, role) {
    var b = document.createElement('button');
    b.type = 'button';
    b.setAttribute('role', role);
    b.setAttribute('data-value', value);
    if (role !== 'menuitem') { b.setAttribute('aria-checked', String(checked)); }
    if (checked) {
      var mark = document.createElement('span');
      mark.className = 'check-mark';
      mark.textContent = '✓ ';
      b.appendChild(mark);
    }
    b.appendChild(document.createTextNode(text));
    return b;
  }

  function currentMenuItems(layer) {
    if (!menuState) { return []; }
    if (layer === 'main') {
      return Array.prototype.filter.call(menuState.node.children, function (n) { return n.tagName === 'BUTTON'; });
    }
    var subs = menuState.node.querySelectorAll('.ctx-sub');
    for (var i = 0; i < subs.length; i++) {
      if (!subs[i].hidden) { return Array.prototype.slice.call(subs[i].children); }
    }
    return [];
  }

  function openSubmenu(parentBtn) {
    var idx = Array.prototype.indexOf.call(menuState.node.children, parentBtn);
    var sub = menuState.node.children[idx + 1];
    if (!sub || sub.tagName !== 'DIV') { return; }
    sub.hidden = false;
    parentBtn.setAttribute('aria-expanded', 'true');
    menuState.layer = 'sub';
    sub.children[0].focus();
  }

  function closeSubmenu() {
    var subs = menuState.node.querySelectorAll('.ctx-sub:not([hidden])');
    for (var i = 0; i < subs.length; i++) {
      subs[i].hidden = true;
      var idx = Array.prototype.indexOf.call(menuState.node.children, subs[i]);
      var parent = menuState.node.children[idx - 1];
      if (parent) { parent.setAttribute('aria-expanded', 'false'); }
    }
    menuState.layer = 'main';
  }

  function menuKeydown(e) {
    if (!menuState) { return; }
    var items = currentMenuItems(menuState.layer);
    var focused = document.activeElement;
    var idx = items.indexOf(focused);
    var handled = true;
    if (e.key === 'ArrowDown') {
      items[(idx + 1 + items.length) % items.length].focus();
    } else if (e.key === 'ArrowUp') {
      items[(idx - 1 + items.length) % items.length].focus();
    } else if (e.key === 'Home') {
      items[0].focus();
    } else if (e.key === 'End') {
      items[items.length - 1].focus();
    } else if (e.key === 'ArrowRight') {
      if (menuState.layer === 'main' && focused && focused.getAttribute('aria-haspopup') === 'menu') { openSubmenu(focused); }
    } else if (e.key === 'ArrowLeft') {
      if (menuState.layer === 'sub') { closeSubmenuAndFocusParent(focused); }
    } else if (e.key === 'Escape') {
      if (menuState.layer === 'sub') { closeSubmenuAndFocusParent(focused); }
      else { closeMenu(false); }
    } else { handled = false; }
    if (handled) { e.preventDefault(); e.stopPropagation(); }
  }

  function visibleSubmenu() {
    if (!menuState) { return null; }
    var subs = menuState.node.querySelectorAll('.ctx-sub');
    for (var i = 0; i < subs.length; i++) {
      if (!subs[i].hidden) { return subs[i]; }
    }
    return null;
  }

  function submenuParent(sub) {
    var idx = Array.prototype.indexOf.call(menuState.node.children, sub);
    return idx > 0 ? menuState.node.children[idx - 1] : null;
  }

  function closeSubmenuAndFocusParent() {
    var sub = visibleSubmenu();
    var parent = sub ? submenuParent(sub) : null;
    if (sub) { sub.hidden = true; }
    if (parent) { parent.setAttribute('aria-expanded', 'false'); }
    menuState.layer = 'main';
    if (parent) { parent.focus(); }
  }

  function menuActivate(target) {
    if (target.parentElement && target.parentElement.classList.contains('ctx-sub')) {
      var value = target.getAttribute('data-value');
      if (target.parentElement.getAttribute('aria-label') === '显示模式（演示）') {
        if (value === 'mini') {
          state.miniMode = !state.miniMode;
          appendEvent('演示', '迷你显示 ' + (state.miniMode ? '开' : '关') + '（演示）');
        } else {
          state.displayKind = value === 'work' ? 'work' : 'quota';
          appendEvent('演示', '显示模式切换为' + (value === 'work' ? '工作摘要' : '额度摘要') + '（演示）');
        }
      } else if (value === 'resume-until') {
        setPause('resume-until');
      } else {
        setPause(value);
      }
      closeMenu(false);
      renderAll();
      return;
    }
    var act = target.getAttribute('data-act');
    if (act === 'display' || act === 'pause') {
      if (menuState.layer === 'sub') { closeSubmenu(); target.setAttribute('aria-expanded', 'false'); target.focus(); }
      else { openSubmenu(target); }
      return;
    }
    closeMenu(false);
    if (act === 'resume') { setPause('resume'); el('hoverBar').focus(); }
    else if (act === 'settings') { openSettings('conn'); }
    else if (act === 'exit') {
      state.exited = true;
      state.view = 'bar';
      hideToast();
      renderAll();
      el('stageMsg').hidden = false;
    }
  }

  function pausePopItems() {
    return [el('pp15'), el('pp1h'), el('ppResume')];
  }

  function openPausePop() {
    if (state.pause && state.pause.active) {
      setPause('resume');
      el('btnPanelPause').focus();
      return;
    }
    var pop = el('pausePop');
    var desktop = el('fakeDesktop');
    var btn = el('btnPanelPause');
    pop.hidden = false;
    var items = pausePopItems();
    items.forEach(function (b, i) { b.tabIndex = i === 0 ? 0 : -1; });
    var bRect = btn.getBoundingClientRect();
    var dRect = desktop.getBoundingClientRect();
    pop.style.left = Math.max(8, Math.min(bRect.left - dRect.left, dRect.width - pop.offsetWidth - 8)) + 'px';
    pop.style.top = (bRect.bottom - dRect.top + 4) + 'px';
    btn.setAttribute('aria-expanded', 'true');
    items[0].focus();
  }

  function closePausePop(silent) {
    var pop = el('pausePop');
    if (pop.hidden) { return; }
    pop.hidden = true;
    el('btnPanelPause').setAttribute('aria-expanded', 'false');
    if (!silent) { el('btnPanelPause').focus(); }
  }

  function tick() {
    clockOffset += 1000;
    if (state.pause && state.pause.active && state.pause.mode !== 'resume' && now() >= state.pause.until) {
      state.pause = null;
      appendEvent('提醒', '暂停结束（演示时钟）');
      renderAll();
      return;
    }
    renderBar();
    if (state.view === 'panel') { renderPanel(); }
    renderSettingsStatus();
  }

  function initFolds() {
    [['fhSources', 'fbSources'], ['fhCustom', 'fbCustom'], ['fhTools', 'fbTools'], ['fhDiag', 'fbDiag']].forEach(function (pair) {
      var head = el(pair[0]);
      var body = el(pair[1]);
      head.addEventListener('click', function () {
        var open = head.getAttribute('aria-expanded') === 'true';
        head.setAttribute('aria-expanded', String(!open));
        body.hidden = open;
      });
    });
  }

  function initTabs() {
    var names = ['conn', 'remind', 'appearance'];
    names.forEach(function (name) {
      el('tab-' + name).addEventListener('click', function () { selectTab(name, false); });
    });
    el('settingsWin').querySelector('.settings-nav').addEventListener('keydown', function (e) {
      var idx = names.indexOf(state.settingsTab);
      var handled = true;
      if (e.key === 'ArrowDown' || e.key === 'ArrowRight') { selectTab(names[(idx + 1) % 3], true); }
      else if (e.key === 'ArrowUp' || e.key === 'ArrowLeft') { selectTab(names[(idx + 2) % 3], true); }
      else if (e.key === 'Home') { selectTab(names[0], true); }
      else if (e.key === 'End') { selectTab(names[2], true); }
      else { handled = false; }
      if (handled) { e.preventDefault(); }
    });
  }

  function initSources() {
    [['srcOfficial', 'official', '官方额度页'], ['srcApi', 'api', 'API 平台'], ['srcApiInstance', 'apiInstance', '实例 1']].forEach(function (triple) {
      var input = el(triple[0]);
      input.addEventListener('change', function () {
        state.drafts.sources[triple[1]] = input.checked;
        setChip(triple[0] + 'Chip', input.checked ? '已启用（演示）' : '已停用（演示）');
        appendEvent('额度', '来源 ' + triple[2] + ' ' + (input.checked ? '启用' : '停用') + '（演示）');
      });
    });

    el('btnSampleCred').addEventListener('click', function () {
      state.drafts.cred.mask = 'sk-demo-••••••-SAMPLE';
      state.drafts.cred.saved = false;
      state.drafts.cred.verified = false;
      setChip('credMask', '示例凭据（固定掩码）：sk-demo-••••••-SAMPLE', 'demo-warn');
      el('credResult').textContent = '已载入示例凭据（固定掩码 · 演示），尚未保存。';
      appendEvent('凭据', '使用示例凭据（演示）');
    });
    el('btnCredSave').addEventListener('click', function () {
      var r = el('credResult');
      if (!state.drafts.cred.mask) { r.textContent = '请先点击“使用示例凭据”（演示）。'; r.className = 'result err'; return; }
      state.drafts.cred.saved = true;
      r.textContent = '已保存示例凭据（演示）· 不写入任何真实存储。';
      r.className = 'result';
      appendEvent('凭据', '保存示例凭据（演示）');
    });
    el('btnCredVerify').addEventListener('click', function () {
      var r = el('credResult');
      if (!state.drafts.cred.saved) { r.textContent = '请先保存示例凭据（演示）。'; r.className = 'result err'; return; }
      state.drafts.cred.verified = true;
      r.textContent = '验证通过（示例 · 演示）—— 模拟路径，未发起请求。';
      r.className = 'result';
      appendEvent('凭据', '验证示例凭据通过（演示）');
    });
    el('btnCredDelete').addEventListener('click', function () {
      if (!state.drafts.cred.mask && !state.drafts.cred.saved) {
        el('credResult').textContent = '没有可删除的示例凭据（演示）。';
        el('credResult').className = 'result err';
        return;
      }
      state.drafts.cred = { mask: null, saved: false, verified: false };
      setChip('credMask', '未保存示例凭据（演示）');
      el('credResult').textContent = '已删除示例凭据（演示）。';
      el('credResult').className = 'result';
      appendEvent('凭据', '删除示例凭据（演示）');
    });
  }

  function syncCustomForm() {
    var d = state.drafts.custom;
    el('cName').value = d.name;
    el('cPoll').value = String(d.poll);
    el('cEndpoint').value = d.endpoint;
    el('cHeader').value = d.header;
    el('cPrefix').value = d.prefix;
    el('cWindow').value = d.window;
    el('cInvert').checked = d.invert;
  }

  function readCustomForm() {
    state.drafts.custom = {
      name: el('cName').value.trim() || C.CUSTOM_DEFAULTS.name,
      poll: parseInt(el('cPoll').value, 10),
      endpoint: el('cEndpoint').value.trim() || C.CUSTOM_DEFAULTS.endpoint,
      header: el('cHeader').value.trim() || C.CUSTOM_DEFAULTS.header,
      prefix: el('cPrefix').value,
      window: el('cWindow').value.trim() || C.CUSTOM_DEFAULTS.window,
      invert: el('cInvert').checked
    };
  }

  function validateCustom() {
    var d = state.drafts.custom;
    if (isNaN(d.poll) || d.poll < 1 || d.poll > 1440) {
      return 'ERR_POLL_RANGE（演示）：轮询间隔需为 1–1440 分钟';
    }
    var seg = d.window.split('|').map(function (s) { return s.trim(); }).filter(function (s) { return s.length > 0; });
    if (seg.length < 3) {
      return 'ERR_WINDOW_MAPPING（演示）：需为“标签 | 已用字段 | 限额字段或字面量 [| 重置字段 [| invert]]”（点路径如 data.used）';
    }
    return null;
  }

  function initCustom() {
    syncCustomForm();
    ['cName', 'cPoll', 'cEndpoint', 'cHeader', 'cPrefix', 'cWindow', 'cInvert'].forEach(function (id) {
      el(id).addEventListener('input', readCustomForm);
      el(id).addEventListener('change', readCustomForm);
    });
    el('btnCustomSave').addEventListener('click', function () {
      readCustomForm();
      var r = el('customResult');
      var err = validateCustom();
      if (err) { r.textContent = err; r.className = 'result err'; return; }
      r.textContent = '已保存自定义来源（演示）：' + state.drafts.custom.name;
      r.className = 'result';
      appendEvent('额度', '保存自定义来源（演示）');
    });
    el('btnCustomSaveVerify').addEventListener('click', function () {
      readCustomForm();
      var r = el('customResult');
      var err = validateCustom();
      if (err) { r.textContent = err; r.className = 'result err'; return; }
      r.textContent = '已保存并验证（示例验证通过 · 演示）—— 未发起真实请求。';
      r.className = 'result';
      appendEvent('额度', '保存并验证自定义来源（演示）');
    });
    el('btnCustomDelete').addEventListener('click', function () {
      state.drafts.custom = Object.assign({}, C.CUSTOM_DEFAULTS);
      syncCustomForm();
      var r = el('customResult');
      r.textContent = '已删除（演示）· 已恢复示例初值。';
      r.className = 'result';
      appendEvent('额度', '删除自定义来源（演示）');
    });
  }

  function initTemplates() {
    el('tplList').addEventListener('click', function (e) {
      var btn = e.target.closest('button[data-tpl]');
      if (!btn) { return; }
      var key = btn.getAttribute('data-tpl');
      var act = btn.getAttribute('data-act');
      var st = state.drafts.templates[key];
      if (act === 'fill') {
        state.drafts.custom = Object.assign({}, C.TEMPLATES[key].values);
        syncCustomForm();
        appendEvent('额度', '填入模板 ' + C.TEMPLATES[key].name + '（演示）');
      } else if (act === 'save') {
        st.saved = true; st.verified = false;
        appendEvent('额度', '保存模板 ' + C.TEMPLATES[key].name + '（演示）');
      } else if (act === 'saveVerify') {
        st.saved = true; st.verified = true;
        appendEvent('额度', '保存并验证模板 ' + C.TEMPLATES[key].name + '（演示）');
      } else if (act === 'delete') {
        st.deleted = true;
        appendEvent('额度', '删除模板 ' + C.TEMPLATES[key].name + '（演示）');
      }
      renderTemplates();
    });
  }

  function initTools() {
    var radios = document.querySelectorAll('input[name="tool"]');
    Array.prototype.forEach.call(radios, function (r) {
      r.addEventListener('change', function () {
        state.selectedTool = r.value;
        state.tool = { generated: false, installed: false };
        el('toolCheckResult').textContent = '';
        appendEvent('配置', '已切换工具（演示）：' + C.TOOL_CONFIGS[r.value].name + ' · 旧配置与旧结果已清除');
        renderToolPanel();
      });
    });
    el('btnGenConfig').addEventListener('click', function () {
      if (!state.selectedTool) { return; }
      state.tool = { generated: true, installed: false };
      el('toolCheckResult').textContent = '';
      appendEvent('配置', '生成配置（演示）：' + C.TOOL_CONFIGS[state.selectedTool].name + ' · 虚构路径');
      renderToolPanel();
    });
    el('btnInstall').addEventListener('click', function () {
      state.tool.installed = true;
      el('toolCheckResult').textContent = '示例配置匹配（演示）—— 不代表已登录、已信任或已连接。';
      appendEvent('配置', '模拟安装（演示）：' + C.TOOL_CONFIGS[state.selectedTool].name);
      renderToolPanel();
    });
    el('btnRemove').addEventListener('click', function () {
      state.tool = { generated: false, installed: false };
      el('toolCheckResult').textContent = '已移除示例配置（演示）。';
      appendEvent('配置', '移除配置（演示）：' + C.TOOL_CONFIGS[state.selectedTool].name);
      renderToolPanel();
    });
  }

  function initDiag() {
    el('btnDiagPreview').addEventListener('click', function () {
      var pre = el('diagPre');
      pre.hidden = !pre.hidden;
      if (!pre.hidden) { pre.textContent = C.DIAG_SAMPLE; }
    });
    el('btnDiagCopy').addEventListener('click', function () {
      el('diagResult').textContent = '已复制（演示）—— 实际未写入剪贴板。';
      appendEvent('诊断', '诊断信息复制（演示 · 未写剪贴板）');
    });
  }

  function initRemind() {
    el('notifMaster').addEventListener('change', function () {
      state.notifyEnabled = el('notifMaster').checked;
      appendEvent('提醒', '提醒总开关 ' + (state.notifyEnabled ? '开启' : '关闭') + '（演示）· 最近事件保留');
      renderAll();
    });
    el('btnTestNotif').addEventListener('click', testNotification);
    el('p15').addEventListener('click', function () { setPause('15m'); });
    el('p1h').addEventListener('click', function () { setPause('1h'); });
    el('pResume').addEventListener('click', function () { setPause('resume-until'); });
    el('btnUnpause').addEventListener('click', function () { setPause('resume'); });
  }

  function initAppearance() {
    el('opacityRange').addEventListener('input', function () {
      state.opacity = parseFloat(el('opacityRange').value);
      renderBar();
      renderSettingsStatus();
    });
    Array.prototype.forEach.call(document.querySelectorAll('input[name="barwidth"]'), function (r) {
      r.addEventListener('change', function () {
        state.width = parseInt(r.value, 10);
        appendEvent('演示', '悬浮条宽度 ' + state.width + 'px（演示）');
        renderBar();
      });
    });
    el('miniToggle').addEventListener('change', function () {
      state.miniMode = el('miniToggle').checked;
      renderBar();
      renderSettingsStatus();
    });
    el('autoStartToggle').addEventListener('change', function () {
      state.autoStart = el('autoStartToggle').checked;
      renderSettingsStatus();
    });
  }

  function initAboutGuide() {
    el('btnAccessGuide').addEventListener('click', function () {
      el('guideOverlay').hidden = false;
      el('guideBack').focus();
    });
    el('guideBack').addEventListener('click', function () {
      el('guideOverlay').hidden = true;
      el('btnAccessGuide').focus();
    });
    el('aboutBtn').addEventListener('click', function () {
      el('aboutOverlay').hidden = false;
      el('aboutBack').focus();
    });
    el('aboutBack').addEventListener('click', function () {
      el('aboutOverlay').hidden = true;
      el('aboutBtn').focus();
    });
    el('btnUpdate').addEventListener('click', function () {
      el('aboutResult').textContent = '演示：当前为示例版本 0.0.0-prototype，无更新通道，不访问网络。';
      appendEvent('演示', '关于与更新 · 检查更新（演示）');
    });
    el('btnReport').addEventListener('click', function () {
      el('aboutResult').textContent = '演示：此处仅记录示例反馈，不打开浏览器、不发送数据。';
      appendEvent('演示', '关于与更新 · 报告问题（演示）');
    });
    el('btnSponsor').addEventListener('click', function () {
      el('aboutResult').textContent = '演示：感谢支持，此处不打开任何外部页面。';
      appendEvent('演示', '关于与更新 · 赞助（演示）');
    });
  }

  function initPanelAndBar() {
    var bar = el('hoverBar');
    bar.addEventListener('click', function () {
      if (state.view === 'settings') { state.view = 'bar'; }
      togglePanel();
    });
    bar.addEventListener('keydown', function (e) {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        bar.click();
      } else if (e.key === 'ContextMenu' || (e.key === 'F10' && e.shiftKey)) {
        e.preventDefault();
        bumpAction();
        openMenu();
      }
    });
    bar.addEventListener('contextmenu', function (e) {
      e.preventDefault();
      bumpAction();
      openMenu();
    });
    el('panelClose').addEventListener('click', function () { setView('bar'); el('hoverBar').focus(); });
    el('btnRefresh').addEventListener('click', doRefresh);
    el('btnOpenApp').addEventListener('click', openApp);
    el('btnPanelPause').addEventListener('click', openPausePop);
    el('btnAllRecords').addEventListener('click', openRecords);
    el('btnRecordsBack').addEventListener('click', closeRecords);
    el('pp15').addEventListener('click', function () { closePausePop(true); setPause('15m'); });
    el('pp1h').addEventListener('click', function () { closePausePop(true); setPause('1h'); });
    el('ppResume').addEventListener('click', function () { closePausePop(true); setPause('resume-until'); });
    el('pausePop').addEventListener('keydown', function (e) {
      var items = pausePopItems();
      var idx = items.indexOf(document.activeElement);
      if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        closePausePop(false);
        return;
      }
      if (e.key === 'Tab') {
        closePausePop(false);
        return;
      }
      if (e.key === 'ArrowDown') { e.preventDefault(); items[(idx + 1) % items.length].focus(); }
      else if (e.key === 'ArrowUp') { e.preventDefault(); items[(idx - 1 + items.length) % items.length].focus(); }
      else if (e.key === 'Home') { e.preventDefault(); items[0].focus(); }
      else if (e.key === 'End') { e.preventDefault(); items[items.length - 1].focus(); }
    });
    el('pausePop').addEventListener('focusin', function (e) {
      pausePopItems().forEach(function (b) { b.tabIndex = b === e.target ? 0 : -1; });
    });
    el('toastClose').addEventListener('click', hideToast);
    el('toastDismiss').addEventListener('click', hideToast);
    el('toastView').addEventListener('click', function () {
      hideToast();
      setView('panel');
      showFeedback('演示：由演示通知打开面板（非系统通知跳转）。');
    });
    el('settingsClose').addEventListener('click', function () { closeSettings(false); el('hoverBar').focus(); });
    el('btnReenter').addEventListener('click', function () {
      state.exited = false;
      state.view = 'bar';
      el('stageMsg').hidden = true;
      renderAll();
      el('hoverBar').focus();
    });
  }

  function initMenuEvents() {
    document.addEventListener('click', function (e) {
      if (menuState && !menuState.node.contains(e.target) && !el('hoverBar').contains(e.target)) {
        closeMenu(true);
      }
      if (!el('pausePop').hidden && !el('pausePop').contains(e.target) && e.target !== el('btnPanelPause')) {
        closePausePop(true);
      }
    });
    document.addEventListener('contextmenu', function (e) {
      if (menuState && !menuState.node.contains(e.target) && !el('hoverBar').contains(e.target)) {
        closeMenu(true);
      }
    });
  }

  function initEsc() {
    document.addEventListener('keydown', function (e) {
      if (e.key !== 'Escape') { return; }
      if (menuState) {
        e.preventDefault();
        if (menuState.layer === 'sub') { closeSubmenuAndFocusParent(document.activeElement); }
        else { closeMenu(false); }
        return;
      }
      if (!el('pausePop').hidden) { e.preventDefault(); closePausePop(false); return; }
      if (!el('aboutOverlay').hidden) { e.preventDefault(); el('aboutOverlay').hidden = true; el('aboutBtn').focus(); return; }
      if (!el('guideOverlay').hidden) { e.preventDefault(); el('guideOverlay').hidden = true; el('btnAccessGuide').focus(); return; }
      if (state.view === 'panel' && state.panelSection === 'records') { e.preventDefault(); closeRecords(); return; }
      if (state.view === 'settings') { e.preventDefault(); closeSettings(false); el('hoverBar').focus(); return; }
      if (state.view === 'panel') { e.preventDefault(); setView('bar'); el('hoverBar').focus(); return; }
    });
  }

  function initCounter() {
    el('productStage').addEventListener('click', function (e) {
      if (e.target.closest('button, input, select, [role="button"]')) { bumpAction(); }
    });
    el('btnCountReset').addEventListener('click', function () {
      productActions = 0;
      setText('actionCount', '0');
    });
  }

  function initFramework() {
    var select = el('sceneSelect');
    C.SCENARIOS.forEach(function (sc) {
      var opt = document.createElement('option');
      opt.value = sc.id;
      opt.textContent = sc.id + ' · ' + sc.name;
      select.appendChild(opt);
    });
    select.addEventListener('change', function () { applyScenario(select.value); });
    el('fxBar').addEventListener('click', function () { if (!state.exited) { hideToast(); setView('bar'); } });
    el('fxPanel').addEventListener('click', function () { if (!state.exited) { setView('panel'); } });
    el('fxSettings').addEventListener('click', function () { if (!state.exited) { openSettings(state.settingsTab); } });
    el('btnFastClock').addEventListener('click', function () {
      clockOffset += 60000;
      if (state.pause && state.pause.active && state.pause.mode !== 'resume' && now() >= state.pause.until) {
        state.pause = null;
        appendEvent('提醒', '暂停结束（演示时钟 · 快进）');
      }
      renderAll();
    });
    el('btnResetDemo').addEventListener('click', function () { window.location.reload(); });
  }

  function init() {
    initFolds();
    initTabs();
    initSources();
    initCustom();
    initTemplates();
    initTools();
    initDiag();
    initRemind();
    initAppearance();
    initAboutGuide();
    initPanelAndBar();
    initMenuEvents();
    initEsc();
    initCounter();
    initFramework();
    document.addEventListener('keydown', menuKeydown, true);
    renderTemplates();
    applyScenario('S01');
    selectTab('conn', false);
    renderAll();
    window.setInterval(tick, 1000);
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
