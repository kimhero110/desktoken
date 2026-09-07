# 任务监控严格审查记录 · 2026-09-06

范围：本机事件接收、状态机、OpenCode 适配器、安装器、配置诊断。结论：复现并修复 9 个缺陷；这不是“已无缺陷”或“可以正式发布”的结论。

## 已复现并修复

| 编号 | 级别 | 触发与原错误 | 修复与证据 |
|---|---|---|---|
| R1 | P1 | Stop 后立刻 SessionEnd，把 ended 覆盖为 unknown，取消两秒后应发的结束提醒 | 单独记录会话关闭信号；已有结束/错误/中断结果及待发提醒保留。没有终态证据的关闭仍为未知。`session_close_must_not_cancel_completed_turn_notification` 修复前失败，修复后通过 |
| R2 | P1 | 同会话两个并行授权请求，回复一个就发 permission.replied，错误恢复工作；busy 也会掩盖另一个等待 | OpenCode 在内存按 sessionID/requestID 维护未处理请求；有剩余请求时继续显示等待。对应 Node 并发授权测试先失败后通过 |
| R3 | P1 | 文件暂时禁止读取但允许删除时，drain 读取失败仍执行删除，永久丢事件 | 读取错误留待下一轮；Windows FILE_SHARE_DELETE 锁场景真实复现，`temporarily_unreadable_event_must_survive_drain` 先失败后通过 |
| R4 | P1 | 原子写 create_new 碰到已有临时文件失败，清理逻辑却删除了别人的临时文件 | 仅清理本次成功创建的文件。`atomic_failure_must_not_delete_preexisting_temp_file` 先失败后通过 |
| R5 | P2 | 无自身条目时执行卸载，仍把用户的空 hooks 对象删除 | 保留原有空 hooks。`uninstall_without_owned_hooks_preserves_empty_hooks_object` 先失败后通过 |
| R6 | P2 | matcher 为数字时，诊断将其当作未设置，显示 matched | 显式验证 matcher 类型及事件组结构；安装器同样拒绝该结构。诊断测试先报告 matched 而失败，修复后为 invalid |
| R7 | P2 | 同一毫秒内开始与结束事件被仅按毫秒去重，结束被丢弃 | 接收器保存高精度接收时间，按毫秒及子毫秒时间排序与去重；兼容旧事件缺少新字段。`separate_events_in_same_millisecond_must_not_be_treated_as_duplicates` 先失败后通过 |
| R8 | P2 | 安装器异常终止后残留锁文件，后续安装永久失败 | 改用操作系统文件锁；残留文件测试先失败后通过，子进程持锁时安装被拒绝，强制结束该进程后无需清理即可安装 |
| R9 | P1 | 安装替换文件及备份只复制只读属性，丢失原 Windows DACL；保护继承的配置恢复成父目录继承权限 | 创建文件时传入原所有者、组和 DACL，在写入内容前应用权限；安装、升级、卸载及其备份覆盖继承和禁止继承（含显式拒绝规则），原权限读取失败时拒绝创建副本 |

P1 表示会造成核心通知丢失、状态误导或数据丢失；P2 表示边界条件错误或误导诊断。以上均有具体失败输出，不是仅凭推测列出的风险。

## 扩展验证

- 本次全量验证：107 项 Rust 测试、8 项 Node 测试通过。Windows ACL 测试仅修改隔离目录中的模拟配置，未改动真实工具配置。

- 安装/移除往返覆盖 128 组不同数量的用户 hooks、权限字段和 prompt handler，确认无关配置保留。
- 关闭会话分别覆盖正常结束、失败、中断、只有工作而没有结束证据四条路径。
- OpenCode 实机会话复测：working → waiting_approval → ended；等待和结束各出现一次通知候选；明确拒绝 echo 请求，没有执行命令。报告保留在本机 `src-tauri/target/live-approval-pXP8Zo/report.json`，不包含消息正文。
- 文件读取上限改为限制实际读取字节数，避免先读取整文件再检查大小；事件扫描先过滤 JSON 再计入单轮上限，避免临时文件占满扫描额度。

## 仍未证明、不能写成“已支持可靠运行”的部分

- Codex 实际 hook 会话与 Claude Code 登录后的完整流程仍待验证；OpenCode 的并发请求修复有契约测试，实机验证为单个授权请求。
- Windows 通知 API 已返回成功，用户是否实际看到弹窗尚未确认；勿扰、系统权限和投递策略会影响显示。
- 高精度时间是本机接收时间，不是上游事件序号，不能消除时钟回拨、延迟启动的旧 hook 或不同轮次的乱序。新协议也不为旧事件凭空补出顺序。
- Windows DACL、所有者、组及继承保护已通过隔离实测；不宣称完整安全描述符保真：审计 SACL、强制完整性标签及企业特殊策略尚未覆盖。比较访问规则时忽略系统重建文件后可能清除的 AUTO_INHERITED 历史标记，仍逐项比较 ACE 及其继承标记。
- macOS 运行链路尚未实机联调；非协作程序并发编辑配置时，提交前重读仍不是跨进程原子 CAS。

以上限制继续作为发布前检查项；本次未发布新版本。

权限实现依据：[Microsoft 文件安全与访问权限](https://learn.microsoft.com/en-us/windows/win32/fileio/file-security-and-access-rights)、[GetFileSecurityW](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-getfilesecurityw)。

## 后续验收补充

- 修正真实授权测试的判定漏洞：仅有等待提醒不能通过，必须明确拒绝后正常结束，且等待/结束通知各一次。新增回归测试拒绝失败、漏发和重复提醒。
- OpenCode 新一轮实测先在会话创建时超时；后续复测通过完整工作→等待→结束，报告 `src-tauri/target/live-approval-g1EBhT/report.json`。保留失败事实，不把短暂环境失败归为状态机成功。
- 原生合成验收通过：三个独立会话、7 次预期提醒候选；调用真实生成 hook、插件及原生状态机，但不是上游模型会话。已纳入 Windows CI。
- 独立系统通知再次返回 submitted=true、error=null；实际弹窗可见性仍未确认。Claude Code auth status 仍为未登录；Codex 信任步骤未完成。

- 本地 release 构建通过；预览包内 exe 再次通过事件桥和 3 会话/7 提醒原生验收，报告 `src-tauri/target/lifecycle-Qe9kUl/report.json`。同一 exe 的通知自测返回 submitted=true。未正式发布。

## 本机试用落地

已备份旧桌面程序和设置，在桌面原路径启动预览版并启用任务通知；未停止 AI 工具。Codex/Claude/OpenCode 默认位置接入已安装，Claude 原设置单独备份，未更改授权策略或信任。检查桌面运行程序哈希与验收预览包一致。新开 OpenCode 真实会话成功，旁观全局事件目录捕获 working 和 ended；报告 `src-tauri/target/desktop-rollout/global-session-WyQW1P/report.json`。该旁观器不消费事件；系统弹窗的实际可见性仍需人与桌面交互确认。
