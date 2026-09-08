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

### 控制台窗口

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
