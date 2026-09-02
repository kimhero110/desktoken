# TODOS — DeskToken 延期项（/autoplan 收集）

## v2 候选（有明确价值，v1 不做）
- [ ] E8 本地用量历史（SQLite ring buffer）+ 7 日 sparkline
- [ ] E11 用量预测（"按当前速率将于 X 时打满"，依赖 E8）
- [ ] E9 与 cc-switch 联动：95% 时"切换账号"入口（v1 先放"去管理订阅"外链）
- [ ] E10 Provider 插件贡献文档（CONTRIBUTING-providers.md；架构已预留）
- [ ] 单行摘要密度模式（设计 voice2 方案：默认"最差窗口"一行 + hover 三级展开）
- [ ] 浅色主题（跟随系统；半透明浅色需单独的对比度方案）
- [ ] Provider 拖拽排序
- [ ] macOS / Linux 移植（凭据路径与 keychain 读取方式不同，CodexBar 已有参考）
- [x] ~~Antigravity provider~~（已完成 2026-09-02：Gemini provider 内置 Antigravity 通道——凭据管理器只读 + daily-cloudcode-pa fetchAvailableModels + UA 门禁）

## 工程遗留（低优先级但已知）
- [ ] OAuth 写回后 re-apply 原文件显式 ACL（当前各家 CLI 未设显式 ACL，无害；协议文档已标注）
- [ ] 版本检查自定义源：ghproxy 镜像或自建 CDN manifest（国内可达性）
- [ ] 完整签名 auto-updater（tauri-plugin-updater + 证书；定位升级为对外分发产品时再做）

## 永不做（非目标，防 PR 引入）
- 内置 telemetry / 崩溃上报（与隐私定位冲突；诊断靠 E6 复制诊断信息）
- 重造 cc-switch 核心域（账号切换/代理切换）
- Web 版 / 浏览器扩展形态
- 凭据跨工具双向同步
