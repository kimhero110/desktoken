# 本机任务状态（v0.4.0-beta.1 预览版）

QuotaBar 在额度之外增加“本机任务”页签，回答当前电脑上的 AI 是否正在工作、等待你操作或已结束一轮。范围仅为当前电脑；不提供远程机器、团队或跨设备管理。

## 接入

1. 从当前源码构建并运行 QuotaBar。v0.3.2 发布包不包含此功能。
2. 右键 → 设置 → 本机任务，选择工具并点击“生成配置”。复制文本框内容。
   也可以先查看生成内容，再点击“安装接入”：程序仅合并本工具条目，原文件备份路径会显示在页面。已有自定义内容冲突时保留原文件并报告错误。移除时同样先生成配置再点击“移除接入”。
3. Codex：合并到 `~/.codex/hooks.json`；Claude Code：合并到 `~/.claude/settings.json`。`~` 是当前用户目录（Windows 通常为 `%USERPROFILE%`）。已有文件请先备份；保留原有字段，在每个同名事件数组中追加生成的条目。**不能用整段内容覆盖已有配置**，也不要重复追加。
4. OpenCode V1：把生成的 JavaScript 保存为 `~/.config/opencode/plugins/quotabar-tasks.js`。若已存在同名文件，先检查并备份。自定义配置目录应使用工具实际的插件目录。
5. 重启对应工具。Codex 非托管 hook 需要在工具内完成信任；被策略禁用或旧版不支持 hooks 时不会产生事件。桌面版是否运行 hook 取决于其版本和配置，不能仅凭安装了 CLI 判断已接入。
6. 开始新的一轮任务，切换 QuotaBar 的“本机任务”页，确认出现对应工具、项目和会话。需要弹窗时，勾选设置里的“桌面通知”（默认关闭）。通知仍受系统权限、勿扰模式和系统投递策略影响。
7. 点击“检查接入”，分别查看默认配置文件匹配情况和最近收到事件的时间。配置缺失、不可读、JSON 无效、仅部分事件匹配、程序路径变化会分别提示。它不会检查账号登录或替你信任 hooks；自定义配置目录需手动核对。
8. 点击“发送测试通知”，确认桌面或通知中心出现“QuotaBar · 测试通知”。显示“已提交给系统”仅表示通知 API 返回成功，不等于用户实际看见。

配置使用当前 QuotaBar 可执行文件的绝对路径。移动程序或换安装位置后重新生成并安装；安装器借助接入记录移除旧的原样条目；固定路径原位更新无需修改。仅生成配置不会写入工具设置，“安装接入”按钮才写入。安装不更改授权策略，也不自动批准操作。Windows 配置通过 PowerShell EncodedCommand 安全传递程序路径；这是 UTF-16 编码的程序调用，不含网络脚本。

## 安装、备份与恢复

安装器只处理界面中三种工具的默认配置路径，保留其他设置和 hooks；相同配置重复安装不增加条目。变更前在目标文件旁保存 `*.quotabar-backup-*.bak`，并用 `*.quotabar-receipt.json` 记录自己安装的定义，供路径迁移和移除使用。原始备份可能包含你原有设置中的敏感信息，应保留在本机，不要提交仓库或上传。

Windows 替换配置和创建备份时，在文件创建阶段应用原文件的所有者、组和访问控制规则（DACL），保留继承保护；读取或应用权限失败则报错。审计 SACL 和特殊完整性标签不在本次验证范围。

配置采用临时文件重命名提交；安装器以操作系统文件锁防止自身并发修改，并在提交前重读以检测其他程序的编辑。其他程序不遵守锁时仍存在极短的竞争窗口，因此安装时不要同时编辑该配置。符号链接、损坏 JSON、结构异常、被改过的接入条目或同名 OpenCode 插件冲突会返回错误，不自动覆盖。

恢复时先退出对应工具，核对页面报告的备份文件，将该备份内容恢复到同目录的原配置路径，再重启工具。备份不会自动清理。正常“移除接入”只移除原样的 QuotaBar 条目，保留其他设置；手工改过的条目需自行核对。`*.quotabar-lock` 是保留的空锁载体，不代表安装器仍在运行；操作系统会在进程退出或被强制终止后释放实际锁，下次可直接重试。不要在安装期间删除锁文件。

## 状态含义与边界

| 状态 | 依据 / 含义 |
|---|---|
| 正在工作 | 提交提示、执行工具、授权回复或明确的 busy/retry 事件 |
| 等待授权 | PermissionRequest / permission.asked；可能随后被其他 hook 或策略处理 |
| 等待输入 | 已接入的提问工具或输入通知；未暴露事件的交互无法推断 |
| 本轮结束 | Stop / session.idle；只表示本轮停止，不证明任务目标完成或后台工作结束 |
| 发生错误 / 已中断 | 明确的失败 / 中断事件；后续 idle 不覆盖错误 |
| 状态未知 | 会话打开、没有明确终态的会话关闭，或工作/等待状态超过 15 分钟未获新事件；已有终态不会被关闭事件抹除 |

