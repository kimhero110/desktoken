# WP-003 原型验收记录（acceptance）

日期：2026-09-08。实现代理：OpenCode/GLM。本文件仅记录实际执行过的静态自检；所有浏览器交互与用户走查均 **not_run**，不得因自动化或静态检查代签。PUI 检查项定义见 reference/ACCEPTANCE.md。

## 1. 交付物清单

- prototypes/chatgpt-desktop/index.html —— 演示框架 + 产品画框（悬浮条 / 面板 / 演示通知 / 设置窗口 640×520 三页签 + 页脚关于子面板 + 接入说明子面板）。
- prototypes/chatgpt-desktop/prototype.css —— 紧凑低饱和深色主题（主文 14px / 辅助 12px / 8px 间距 / 控件 ≥28px / 状态非仅颜色）。
- prototypes/chatgpt-desktop/scenarios.js —— 12 个固定场景、合成时钟基准、额度样例、模板/工具虚构配置、脱敏诊断样例（defer 普通脚本）。
- prototypes/chatgpt-desktop/prototype.js —— 内存状态机、渲染与全部交互（defer 普通脚本）。
- prototypes/chatgpt-desktop/README.md —— 启动 / 边界 / 走查 / 14 组迁移映射 / 键盘说明。
- prototypes/chatgpt-desktop/acceptance.md —— 本文件。

## 2. 静态自检（已执行：人工通读源码 + 检索禁止调用）

结果：**通过（仅静态层面）**。逐项：

- 网络类：无 fetch / WebSocket / EventSource / sendBeacon / XMLHttpRequest / navigator.sendBeacon / 外链（无任何 http(s) 导航目标；端点字符串仅为 `.invalid` 合成样例，永不请求）。
- IPC：无 tauri / invoke / __TAURI__。
- 存储：无 localStorage / sessionStorage / indexedDB / cookie 写入 / document.cookie。
- 剪贴板 / 通知 / 权限：无 clipboard / Notification / Notification.requestPermission / navigator.permissions。
- CSP：index.html 含计划指定的完整 CSP meta；无内联 script / 内联 style 属性 / 内联事件处理器；全部按钮 type="button"（无 form，无提交）。
- 持久化：状态仅存在于内存对象；重置演示 = location.reload()；刷新恢复 S01 初始样例。
- 页面标题无用户内容；输入框均为合成示例字段并带“请勿输入真实数据”提示；错误仅固定码（ERR_POLL_RANGE）与预设文本。
- 时钟：合成基准 10:00:00 + 内存偏移（含快进），暂停 / 冷却逻辑仅依赖合成时钟；不读取系统时间参与逻辑。

## 3. 14 组迁移映射（原位置 → 原型入口 → 模拟动作 → 结果）

| 组 | 原位置 | 原型入口 | 模拟动作 | 结果（静态核对） |
|---|---|---|---|---|
| 1 刷新额度 | 旧菜单刷新 | 面板额度区“刷新” | 刷新示例数据，30s 冷却禁用 | 已实现，含冷却倒计时文案；不发真实请求 ✓ |
| 2 迷你 | 旧显示设置 | 右键菜单→显示模式→迷你显示；外观页开关 | 尺寸开关 | 与摘要类型分别建模，可组合 ✓ |
| 3 诊断 | 旧设置诊断 | 连接页·诊断折叠区 | 预览脱敏样例 / 复制（演示） | 复制不写剪贴板，仅提示 ✓ |
| 4 更新 | 旧关于 | 页脚关于与更新→检查更新 | 演示反馈 | 仅文案，无网络 ✓ |
| 5 报告 | 旧关于 | 页脚关于与更新→报告问题 | 演示反馈 | 仅文案，不开浏览器 ✓ |
| 6 赞助 | 旧关于 | 页脚关于与更新→赞助 | 演示反馈 | 仅文案 ✓ |
| 7 设置/退出 | 旧菜单 | 右键菜单第 3/4 项 | 打开设置 / 退出演示 | 退出隐藏悬浮条，画框下方可重新进入 ✓ |
| 8 不透明度/宽度 | 旧外观 | 外观页滑块 + 7 值宽度 | 0.60–1.00 步长 0.02；240–360 七值 | 仅影响原型内悬浮条 ✓ |
| 9 开机启动 | 旧通用 | 外观页开关 | 仅演示 | 不改系统配置，标注“仅演示” ✓ |
| 10 平台/实例/凭据 | 旧额度来源 | 额度来源折叠区 | 来源/实例启用开关；示例凭据载入/保存/验证/删除 | 真实密钥输入禁用 + 固定掩码 ✓ |
| 11 自定义/模板 | 旧高级来源 | 高级自定义展开编辑 + Moonshot 国内/国际 | 名称/轮询 1–1440/端点 .invalid/认证头名称/前缀 `Bearer `/竖线窗口映射/invert；保存/保存验证/删除；模板填入/保存/保存验证/删除 | R1 修正字段语义；固定码 ERR_POLL_RANGE / ERR_WINDOW_MAPPING；切页签草稿保留（代码层，未执行） |
| 12 CLI 接入 | 旧已有工具 | 已有工具折叠区 | 三工具独立选择；生成→安装/移除 | 切换工具清除配置与结果并禁用操作；虚构路径；结果只称“示例配置匹配” ✓ |
| 13 任务提醒 | 旧提醒 | 提醒页 | 总开关 / 测试通知 / 暂停 15m/1h/直到恢复 | 与连接状态分列显示 ✓ |
| 14 检查/测试通知 | 旧检查按钮 | 提醒页测试通知 + 连接页“查看接入说明” | 无假装真实扫描的检查按钮 | 提交结果四态：尚未提交/已提交/可见性未确认/提交失败 ✓ |

