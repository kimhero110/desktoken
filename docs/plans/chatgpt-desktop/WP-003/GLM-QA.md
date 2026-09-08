# GLM原型实施与QA记录（2026-09-08）

当前结论：GLM已产出六文件并完成两轮指定返修；源码/纯逻辑检查通过，浏览器门槛阻塞，整体C未通过、不可到A或发布。原型保留在D:/Project/quotabar/.analysis/glm-wp003/prototypes/chatgpt-desktop，未接回项目源码。

编码执行：OpenCode 1.18.29，zhipuai-coding-plan/glm-5.3，会话ses_f82f40162ffeMI12UbiDmrkqGk。首次整包请求达到length而无文件，随后同模型low分两阶段生成并返修；没有改用Astra编码。过程与输出位置见GLM-HANDOFF.md。

实际检查：两个JS的node --check通过；HTML重复ID为0、内联事件为0、普通defer脚本及限制CSP存在；10项静态/纯逻辑断言通过，含两次A/B交错、两次同任务旧代晚到、S12当前3任务/历史1轮、断连unknown和凭据样例字段语义。证据glm-qa-results.json含文件哈希。这不是浏览器、DOM模拟或原生实测。

返修范围：断连不得推断失败、额度与任务连接独立、mini显示等待、实际事件reducer替代硬编码并发结果、同任务轮次当前/历史分离、认证前缀尾空格、点语法窗口映射、不支持文案、最小命中区源样式。已确认上述源码修订，实际渲染尺寸仍未验证。

未通过项：PUI-01运行期网络/副作用、PUI-02/03/04实际点击、PUI-05/07实际呈现、PUI-08键盘焦点、PUI-09视觉缩放、PUI-10/11真实控件操作均仍需浏览器检查；PUI-06只有纯逻辑部分证据。UA-01用户走查pending；生产ChatGPT监控AT均未执行。原型自身的acceptance.md为GLM自述，主代理以本文件和实际日志为准。

阻塞依据：WebBridge不具本地文件访问；随后CUA的Browser URL policy明确拒绝本地file导航，并禁止通过另一浏览器、localhost中转、原始CDP等变相访问。未更改安全设置、未绕过策略。可安全继续的源码及纯逻辑检查已完成，不能用它们填补浏览器证据。

处置：自动跟进glm保持暂停，避免对同一浏览器阻塞重复重试。用户要求继续后，主代理已启动独立源码复核；可复现缺陷继续交GLM返修，不等待例行授权。源码复核不替代浏览器或用户验收，未通过的门槛继续保留。保存隔离成果，不发布、不改真实应用/配置/凭据/hooks。不关闭或归档用户任务。

## 独立源码复核 R3（2026-09-08）

独立审查员 prototype_source_review 确认4项P2：场景事件升序存入但最近列表取前5条；子菜单先隐藏后寻找可见父项导致焦点返回路径失效；默认view为bar而非设置；右键迷你状态未同步外观checkbox。未发现可证实P0/P1。只读复核不等于浏览器交互验证。

主代理已复核相关源码并交OpenCode同一GLM-5.3会话定点返修，指令位于隔离目录QA-R3.md，日志qa-r3-events.jsonl和qa-r3-stderr.txt。返修完成前，旧10项通过记录仅对应glm-qa-results.json中的旧文件哈希。当前保持D返修，整体C未通过。浏览器限制不妨碍本轮源码修复，不需用户例行授权。

R3结果：GLM正常结束stop/exit0；双JS语法检查通过；独立审查员定点复核确认4项P2均在源码层关闭。新版哈希及边界见glm-qa-r3-results.json。原型仍保留隔离目录，实际浏览器、焦点、视觉及UA未验证，整体C不通过。无活动GLM进程，自动跟进仍暂停以避免重复空转。

视觉R4已实施：沿用现有产品配色/字体/圆角，调试区默认折叠、hidden规则修复、内容面板和正常流布局。8项静态检查通过，双JS与R3哈希一致；详见VISUAL-R4.md和visual-r4-results.json。用户可刷新已打开原型查看；代理未自动访问浏览器，整体C仍未通过。

U5可用性R5完成：暂停时长选择后焦点恢复、Esc单层关闭和菜单键盘导航已由GLM修复；源码/语法检查通过，HTML/CSS/scenarios与R4相同。详见USABILITY-R5.md与usability-r5-results.json，实际交互验收仍pending。
