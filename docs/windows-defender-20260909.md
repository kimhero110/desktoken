# Windows Defender 检测记录（2026-09-09）

## 已确认的证据

- 本机 `Get-MpThreatDetection` 记录 2026-09-09 的检测，资源包含 `Downloads/quotabar (2).exe` 和 `HKCU/.../Run/QuotaBar`。
- `Get-MpThreat` 映射检出名称为 `Trojan:Win32/Bearfoos.A!ml`、`Behavior:Win32/Persistence.A!ml`；查询时 IsActive=false，处置记录 ActionSuccess=true。这不证明原文件安全。
- 对此前下载并校验过的归档 beta.2 程序执行 `Get-AuthenticodeSignature`，系统返回“无法成功完成操作，因为文件包含病毒或潜在的垃圾软件”。没有恢复隔离文件或绕过拦截。
- 2026-09-06 也有同一下载路径的历史检测。仅凭重复下载文件名不能确定当时的版本，不能据此宣称稳定版已通过杀毒验收。

## 闪窗修复

Windows 的 Antigravity 探测启动 PowerShell 时遗漏无控制台窗口标志。源码加入 `CREATE_NO_WINDOW`，沿用项目已有子进程处理方式，不改变探测命令、读取范围或数据流。此修改只针对控制台闪窗，尚未经过新发布包的视觉验收，也不证明 Defender 检测已解除。

## 尚未确认

杀毒检测的确切触发原因、是否误报、是否涉及供应链或运行行为，均未确定。检测资源中出现自启动项，只能证明该项在事件记录内，不能直接认定它就是根因。尚未取得 Microsoft 样本复核结论。

SmartScreen 信誉提示与 Defender 恶意软件检测是不同问题。代码签名、源码构建、哈希或构建证明都不能单独证明程序无恶意风险。不要通过关闭防护、添加排除项或恢复隔离文件来绕过当前检测。

GitHub 与 Gitee beta.2 发布页已同步提示 Windows 检测待核查，暂不推荐运行该 Windows 下载包。稳定版附件保留，但不额外声称其杀毒兼容性。