停止输出、CPU 降低、窗口切换不作为完成证据。等待、结束和异常通知去抖两秒，期间恢复工作会取消提示；相同状态不重复通知。新一轮工作后再次结束可再次提示。静默超时只改为未知，不推送“已完成”。

以工具加会话 ID 区分多个任务，显示项目目录末级名称及会话 ID 前八位。等待项优先；迷你条优先提示等待/错误，额度百分比仍保持其原意。任务页最多保留 100 条、最长 24 小时。重启后不重放旧通知；已经消费的事件不作为持久历史，需等工具再次上报。

## 本地数据与接收协议

hook 把 JSON stdin 交给 `quotabar task-event codex|claude|opencode`。该入口在桌面窗口和单实例插件初始化前执行，返回 `{}`，不返回批准、拒绝或阻止停止的决策。

原始输入最大 1 MiB，只提取工具名、会话 ID、项目目录末级名称、状态和接收时间。提示词、命令、工具参数、完整路径、回复和错误详情不写入事件文件。会话 ID 本身仍属于本地元数据。

事件目录为 QuotaBar 应用数据目录下的 `task-events`：Windows `%APPDATA%/quotabar/task-events`；macOS `~/Library/Application Support/quotabar/task-events`；Linux `$XDG_CONFIG_HOME/quotabar/task-events`（默认 `~/.config`）。事件经临时文件重命名写入，每 750 毫秒读取并删除，不启动监听端口、不上传。写入时清理旧文件，通常保留最多 256 条（并发写入可能短暂超出）；应用内上限 100 个会话。文件写失败时 hook 仍被动返回成功，不影响工具工作，但状态可能缺失。

事件时间是接收器启动时间，新事件还保留高精度接收时间，帮助区分同一毫秒内的事件；这不是各工具全局单调序号。并发 hook、后台代理和工具版本差异仍可能造成缺失或乱序。会话关闭单独标记以保留已有终态。接收器拒绝旧事件覆盖新状态，但不能据此证明整项工作成功。

OpenCode 按请求 ID 跟踪同会话内多个授权/提问；回复一个请求不会掩盖其他等待。内存上限为 100 个会话、每会话 64 个请求，单会话请求超限转为未知；终态清理请求记录。暂时读取失败的事件文件留待重试，不因读取失败直接删除。

严格审查的复现、修复和未验证项见 [审查记录](review-task-monitor-2026-09-06.md)。

## 支持与验证范围

- Codex：UserPromptSubmit、Pre/PostToolUse、PermissionRequest、Stop、Interrupt、SessionStart/End。
- Claude Code：上述共有事件，加 Notification、StopFailure；AskUserQuestion 对应等待输入。没有中断事件的退出只显示未知。
- OpenCode：V1 插件生命周期接口，busy/retry、权限、提问、idle/error/deleted。V2 插件 API 尚未实现。
- Kimi Code、OpenClaw、Antigravity：后续接入，现有额度查询支持不等于任务监控支持。

2026-09-06 本机联调结果：

| 工具 / 环节 | 结果 |
|---|---|
| OpenCode 1.18.29 | 真实极短会话退出码 0；收到 working → ended，状态机生成一次 ended 通知候选 |
| OpenCode 退出竞态 | 实测异步投递会丢 session.idle，已改为限时同步投递；回归测试要求事件在回调返回前写入 |
| Claude Code 2.1.220 | auth status 显示未登录；测试退出码 1，未收到有效状态，SessionEnd hook 被取消；尚未通过 |
| Codex CLI 0.153.4 | doctor 确认已配置认证；JSON 的 Windows 覆盖字段修正为 commandWindows；仍需在 /hooks 审阅并信任接入定义后验证真实会话 |
| OpenCode 真实授权等待 | 已通过：working → waiting_approval → ended；等待和结束各触发一次通知候选。测试明确拒绝 echo 请求，没有执行命令 |
| Windows 通知调用 | 通过实际 Tauri 通知接口发送测试消息，进程退出码 0，submitted=true；弹窗可见性仍需用户确认 |
| macOS | 尚未联调 |

此前 OpenCode 的 EEXIST 是沙箱无法读取用户配置目录造成，在当前用户权限下能正常启动。测试使用临时项目插件、临时 Claude 设置和隔离的接收目录；没有修改全局工具配置或中断现有会话。通知候选仅证明状态机触发条件成立，不能证明系统已显示弹窗。