注：以上“结果”为源码级核对，未在浏览器实际操作。

## 4. PUI 预声明检查状态（R1 修订后）

R1 说明：初版对部分项标记的 static_pass 已撤回——涉及额度语义（PUI-05/11）、S12 并发聚合（PUI-06，原为硬编码终态，现已改为纯 reducer 回放但仅经源码审查）、自定义字段语义（PUI-02 第 11 组）与 CSS 命中目标（PUI-09 静态几何部分）。下表为修订后的诚实状态；所有 browser/user 项仍为 not_run。

| ID | 状态 | 说明 |
|---|---|---|
| PUI-01 原型隔离 | static_pass / browser not_run | 静态检索 0 违禁调用（见 §2）；浏览器 5 流程 0 外网请求未验证 |
| PUI-02 入口迁移 | source_reviewed / browser not_run | 14/14 组源码级核对（§3）；第 11 组字段语义按 R1 修正；逐项实际操作 not_run |
| PUI-03 菜单层级 | static_pass / browser not_run | 3 页签、4 右键主项、子层 1；摘要与迷你非互斥；“关于”为页脚子面板非第四页 |
| PUI-04 五流程 | not_run | 计数器已提供；代理与用户走查均未执行 |
| PUI-05 状态诚实 | source_reviewed / browser not_run | R1 后：额度与连接解耦、断连不推断失败、12 场景标记；仅源码审查 |
| PUI-06 并发/旧代 | source_reviewed / browser not_run | R2 后：reducer 增加 currentTasks/historyTasks 分离，同任务多代仅最新代为当前；S12 改用同一 taskId SAMPLE-C（G1 start→cancel→G2 start→晚到 G1 end），当前 3 项 = A 已结束、B 工作中、C(G2) 工作中。纯逻辑复测与浏览器重放 not_run |
| PUI-07 提醒解耦 | static_pass / browser not_run | 关闭/暂停不产生提交事件、事件留存；失败态不改连接；四态分列 |
| PUI-08 键盘/焦点 | static_pass / browser not_run | 页签方向键/Home/End、折叠 aria-expanded、菜单左右键/Esc 逐层、各弹层焦点返回均在代码中；实际键盘遍历 not_run |
| PUI-09 几何/视觉 | not_run | CSS 命中目标已按 R1 提至 ≥28px（源码层），渲染几何/缩放/截图全部 not_run |
| PUI-10 草稿/重置 | static_pass / browser not_run | 草稿仅存内存；切页签保留（DOM 不重建）；重置=reload 恢复初值；实际操作 not_run |
| PUI-11 额度旧语义 | source_reviewed / browser not_run | R1 后：额度来源独立于连接；冷却禁用、宽度 7 值、透明度边界、迷你组合均在；无真实额度请求 |
| PUI-12 发布边界 | static_pass | 本包仅新增/修改 prototypes/chatgpt-desktop/ 六文件；未触碰 src/、src-tauri/、integrations/、reference/；README 明确演示；无自动授权入口 |

## 5. QA-R1 修正记录（2026-09-08，仅源码层）

