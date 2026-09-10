# TODOS — QuotaBar 延期项

## 本机任务状态（开发版）

- [ ] ChatGPT 桌面端接入：本机身份可核实，外部只读订阅信号与真实四态仍缺实测证据，未接通。

- [x] 严格审查复现并修复 9 项缺陷，保留失败→通过的回归证据（见 docs/review-task-monitor-2026-09-06.md）。
- [ ] 上游轮次/事件序号乱序防护。
- [x] Windows DACL、所有者、组及继承保护回归验证；安装器异常退出锁恢复。
- [x] 三工具原生链路合成验收，多会话、并行授权、恢复、终态和重复提醒检查；接入 Windows CI。

- [x] 本地事件桥、最小元数据、工作/等待/结束/异常/未知状态。
- [x] 额度与任务分开展示；通知去抖和重复抑制；手动接入配置生成。
- [x] OpenCode 1.18.29 真实会话结束事件与通知候选联调，修复退出时异步丢事件。
- [x] OpenCode 真实授权等待与拒绝后结束验证；Windows 通知接口实际调用。
- [x] 设置页默认路径接入诊断及测试通知入口。
- [x] 本机预览版实际切换、配置备份、三个默认工具接入安装及 OpenCode 全局事件实测。
- [x] Codex只读诊断：17项测试通过；桌面子进程运行时匹配，8个QuotaBar hooks均已启用但未信任，见[实测记录](docs/codex-desktop-probe.md)。
- [x] 桌面Hooks页面已由用户截图确认；8项信任就绪，当前桌面对话working事件已真实到达QuotaBar接收器。
- [x] 当前桌面对话working事件接收、本轮结束面板截图确认；安装版隔离原生链路回归通过（3会话/7个通知候选）。
- [ ] 桌面真实等待授权/输入及恢复联调、多任务与系统通知可见性；Claude Code登录后联调。
- [ ] macOS hook 调用、桌面通知验证。
- [ ] Kimi Code、OpenClaw、Antigravity、OpenCode V2 适配。
- [x] 保留现有设置的一键安装/卸载、原文件备份和接入诊断。
- [ ] 一键回到对应任务窗口。

## v2 候选（有明确价值，v1 不做）
- [x] E8 本地用量历史（SQLite，7 天保留）+ 7 日 sparkline
- [ ] E11 用量预测（"按当前速率将于 X 时打满"，依赖 E8）
- [ ] E9 与 cc-switch 联动：95% 时"切换账号"入口（v1 先放"去管理订阅"外链）
- [ ] E10 Provider 插件贡献文档（CONTRIBUTING-providers.md；架构已预留）
- [ ] 单行摘要密度模式（设计 voice2 方案：默认"最差窗口"一行 + hover 三级展开）
- [ ] 浅色主题（跟随系统；半透明浅色需单独的对比度方案）
- [ ] Provider 拖拽排序
- [ ] macOS / Linux 移植（凭据路径与 keychain 读取方式不同，CodexBar 已有参考）
- [x] ~~Antigravity provider~~（已完成 2026-09-02：Gemini provider 内置 Antigravity 通道——凭据管理器只读 + daily-cloudcode-pa fetchAvailableModels + UA 门禁）

## 工程遗留（已知）
- [ ] OAuth 刷新端点的 429 独立退避/Retry-After（当前只覆盖额度 HTTP 请求）
- [ ] 系统唤醒后的专用错峰刷新
- [ ] 跨进程凭据写入竞争：compare-before-write 并非原子 CAS；进程退出会丢失待写回内存状态
- [ ] macOS `is_zh_locale` 对 Finder 启动的 GUI 应用不可靠（无 LANG/LC_ALL → 中文用户见英文菜单）：改走 NSLocale/CFLocale（2026-09-06 autoplan eng 评审发现，预存 bug）
- [x] OAuth 写回保留原文件权限/ACL（改走 atomic_file::replace，创建时即套用原文件安全描述符）
- [ ] 版本检查自定义源：ghproxy 镜像或自建 CDN manifest（国内可达性）
- [ ] 完整签名 auto-updater（tauri-plugin-updater + 证书；定位升级为对外分发产品时再做）

## 永不做（非目标，防 PR 引入）
- 内置 telemetry / 崩溃上报（与隐私定位冲突；诊断靠 E6 复制诊断信息）
- 重造 cc-switch 核心域（账号切换/代理切换）
- Web 版 / 浏览器扩展形态
- 凭据跨工具双向同步

发布边界（2026-09-07）：ChatGPT 桌面端未接入，Codex 桌面端未验证。Codex CLI hook 配置与桌面端监控不能混为一谈。v0.4.0-beta.1 为预览版，v0.3.2 保持稳定版。