OpenCode 适配器只对少量生命周期事件同步调用接收器，单次上限 1.5 秒，不转发逐 token 消息。此处使用有界同步是为防止 CLI 退出丢失末尾事件；接收器失败仍不阻止工具继续运行。

可重复验证：`cargo test --locked --manifest-path src-tauri/Cargo.toml`、`node --test tests/*.test.cjs`；Windows 构建后运行 `node scripts/test-task-bridge.cjs <exe路径>`，测试直接调用和生成的 PowerShell 输入链路。进程测试仅为子进程指定临时 APPDATA，结束后清理，不触碰真实配置。机器负载过高导致 hook 超时时，事件可能丢失，应按未知状态处理。

真实会话测试（需现有登录，会消耗一次极短模型请求）：`node scripts/test-live-tasks.cjs opencode` 或 `claude`。测试报告及临时项目保留在 `src-tauri/target/live-tasks-*`，报告不含消息正文；未收到结束及其通知候选时退出码为 2。

真实授权测试：`node scripts/test-live-approval.cjs` 临时启动仅监听 127.0.0.1、随机密码保护的 OpenCode 服务，在临时项目请求一次 echo，等待授权通知候选出现后拒绝请求，结束时终止测试进程。报告在 `src-tauri/target/live-approval-*/report.json`。该脚本会使用现有登录进行极短模型请求，不更改全局权限配置。

`quotabar task-test-notification` 通过和设置页相同的接口发送一条系统测试通知，输出 submitted/error 后退出；不创建窗口、不启动额度轮询、不修改设置。它可以与现有悬浮条同时运行。

诊断入口：`quotabar task-config <工具名>` 输出和设置页相同的配置；`quotabar task-watch <秒数>` 在 1–120 秒内输出脱敏状态和通知候选，复用桌面消费逻辑。**task-watch 会消费事件文件，只应用于隔离接收目录或桌面程序退出后的诊断**，不与桌面程序同时运行；它不发送系统通知。

依据官方接口：[Codex hooks](https://learn.chatgpt.com/docs/hooks)、[Claude Code hooks](https://code.claude.com/docs/en/hooks)、[OpenCode V1 plugins](https://opencode.ai/docs/plugins/)。接口以实际运行版本为准。

## 移除

先关闭任务通知，选择对应工具、生成配置并点击“移除接入”，再重启该工具。手动移除时：Codex/Claude 仅删除生成的 hook 条目，保留其他 hooks 和设置；OpenCode 删除自己添加的 `quotabar-tasks.js`。确认移除成功后，可自行清理旁边的接入记录与备份。退出 QuotaBar 后可删除其 `task-events` 目录。卸载 QuotaBar 前应先移除这些接入配置，避免工具继续调用不存在的程序。

## 可重复的本地验收

运行 `node scripts/test-task-lifecycle.cjs [exe路径]`：使用隔离 APPDATA，调用实际生成的 Codex/Claude PowerShell hook 和 OpenCode 插件，再由原生接收器、状态机处理。覆盖三个并行会话、并行授权部分回复、恢复、结束/失败保护和新一轮提醒，断言共 7 个通知候选且无敏感正文。报告保存到 `src-tauri/target/lifecycle-*/report.json`。此测试不调用模型、不修改工具配置，不替代真实 CLI 接入和系统弹窗验收。Windows CI 自动执行此测试及事件桥冒烟测试。

真实 OpenCode 授权验收仅在拒绝请求后正常结束、等待与结束通知各一次时通过；出现 failed、漏发或重复通知均失败。失败报告也保存到临时测试目录。

v0.4.0-beta.1 命令行安装入口：`quotabar task-install codex|claude|opencode 接收程序绝对路径`，移除用 `task-remove`。与设置页共用配置合并、备份、权限保留及冲突检测，不修改 hook 信任。接收程序必须已经存在，目标配置路径固定为相应工具默认位置。

## Codex 已安装但没有状态

运行 `node scripts/diagnose-codex-hooks.cjs [codex.exe绝对路径]`，只读查询 Codex 自己的 hooks/list。若显示 enabled=true、trust=untrusted，表示配置被发现但执行被信任门槛阻止；目录的 trust_level=trusted 不等于 hook 已信任。打开 PowerShell，运行 codex 进入 CLI，再输入 /hooks，审阅并信任来自 ~/.codex/hooks.json 的 QuotaBar 条目；重新打开 Codex 并开始任务。不要把 /hooks 当作普通聊天消息发送给助手。安装器不写信任记录，也不使用绕过信任参数。

发布边界（2026-09-07）：ChatGPT 桌面端未接入，Codex 桌面端未验证。Codex CLI hook 配置与桌面端监控不能混为一谈。v0.4.0-beta.1 为预览版，v0.3.2 保持稳定版。
