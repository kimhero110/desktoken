# 设计证据与局限

观测日期2026-09-08。本包规划依据，不是产品支持证明。

| 来源 | 核实内容 | 不能推出 |
|---|---|---|
| src/settings.html 静态源码 | 3类现有配置与外观、通知混列；14组入口映射参见上游UI-MAP | 用户已接受新布局 |
| src-tauri/src/main.rs build menu区域 | 原菜单刷新/迷你/诊断/更新/报告/赞助/设置/退出及真实handler | 网页模拟能证明原生窗口焦点行为 |
| ../WP-002/identity-evidence.json | 本机显示名ChatGPT、包名OpenAI.Codex、版本26.901.6511.0、签名与路径匹配 | 同一应用内Chat/Work/Codex都可被外部监听 |
| ../WP-002/protocol-evidence.json | 静态状态字段、含消息预览的读取接口风险 | 当前桌面可订阅或真实测试通过 |
| [官方通知](https://learn.chatgpt.com/docs/notifications) | 完成通知设置、授权/问题通知独立开关、活动视图、宠物状态能力 | 本机开关已开启、系统通知必定可见 |
| [官方桌面](https://learn.chatgpt.com/docs/app) | ChatGPT/Codex入口及Chat/Work区分 | 包名足以识别当前模式 |
| [官方协议](https://learn.chatgpt.com/docs/app-server) | stdio默认、状态事件、读/恢复差异 | 第三方只读入口存在且满足本项目隐私约束 |

本轮额外只读检查已安装app.asar的文件目录和少量相关静态声明：存在hooks-settings、notifications-settings资源和内部pipe服务代码。未连接pipe、未执行包内脚本、未保存应用代码副本、未读取用户会话。内部实现不是稳定公开契约；这些线索不足以解除WP-004阻塞，故不作为本包依赖。

[Microsoft UI Automation缓存文档](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-cachingforclients)允许限定缓存属性；仅能支撑未来受限探测设计，不证明目标提供了纯状态控件或稳定任务ID。本包不读取UIA、不做截图识别或实时监听。

方向结论：必须将原生通知/活动视图作为后续比较基线。当前已获授权的明确价值是菜单失焦改善，尚无证据证明额外ChatGPT监控优于官方现成功能。因此先做小范围原型，保留用户走查和真实信号两道后续门槛。
