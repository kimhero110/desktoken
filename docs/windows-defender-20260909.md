# Windows Defender 检测记录（2026-09-09）

## 已确认的证据

- 本机 `Get-MpThreatDetection` 记录 2026-09-09 的检测，资源包含 `Downloads/quotabar (2).exe` 和 `HKCU/.../Run/QuotaBar`。
- `Get-MpThreat` 映射检出名称为 `Trojan:Win32/Bearfoos.A!ml`、`Behavior:Win32/Persistence.A!ml`；查询时 IsActive=false，处置记录 ActionSuccess=true。这不证明原文件安全。
- 对此前下载并校验过的归档 beta.2 程序执行 `Get-AuthenticodeSignature`，系统返回“无法成功完成操作，因为文件包含病毒或潜在的垃圾软件”。没有恢复隔离文件或绕过拦截。
- 2026-09-06 也有同一下载路径的历史检测。仅凭重复下载文件名不能确定当时的版本，不能据此宣称稳定版已通过杀毒验收。

## 进一步排查与修复

### 检测阶段与产物身份

Defender Operational 日志 1116/1117 补充确认：北京时间 2026-09-09 00:02:14，归档目录 `releases/v0.4.0-beta.2/quotabar.exe` 被检出 `Trojan:Win32/Bearfoos.A!ml` 并隔离，关联进程为执行签名查询的 PowerShell。这说明读取原始归档产物也能触发检测，不能仅用应用启动或写自启动项解释全部事件。

当时安全情报版本为 `1.459.97.0`，引擎版本为 `1.1.26080.3`。下载目录另有 `Behavior:Win32/Persistence.A!ml` 事件；其文件名不能单独确定版本。

本地留存的发布校验清单与 GitHub Release API 的资产 digest 一致：

| beta.2 Windows 产物 | SHA-256 |
| --- | --- |
| `quotabar.exe` | `07d9cce61eccc6767620da507cff93f36d438e332c66cda418991921e3f39673` |
| `QuotaBar_0.4.0-beta.2_x64-setup.exe` | `814e45d694968104f07f0fbb2b888dc0234718e9ab59c2e1dd66c2167e1d7345` |

上述哈希用于识别发布产物，并非重新读取隔离文件所得，也不构成安全结论。

排查期间通过 `Update-MpSignature` 正常更新安全情报至 `1.459.111.0`，确认实时保护仍开启。未恢复隔离样本，未据此宣称复测通过。

### 安装包复测与构建来源（2026-09-09）

在实时保护开启、安全情报 `1.459.111.0` 下，对原有 beta.2 安装包执行自定义扫描，不安装、不运行。初次使用正斜杠路径，扫描返回 `0x80508023`，不能记为未检出；改为 `Get-Item(...).FullName` 提供的 Windows 原生路径后，`MpCmdRun -Scan -ScanType 3 -File <安装包>` 返回 0，并明确输出 `found no threats`。仅代表该安装包在这一环境下的静态扫描结果，不代表隔离的便携程序或安装后行为通过。

重新读取安装包 SHA-256 与上表一致。`gh attestation verify <安装包> --repo kimhero110/desktoken --format json` 验证成功；证明关联发布工作流 `.github/workflows/release.yml`、提交 `edf4fa2ba63399d961977bea7439e7082b359611` 和运行 `34209580821`。同一证明声明中包含上表的便携程序哈希，但未恢复或重新校验隔离文件。来源证明不排除源码、依赖或构建过程风险。

安装包 Authenticode 查询结果为 `NotSigned`。这与信誉提示有关，但不能据此认定 Bearfoos 或 Persistence 检测的根因。没有通过重新打包、混淆或改变文件标识来尝试绕过检测。

### 控制台窗口修复

Windows 的 Antigravity 探测启动 PowerShell 时遗漏无控制台窗口标志。源码加入 `CREATE_NO_WINDOW`，沿用项目已有子进程处理方式，不改变探测命令、读取范围或数据流。此修改只针对控制台闪窗，尚未经过新发布包的视觉验收，也不证明 Defender 检测已解除。

## 自启动修复与未决问题

### 自启动正确性修复（尚未发包）

检查启动链发现自启动命令未加路径引号，启动时重复写入相同配置，关闭时仍调用创建注册表键，且删除失败被忽略。源码修正这些行为：为可执行文件路径加引号；已有命令一致时不写入；关闭且键不存在时直接成功；只删除 QuotaBar 条目，并将实际读写错误返回调用方。启动阶段仍沿用既有的尽力同步方式，设置窗口修改失败不会保存为成功。

这些是代码正确性问题，目前没有证据证明它们是 Defender 检测根因。修复不改变杀毒设置，不隐藏应用自启动条目。