1. S08：断连不再等价 failed_demo；任务显示未知（历史仅保留），事件文案改为“不推断失败/完成”。`quotaReliable()`（额度可用性挂钩 ChatGPT 连接）已删除，改为 `quotaAvailable()`（取决于示例额度来源开关）；额度摘要与连接边界解耦。显式 failed_demo 仅允许出现在可靠 observable_demo 来源下的任务错误（当前无场景使用，枚举保留）。
2. 迷你工作摘要：优先显示等待确认（等 N），其次工/结/消/失；全部未知/不可靠时保持“任务未知（例）”。
3. S12：新增 `QuotaBarDemoLogic`（scenarios.js，挂载于 window/globalThis，纯函数、无 DOM/网络/存储）：`reduceTaskEvents`（taskId+generation 键控回放 start/end/cancel/await_auth/await_input/error，晚到事件只落在本键上）、`summarizeTasks`、`workSummaryLabel`。S12 场景改由 reducer 输出生成任务列表与摘要，不再硬编码终态。Node 逻辑测试：not_run（主代理可在授权环境执行）。
4. 自定义/模板字段语义修正：认证头=头名称（Authorization）；前缀=含尾随空格的 `Bearer `；窗口映射=竖线分隔 `标签 | 已用路径 | 限额路径或字面量 | 重置路径(可选) | invert(可选)`（文本输入），新增固定错误码 ERR_WINDOW_MAPPING；端点保持 .invalid 合成样例。
5. CSS 命中目标：`.hoverbar.mini`、`.tpl-row .btn`、框架按钮 min-height 提至 ≥28px（源码层；渲染几何未验证）。
6. 本表与 README 同步修订；reference 计划文档未改动。以上均为源码审查结论，无任何浏览器/视觉/用户测试被执行。

## 6. QA-R2 修正记录（2026-09-08，仅源码层；依据 qa-r1-pure-results.json 纯逻辑复测失败项）

1. 当前任务/历史分离：`QuotaBarDemoLogic` 新增 `splitCurrentAndHistory` / `currentTasks` / `historyTasks`；同任务多代时当前列表仅含最新一代。原纯逻辑失败用例（同 taskId SAMPLE-A，gen1 晚到 end 与 gen2 working 并存为两条“当前”）在源码层修复；`applyScenario` 改用 `currentTasks(...)`。纯逻辑复测：**not_run**（主代理可在授权环境重跑）。
2. S12 去空泛化：G1/G2 改为同一 taskId SAMPLE-C 的两代（G1 start→cancel→G2 start→晚到 G1 end）；预期当前 3 项 = A 已结束、B 工作中、C(G2) 工作中，摘要“工作中 2 · 已结束 1（演示）”；旧代 G1（已取消 · 结束事件晚到）仅入历史与事件记录。
3. `readCustomForm` 前缀不再 `.trim()`：`Bearer ` 的尾随空格原样保留（其余字段仍去首尾空白）。
4. 窗口映射示例改用点路径：默认 `月额度 | data.used | data.limit | data.reset`，模板同步；标签与 ERR_WINDOW_MAPPING 文案注明点路径语法；不再是 `/demo/used` 斜杠式。
5. `unsupported_demo` 连接文案由“演示性支持（示例）”改为“未支持（示例 · 该来源不支持演示观察）”。
6. 浏览器/视觉/用户测试维持 blocked/not_run；未尝试任何浏览器或 shell 绕过；reference 计划文档未改动。

## 7. QA-R3 修正记录（2026-09-08，仅源码层；四项独立源码审查发现）

1. 场景事件顺序：`applyScenario` 现将时序 fixture 以 `slice().reverse()` 归一为最新在前（不改写 fixture 本体，reducer 回放仍按时序）；S12 最近 5 条 = 晚到 G1 结束、G2 开始、G1 取消、G1 开始、A 结束（不含最旧两条：B 开始、A 开始）。
2. 子菜单焦点：`closeSubmenuAndFocusParent` 先捕获可见子层及其父项再隐藏，Esc / 左方向键正确回焦父项并保持 aria-expanded 准确（原实现先隐藏后查找，父项永不回焦）。
3. 初始视图：`state.view` 初始为 settings（连接页），刷新/重置演示后同样落位；悬浮条默认显示额度摘要不变；场景切换保持导航状态。
4. 控件同步：`renderSettingsStatus` 每次渲染将 `miniToggle.checked` 同步为 `state.miniMode`，右键菜单与外观页两个入口反映并切换同一值。
5. 以上均为源码级验证；浏览器/键盘/视觉执行与用户走查维持 not_run/pending，不主张总体验收；reference 计划与验收文档未改动。

## 8. VISUAL-R4 视觉修订记录（2026-09-08，仅源码层，用户授权）

1. 采用产品视觉基线：Segoe UI Variable Text / Segoe UI 字体栈；背景 #1b1b20、卡片 #22222a、控件 #2a2a32、主色 #2d4f8f；控件圆角 4px、面板圆角 8px；白色低透明度边框。保留 14px/12px 字号、≥28px 命中目标、≥4.5:1 文本对比（未沿用旧版低对比标签色）。
2. 产品突出：评审控件与场景说明移入原生 `<details>`（默认收起、画框外、全部 ID/功能/可访问性保留）；演示横幅“交互原型 · 使用示例数据 · 不连接实际应用”始终可见；页面主标题精简。
3. 去掉多余外框/嵌套卡片描边，改为分隔线+紧凑行；关于/接入说明子面板改为内容尺寸浮层（不再全高近空面板），返回焦点与“非第四页签”保持；640×520 设置框架与响应式滚动保留；外层演示桌面取消强制大最小高度。
4. 对比度与隐藏语义：显式 `color-scheme: dark`；新增全局 `[hidden]{display:none!important}`——此前 `.overlay{display:flex}` 等作者规则会覆盖 hidden 属性造成“幽灵内容”混显（源码审查发现，正是截图病灶）；补齐关于/接入说明标题与正文用色。
5. 保留计划要求的（演示）动作后缀、样例标识与状态诚实文案；仅删减冗余标题/说明（stage 标签、页脚合并、主标题精简），演示边界未删。
6. R3 行为修复未动（初始设置视图、最新在前事件、子菜单回焦、迷你同步），本次仅 HTML/CSS。
7. 本节为源码级修订说明；实际视觉验收仍 pending，浏览器/键盘/视觉/用户走查维持 not_run。

