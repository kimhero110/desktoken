# 桌面身份与状态信号：第一阶段调查

2026-09-08。结论：**确认目标是显示为 ChatGPT 的官方桌面应用；找到有明确等待语义的协议候选，但尚未证明 QuotaBar 能只读订阅当前桌面任务。** WP-002 未完成，WP-004 不可开始。不是“已证实不支持”。

## 已核实身份（S-01）

见 identity-evidence.json：安装包 OpenAI.Codex，版本 26.901.6511.0，清单显示名 ChatGPT，可执行文件 app/ChatGPT.exe，签名 Valid、签署者 OpenAI OpCo, LLC；采样时22个同名进程均与安装目录匹配。两个 app-server 的直接父进程均为 ChatGPT，命令行未指定 --listen。

只保存路径匹配布尔值和传输分类，未保存用户路径、PID、完整命令行、聊天标题或聊天内容。命令类别为 Get-AppxPackage、Get-AppxPackageManifest、Get-AuthenticodeSignature、Get-CimInstance Win32_Process（内存筛选、脱敏输出）。这能排除本次进程来自其他同名目录，不能替代 AT-01 全套对抗样本。

官方桌面文档说明同一应用可选择 ChatGPT 或 Codex，ChatGPT 下还有 Chat/Work。因此安装包名称不能决定当前任务模式，也不能把 Codex 协议能力扩展到所有 Chat/Work 对话。[官方桌面说明](https://learn.chatgpt.com/docs/app)

## 信号矩阵（S-02）

本机既有生成类型的最小摘要及哈希见 protocol-evidence.json。生成二进制的精确哈希未保留，本次未重新生成，故不作为当前桌面版本运行证据。

| 用户需要 | 静态候选 | 仍缺什么 |
|---|---|---|
| 正在工作 | ThreadStatus.active | 当前桌面可达、任务模式覆盖、真实开始事件 |
| 等我授权 | activeFlags.waitingOnApproval | 后台真实等待、消除等待及轮次关联 |
| 等我补充输入 | activeFlags.waitingOnUserInput | 实际输入流程覆盖；不能等同所有自然语言提问 |
| 本轮结束 | turn/completed + turn.id/status | 只含状态的数据通道、真实事件与晚到事件验证 |
| 恢复工作 | 等待标记消失后 active | 同一任务轮次关联，不能用标记消失单独判定 |
| 断连 | 观察器自行失联检测（尚未实现） | 转 unknown，不能把 notLoaded/idle 当完成 |

官方 app-server 文档有 thread/status/changed 及 waitingOnApproval 示例；这只证明协议语义存在。thread/loaded/list 返回服务进程内加载的任务ID；另起服务器不能证明观察到了原桌面实例。当前进程未指定 --listen，按文档默认使用 stdio，未发现可据此使用的对外 WebSocket 参数。此检查没有枚举私有 IPC，也没有证明其他受支持入口不存在。[协议与传输说明](https://learn.chatgpt.com/docs/app-server)

## 两条不能直接采用的路线（S-03）

1. **另起 app-server 查询任务**：没有证明与桌面共用实时运行状态；不能拿它自己的空闲/未加载状态判定桌面结束。
2. **thread/read(includeTurns=false)**：类型仍返回 Thread.preview，可能包含首条用户消息；turn/completed 的 Turn 也包含 items/error。仅不保存正文不足以满足本方案“不读取聊天内容”的要求，因此本轮未调用这些接口。初始化通知黑名单也不等于已验证的只读权限或封闭字段白名单。

Codex hooks 已安装/是否被信任属于另一层证据；它不自动证明本桌面模式四态可用。本轮没有修改信任、配置或订阅用户会话。

## 当前出口与下一实验（S-04）

S-01 当前安装实例身份已核实；S-02 已完成静态字段及缺口矩阵；S-03 已完成传输分类，但独立只读订阅仍未验证；S-04 已同步入口文档。四项是调查交付，不是产品验收。

下一步限定为寻找**既有桌面实例、仅输出状态、带任务/轮次关联**的受支持接入机制，并核对其适用 Chat/Work/Codex 范围；优先公开协议/正式扩展机制。若涉及新认证、私有应用接口、内容读取或 UI 观察，须补充具体数据和权限设计再评审。暂不修改菜单或构建新适配器。

AT-01/02/03/04/11 的真实桌面测试均未通过本次研究完成；通知、性能和视觉验收未运行。没有要求用户逐步确认，也未将资料缺口伪报为实现完成或确定不可行。