验证：`cargo test --locked` 共 118 项通过，包括新增的路径引号、重复启用、只读句柄证明不重复写入、关闭时保留其他条目与错误传播测试。注册表测试只使用独立临时 HKCU 子键，运行后确认无测试键残留；不操作真实 Run 键。此结果不等同于发布包的杀毒或视觉验收。

### 未决结论

杀毒检测的确切触发原因、是否误报、是否涉及供应链或运行行为，均未确定。检测资源中出现自启动项，只能证明该项在事件记录内，不能直接认定它就是根因。尚未取得 Microsoft 样本复核结论。

SmartScreen 信誉提示与 Defender 恶意软件检测是不同问题。代码签名、源码构建、哈希或构建证明都不能单独证明程序无恶意风险。不要通过关闭防护、添加排除项或恢复隔离文件来绕过当前检测。

GitHub 与 Gitee beta.2 发布页已同步提示 Windows 检测待核查，暂不推荐运行该 Windows 下载包。稳定版附件保留，但不额外声称其杀毒兼容性。

## Microsoft 复核资料

使用 [Microsoft 官方样本提交入口](https://www.microsoft.com/en-us/wdsi/filesubmission)，选择 Software developer，产品选 Microsoft Defender Antivirus。提交说明应包含上述检测名、情报及引擎版本、原始发布产物哈希，以及 [beta.2 发布页](https://github.com/kimhero110/desktoken/releases/tag/v0.4.0-beta.2)。说明该程序是本地额度与任务状态桌面工具，含用户可选自启动和本地任务事件接收功能；请求判定检测原因，不预设误报结论。

当前仅完成资料整理，尚未上传样本或取得提交编号。原始便携程序已被隔离，不通过恢复隔离或关闭保护获取样本。代码正确性修复与厂商样本复核分别记录；只有完成后续产物验收，才能更新 Windows 下载建议。

已获维护者授权提交公开产物及脱敏证据；官方页面选择 Software developer 后要求 Microsoft 账户登录，当前尚未完成登录。授权与实际提交分别记录，不把打开页面视为复核已发起。

可直接用于表单的[英文复核说明](releases/beta2-microsoft-submission.md)已备妥，不包含本机账户、原始路径或凭据。

## 后续源码改造：移除 Windows PowerShell 运行链

按维护者要求，当前源码将 Antigravity 探测改为 Rust 调用 WMI 与 IP Helper TCP 表，将生成的 Codex 钩子改为原生接收器命令、Claude 钩子改为参数数组直启；ACL 测试也改用原生安全 API。正常启动不再写入自启动配置；用户设置动作改为管理 Startup 快捷方式，并清理匹配当前程序的旧 Run 值。冲突项保留并报告错误。

已有旧 Run 项不会在普通启动时偷偷迁移，升级后需切换一次设置。旧 PowerShell 钩子也不会自动改变，需重新生成并完成客户端信任确认。维护用 `.ps1` 取证/发布脚本不由应用启动，此次没有把这些独立工具迁入主程序。具体方案见 [Windows 原生运行链](plans/windows-native-runtime.md)。

上述源码改造不修改 beta.2 的既有哈希，不替代 WDSI 对旧样本的判定。签名申请状态单独记录在 [Code signing policy](code-signing-policy.md)。

本地原生改造 release 构建完成，事件桥和生命周期测试通过；SHA-256 为 `a267daccfff0be097c2c3e01038914c9c5f42b431ef67d221924a236f4c44659`。在实时保护开启、安全情报 `1.459.111.0` 下，自定义扫描明确未检出，退出码 0。这不是对 beta.2 原始便携文件的复测，也不构成供应链完整审计或未来检测豁免。尚未发布此新产物。

## 可复用的本机取证

运行 `scripts/collect-defender-evidence.ps1 -Days 7 -OutputPath <新的JSON路径>`，只读取 Defender 状态和与 QuotaBar 路径有关的 1116/1117 事件，不扫描、上传或修改保护设置。输出保留检测名称、时间、版本及资源分类布尔值，不保留原始路径、账户名、SID 或命令行。历史版本缺失时保留空值，不用当前版本填补。

本机实际运行收集到 11 条相关事件、0 条采集错误，覆盖两种已有检测名；输出结论仍为 undetermined。没有事件也不会被解释为安全。原始脱敏证据留在本机，未自动纳入公开仓库。

最终 `scripts/test-defender-evidence.ps1` 共 40 项断言通过；包括真实 XML 命名空间、历史版本缺失、路径过滤、DTD 拒绝、数组形状和拒绝覆盖已有文件。测试不调用真实 Defender 接口；实际采集单独验证。本次未修改应用 Rust 代码，不把前次 118 项 Rust 测试重复记作本次新增验证。
