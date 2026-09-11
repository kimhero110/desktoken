# Changelog

## 未发布

- 安全：OAuth 写回不再放宽凭据文件权限。此前临时文件由 umask 默认创建（macOS 上 0644），rename 后会覆盖 CLI 建立的 0600；现在临时文件按目标文件原权限创建，并补了回归测试。Windows 侧仍沿用目录 ACL 继承，显式 ACL re-apply 保持在 TODOS。
- 安全：自定义监视端点强制 https，明文 http 仅对回环地址放行；保存时与请求前各校验一次，旧配置同样覆盖。
- 安全：共享 HTTP 客户端只在同源内跟随重定向。reqwest 只会在跨主机时剥离 `Authorization`，用户自选的认证头名（如 `X-API-Key`）此前会被带到跳转目标。
- 安全：`tauri.conf.json` 配置了真实 CSP（此前为 null）。内联脚本仍需 `unsafe-inline`，要去掉得先把内联脚本外置，本次未做。
- 修复：设置页 provider 卡片的字段统一转义，与自定义监视卡片一致。
- 修复：OAuth 的撕裂读重试与 rename 退避改用 `tokio::time::sleep`。此前是 `std::thread::sleep`，最长约 6.3 秒占住轮询执行器线程，且持有单飞锁。
- 修复：User-Agent 按真实版本与平台生成，不再硬编码 `QuotaBar/0.1.0 (Windows NT; x64)`。
- 修复：capabilities 描述与实际窗口列表一致（sponsor 窗口确实在内，且使用 `close_sponsor`）。
- 工程：CI 增加 macOS 矩阵（此前只有 Windows，macOS 仅在发版时首次编译）、clippy 步骤（暂不阻断）、cargo-deny 依赖审计与 Dependabot。

## v0.3.3 (2026-09-09，稳定版)

- 稳定版维护：仅额度功能，Antigravity 探测改原生 WMI/TCP API，自启改 Startup 快捷方式（源自 fix/stable-windows-native，与 beta.3 同源改造；该分支独立于主线，未合并产品代码）。
- 验证：89 项 Rust、4 项前端测试通过；本机稳定版测试包通过 Defender 扫描。正式 CI 产物目录亦通过本机 Defender 扫描，哈希与构建来源验证通过；真实稳定版 UI 验收尚未报告。

## v0.4.0-beta.3 (2026-09-09，预览版)

- Windows Antigravity 探测使用原生 WMI/TCP API；任务钩子移除 PowerShell 包装。
- 自启改用 Startup 快捷方式，正常启动不再写 Run 项；显式设置时校验迁移归属和冲突。
- 升级后需重新生成钩子/个人插件并在宿主审核信任。
- 本地测试包经用户确认启动不闪黑窗、未被 Defender 拦截；旧 beta.2 复核结论未确认，保留提示。
- 发布范围、验证边界和下载见 [beta.3 发布说明](docs/releases/v0.4.0-beta.3.md)。


## v0.4.0-beta.1 (2026-09-07，预览版)

- 支持边界更正：ChatGPT 桌面端未接入，Codex 桌面端未验证；Codex CLI hooks 安装后仍需信任，不能据此宣称桌面端可用。

- 新增：命令行 task-install/task-remove 与设置页复用同一安装器，支持显式指定已存在的接收程序绝对路径。

- 验收：新增三工具原生生命周期端到端合成测试并接入 Windows CI；收紧真实授权验收条件，防止失败或漏发结束提醒仍判通过。

- 修复：Windows 接入安装替换配置及备份丢失自定义访问权限；改为创建文件时应用原所有者、组和 DACL，覆盖继承、禁止继承及显式拒绝规则的回归验证。

- 修复：安装器异常退出留下锁文件后无法重试；改用进程退出自动释放的操作系统文件锁，并验证跨进程互斥及强制终止后的恢复。

- 严格审查修复：会话关闭取消结束提醒、并行授权误恢复工作、暂时不可读事件被删除、原子写误删既有临时文件、空配置卸载被改动、非法 matcher 被判为匹配、同毫秒事件误去重。见 [9 项复现与修复记录](docs/review-task-monitor-2026-09-06.md)。

- 新增：接入配置预览后可一键安装/移除，合并已有 hooks、变更前备份、重复安装去重；配置损坏或用户修改冲突时保留原内容并报错。

- 联调：OpenCode 真实授权请求保持等待并触发通知候选，明确拒绝后正常结束；Windows 实际通知 API 调用成功，可见性待用户确认。
- 新增：设置页接入检查、最近事件时间和测试通知；保存通知开关失败时返回错误；增加独立通知自测入口。