### 8.1 R4 几何回归修正（源码 QA 后续，仅布局，浏览器 not_run）

- 回归：`.fake-desktop` 最小高度 200px 且子元素全绝对定位时，`.settings-win` 的 `top:50% + translateY(-50%)` 使其顶部高出画框上方约 160px，遮挡评审横幅并有出屏风险。
- 修正：设置窗口可见时改为常规文档流（`position:relative; margin:56px auto 16px`，保留 640×520 框架与水平居中，顶部 56px 为悬浮条让位）；`[hidden]` 使其不占位，故仅悬浮条模式下桌面保持紧凑（最小高度 96px）；面板可见时以 `.fake-desktop:has(#panel:not([hidden]))` 预留 552px（悬浮条间距+480px 面板），不支持 `:has()` 的环境退化为可见溢出（可滚动、不裁切）。关于/接入说明子面板宽度改为 `min(400px, 父级可用宽度)`，避免收缩过窄。
- R3 行为与全局 `[hidden]` 守卫未动；本次仅 CSS。实际几何/渲染验证 not_run。
- 追加：`.fake-desktop` 增加 `display:flow-root` 建立 BFC，包住 `.settings-win` 的首个流内上边距，防止 margin 穿透父级再次让绝对定位悬浮条压到设置窗口；未用 `overflow:hidden`（菜单/弹层需可见溢出），其余规则不变。

### 8.2 U5 暂停弹层可用性修正（仅源码，键盘/浏览器/UA 仍 pending）

1. 焦点丢失：`pp15/pp1h/ppResume` 点击后弹层已隐藏而焦点残留在隐藏按钮上；`setPause` 现在在 `renderAll()` 之后检测该情形（弹层隐藏且焦点为 body 或仍在其内、触发按钮可见）并回焦 `btnPanelPause`。设置页/菜单路径的 `setPause` 调用焦点不在弹层内，不受影响，无双重处理。
2. Esc 冒泡：弹层自身 keydown 处理 Escape 后 `stopPropagation()`，恰好关闭一层（弹层→回焦触发按钮）；文档级 `initEsc` 的弹层分支保留为焦点在弹层外时的后备，两者互斥。
3. 菜单键盘语义：三个时长项实现 roving tabindex（单表位点，focusin 同步）；ArrowUp/Down 循环、Home/End 跳转；Enter/Space 原生激活（保留 2 击暂停 / 1 击恢复与鼠标行为）；Tab 关闭弹层并按自然顺序离开，无陷阱；打开即聚焦首项。
4. 双重处理审计（限本组处理器）：文档 click 关闭器对弹层内点击不触发；setView 关闭弹层与 Esc 路径互斥；捕获相位的菜单键盘处理器在菜单未开时提前返回。以上为源码审查结论，实际键盘遍历 not_run。

## 9. 用户接受 UA-01

**pending / not_run**。U1–U5 未由实际用户执行；自动化与静态检查不能代签。

## 10. 代理自评与诚实声明

- 实现者为代理（OpenCode/GLM），未在真实浏览器中运行本原型；全部 browser/user 项为真实 not_run 状态。浏览器导航被工具安全策略拒绝，未尝试任何绕过。
- 已知实现取舍：设置窗口在 <600px 视口切换为上下布局以保证可达性；面板/菜单以网页弹层模拟原生菜单，与生产原生菜单不等价；R4 视觉修订未经实际渲染检查。
- 待主代理验证建议：file:// 下 CSP 与 defer 脚本加载（PUI-01 前置）、五流程计数（注意初始视图为设置·连接页，U2/U4/U5 需先用视图快捷回到悬浮条，不计操作）、S12 reducer 纯逻辑复测（同 taskId 多代仅最新代为当前）与最近 5 条顺序、菜单 Esc/左键回焦、两入口迷你同步、键盘遍历、320px 视口与 200% 缩放截图、R4 视觉基线（隐藏元素漏显修复、折叠评审工具可达性、子面板内容尺寸）。
