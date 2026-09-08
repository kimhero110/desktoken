# Codex 桌面端探查诊断记录（2026-09-08）

`scripts/diagnose-codex-hooks.cjs` 是只读、有界的 Codex hooks 诊断脚本。它启动一次
`codex app-server --stdio` 子进程，只执行 `initialize` → `initialized` → `hooks/list`
三步 JSON-RPC 调用，不发起任何模型请求或线程调用，不写入任何设置或信任记录，到点即
终止自己拥有的子进程。官方接口依据：[Codex hooks](https://learn.chatgpt.com/docs/hooks)。

## 用法

```
node scripts/diagnose-codex-hooks.cjs [codex.exe绝对路径]
```

省略参数时使用 PATH 上的 `codex.exe`。输出为单行脱敏 JSON，退出码 0 表示诊断本身
成功完成（含 needs_trust），非 0 表示执行或协议失败（rpc_error、spawn_error、
timeout、invalid_response）。

## 输出 schema

```
{
  "ok": boolean,
  "category": "ready" | "needs_trust" | "disabled" | "no_hooks"
            | "rpc_error" | "spawn_error" | "timeout" | "invalid_response",
  "counts": { directories, hooks, enabled, enabledUntrusted, managed } | null,
  "events": [ { event, enabled, trust, managed } ],
  "desktopConnectionVerified": false
}
```

- `needs_trust`：存在启用但未达执行资格的 hook（`untrusted` 或信任后被修改的 `modified`）。官方 HookTrustStatus 枚举为 managed|untrusted|trusted|modified：`managed` 且 `isManaged=true` 视为有执行资格、绝不标记为 untrusted；未知/缺失的 trust 或事件名归入 `invalid_response`，不产生可行动的 needs_trust 结论。
- `ready`：所有启用的 hook 均已信任。**ready 也不代表 ChatGPT 桌面端已连接**；
  `desktopConnectionVerified` 恒为 `false`，本诊断无法证明桌面端订阅状态。
- 输出绝不包含 cwd、来源路径、警告/错误文本、命令或转录；事件名和信任状态经过
  白名单净化，异常形状的响应归入 `invalid_response`。
- 代码可被 `require` 而不启动任何进程；`summarize` 是纯函数，`diagnose` 的
  spawn 实现可注入，失败路径全部有 `tests/diagnose-codex-hooks.test.cjs` 覆盖
  （node:test + 伪子进程，不触真实运行时或网络）。

## 2026-09-08 实际诊断结论

| 检查 | 结果 |
|---|---|
| 已安装 Codex CLI `hooks/list` | 成功；8 个 hook 全部 `enabled=true`、`trust=untrusted`；目录无 warnings / errors |
| ChatGPT 桌面版（WindowsApps 运行时）直接 spawn | EPERM，无法作为 app-server 启动（`spawn_error`） |
| 桌面端子进程 app-server 实测（2026-09-08 补充） | 两个真实 ChatGPT 子 app-server 进程使用的可执行文件与诊断所用 CLI 运行时**完全相同**（codex-cli 0.153.4），两者均未携带 `--listen`，也没有任何信任绕过参数；全部 8 个 hook 解码后指向当前桌面版 QuotaBar 的 `quotabar task-event codex` 入口 |
| 结论 | 已安装 CLI 的 `hooks/list` 成功只是**配置证据**（hooks.json 被发现），**不是当前桌面端订阅证据** |

可执行文件级同一性是比配置匹配更强的**运行时身份证据**（桌面端确实在跑同一 codex-cli 0.153.4 app-server，且 hook 命令指向当前桌面版接收器），但它仍不构成"桌面端当前正在订阅并执行这些 hooks"的实证；`desktopConnectionVerified` 恒为 `false`。导出的 JSON 摘要不包含任何实际路径、命令或正文。

历史CLI说明（不作为桌面用户的操作要求）：CLI可用 `/hooks` 审阅来自
`~/.codex/hooks.json` 的 QuotaBar 条目，重新打开 Codex 后再验证真实会话事件。安装器、
诊断脚本均不写信任记录，也不使用绕过信任的参数。桌面端是否在 Chat/Work 模式运行
hooks 取决于其版本和配置，本诊断不对此做任何保证。

相关文档：[本机任务状态](task-monitor.md)。

## 实际复核结果

2026-09-08：GLM实现，主代理执行17项Node诊断测试全部通过。本机新脚本复测ok=true、category=needs_trust、hooks=8、enabled=8、enabledUntrusted=8、managed=0、desktopConnectionVerified=false。先前测试计时器及事件名大小写失败已修正，不算作通过记录。

本轮完成只读诊断，不写信任记录、不操作桌面会话。后续应优先核对桌面自身Hooks审核入口，再验证真实桌面事件，不能要求桌面用户切换CLI作为默认方案。信任仅是hook路线的前置条件，不保证所有Chat/Work模式支持。

## 桌面设置入口的新增证据

只读检查本机26.901.6511.0安装包的静态资源：settings-page资源将hooks-settings列入Coding分组；hooks-settings页面包含待审核列表、User config来源、Trust按钮，并通过当前hook的currentHash提交信任动作。由此确认桌面包内存在自身审核界面实现，之前只指引CLI不完整。未执行或修改应用资源、未调用私有接口或写信任。静态实现不等于已实际看到当前账号界面；入口实际可见性及信任后桌面事件仍待验证。

## 桌面用户的最小联调步骤

1. 在桌面应用设置中查找 Coding（编程）下的 Hooks；若没有入口，先核对界面，不切换CLI或改信任文件。
2. 在 User config 中审阅指向桌面quotabar.exe的8个事件条目，确认后使用应用自身Trust按钮。
3. 主代理复测只读诊断确认是否从needs_trust变化，再开始一条普通桌面任务验证工作/结束。不能把信任变化当作任务事件已收到。
4. 等待授权/输入需要单独真实样本；普通任务开始/结束通过不能代替完整四态验收。

最近复测：8个enabled、8个untrusted，无需重新安装或修改文件权限。当前无后台模型任务，等待桌面内实际审核或入口不可见反馈。