- 联调修复：OpenCode CLI 退出时异步队列会丢结束事件，改为限时同步投递；1.18.29 真实会话验证 working → ended 及一次通知候选。
- 修复：Codex hooks JSON 使用官方字段 commandWindows；新增 task-config / task-watch 和可选真实会话测试脚本。

- 新增：本机任务页签、独立于额度的状态展示、可选桌面通知（默认关闭）。
- 新增：被动 hook 接收入口，Codex / Claude Code / OpenCode V1 配置生成；不自动更改工具配置。
- 可靠性：通知去抖、重复及旧事件过滤、会话隔离、有界缓存；无事件超时仅显示未知。
- 隐私：仅本地最小事件元数据，不记录提示词、命令或回复，不开放网络端口。
- 修复：迷你模式隐藏 providers 容器；页签和任务列表点击不触发拖动或额度详情。
- 文档：[接入、卸载与验证限制](docs/task-monitor.md)。真实工具会话、通知投递和 macOS 联调待完成。

## v0.3.2 (2026-09-06)

- 修复：无有效额度时显示灰色「—」；部分失败时标记汇总不完整，过期数据不参与建议。
- 修复：自定义轮询间隔生效（1–1440 分钟）；保存后重新读取配置。
- 修复：额度 HTTP 429 传递 Retry-After，冷却带抖动且不被手动刷新绕过。
- 修复：release 使用 unwind，平台请求 panic 转错误重试；SQLite 初始化失败降级并重试。
- 修复：OAuth 写回失败保留进程内待写回 token 对，跨轮询重试并优先采用 CLI 更新。
- 修复：设置页自定义名称/端点按文本转义；CLI 同源接口标签与凭据网络说明更正。
- 工程：Cargo.lock 包版本与 0.3.1 对齐，CI/构建强制 --locked；增加回归测试和测试数据隔离。


## v0.3.1 (2026-09-06)

- **修复：自定义监视在多实例改造中丢失分发**（providers::fetch_instance 补 custom 分支）
- **修复：settings.json 并发写竞争**——所有写入收口到全局写锁 + `settings::edit()` 原子读改写（poller toast 去重 / UI 开关 / 位置持久化 / 版本缓存）
- **修复：spike.log 无限增长**——超 2MB 自动截断保留尾部 2000 行
- **CI：main 分支 push/PR 跑 cargo test**（此前只在发版时测试）
- 清理死代码（history::series）

## v0.3.0 (2026-09-03)

- 真图标（quota 三柱 logo）+ 左键详情卡：每窗口剩余量、重置绝对时刻、官方页面链接、7 日 sparkline
- E8 用量历史：本地 SQLite，7 天保留
- 微交互：新 provider 加载占位、窗口重置闪绿、≥90% 跨账号指路建议、错误行一次性 hint
- 迷你模式重做：宽度贴合内容、程序员段子/时段关怀轮播（超阈值自动变指路）
- 设置页版本页脚 + 检查更新；CONTRIBUTING；GitHub Pages 落地页
- macOS 移植基础 + CI dmg 产物（Apple Silicon）
- CI 修复：transparent() 平台门控、icon.png、checksums 分平台

## v0.2.3 (2026-09-03)

- 四角灰斑根治（方角铺满；SetWindowRgn 方案实测更糟已回滚）
- 迷你模式 22px + 菜单勾勾与状态联动
- release.ps1 发版一条龙（bump→测试→tag→双推→盯 CI→Gitee 发行版）

## v0.2.0 (2026-09-03)

- GUI 子系统修复（消灭 console 黑窗）+ DWM 圆角（Win11）
- M5 主体：Toast 告警（≥90% 滞回去抖/重置瞬间）、复制诊断信息（统一脱敏+泄漏断言）、版本检查横幅、开机自启、右键菜单全接线
- release 流水线：checksums + build provenance attestation
- README 十章重写 + 真实截图 + 凭据行为清单

## v0.1.0 (2026-09-02)

- M1-M4：窗口骨架（NOACTIVATE/acrylic/拖动/托盘/单实例/迷你）、Kimi/GLM/Codex/Claude/Gemini 五家 provider
- oauth.rs 六步刷新并发协议（单飞/compare-before-write/rename 重试）+ spike B 对抗测试
- 首启 ToS 门禁（同意前零网络）
- Antigravity 通道逆向（本地 LS ConnectRPC + UA 门禁 + csrf token）
