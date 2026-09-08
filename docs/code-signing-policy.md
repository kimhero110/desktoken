# Code signing policy

## 当前状态

QuotaBar 尚未获得 SignPath Foundation 签名资格。Windows beta.2 安装包没有 Authenticode 签名，存在便携程序 Defender 检测待复核事项，见[检测记录](windows-defender-20260909.md)。构建来源证明与代码签名都不等于恶意软件检测豁免。

## 申请路径

优先申请 [SignPath 开源计划](https://signpath.org/apply.html)，保留现有 Windows 便携程序和 NSIS 安装包分发方式。项目使用 MIT 许可，具有公开源码、自动构建和已发布产物。申请表项目信息已准备；申请人姓名、联系邮箱和条款确认尚待维护者完成。没有申请成功回执，不宣称获得赞助。

根据 [SignPath 条件](https://signpath.org/terms.html)，需要可验证的构建、团队多因素认证、明确的签名批准人和每次发布的人工批准；项目信誉由 SignPath 自行评估。当前项目较新，不能保证获批。正式批准前不添加“Free code signing provided by SignPath”声明，也不捏造服务账户、项目 ID 或签名密钥。

## 拟定签名职责

- 源码维护与发布：仓库维护者 `kimhero110`，最终身份与批准人由维护者在申请时确认。
- AI 助手：准备代码、检查和材料，不充当实际身份验证人或签名批准人。
- 获批后的顺序：构建 → 测试 → 原生产物签名 → 安装包构建及签名 → 签名验证 → 最终产物校验和与来源证明 → 发布。签名后的字节发生变化，必须重新生成哈希，不能复用签名前的校验文件。
- 密钥或服务令牌仅存于经维护者授权的 CI 密钥设施；不纳入仓库，不自动放宽分支或签名权限。

## 隐私与系统行为

首次启动取得同意后，应用读取本机已有 AI 工具认证状态并向对应额度服务请求数据；自定义监视向用户配置端点发送请求。没有行为分析、崩溃上报或遥测。任务监视只在用户接入后启用；自启动由用户控制。具体边界与清理方法见 [README](../README.md#凭据行为清单安全核心)。第三方服务的数据处理由其各自政策约束，不承诺应用完全离线。

## Microsoft Store 备选路径

Microsoft Store 的 MSIX 提交在认证后由商店签名，详见 [Microsoft 签名说明](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options)。这是一条后续分发路径，不能追溯解除 beta.2 已有哈希的检测。

当前 Tauri 配置使用 NSIS，尚未配置 MSIX。正式启动商店迁移前，需要维护者 Partner Center 身份与包标识，并验证外部 CLI 钩子对安装路径的引用、更新后的路径稳定性、包内自启动声明、凭据访问、通知和卸载清理。不能直接把 Startup 快捷方式实现当作 MSIX 自启动方案。当前不生成含虚假 Publisher/Identity 的可发布包，也不声称已上架。
