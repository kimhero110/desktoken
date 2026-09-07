# 安全策略 / Security Policy

QuotaBar 读取本机各家 CLI 的登录凭据去轮询用量接口。这是敏感定位，所以本文件把行为逐条写清——**欢迎抓包验证**。

## 凭据行为清单

| 行为 | 明细 |
|------|------|
| 读取的文件 | `~/.kimi-code/credentials/kimi-code.json`、`~/.codex/auth.json`、`~/.claude/.credentials.json`、`~/.gemini/oauth_creds.json` |
| 读取的凭据管理器条目 | `gemini:antigravity`（Antigravity 存的 Google 凭据，**只读**，从不写入） |
| 唯一写回场景 | OAuth token 过期时换新并写回**同一个文件**。写回前 compare-before-write：若官方 CLI 刚好也在刷新，采用它的、丢弃我们的 |
| 手动 API key | 只进系统凭据管理器（Windows 凭据管理器服务名 `quotabar` / macOS Keychain），绝不落盘 |
| 触达域名 | `api.kimi.com`、`auth.kimi.com`、`open.bigmodel.cn`、`api.z.ai`、`chatgpt.com`、`auth.openai.com`、`api.anthropic.com`、`console.anthropic.com`、`cloudcode-pa.googleapis.com`、`daily-cloudcode-pa.googleapis.com`、`oauth2.googleapis.com`、`api.github.com`、`github.com`（版本检查）。自定义监视/官方模板触达的域名由你自己的配置决定 |
| 绝不发送 | 凭据永不出本机；没有分析、没有崩溃上报、没有遥测 |

日志位于 `%APPDATA%\quotabar\spike.log`（macOS：`~/Library/Application Support/quotabar/`），落盘前统一过脱敏层。

## 报告安全问题

**请优先使用本仓库的 GitHub 私有安全通告（Security → Report a vulnerability）**——不要开公开 issue 报告可利用的漏洞。

报告时请附上：

1. 影响的行为（凭据读取 / 写回 / 日志脱敏 / 网络请求）
2. 复现步骤与 QuotaBar 版本
3. 如有，抓包或日志片段（**请先自行移除 token、key 等敏感值**）

期望响应时间：确认收到 7 天内，修复或缓解方案 30 天内。修复版本发布后会在 Release 说明中注明。

## 已知边界（非漏洞）

以下为设计取舍，不是安全问题，但欢迎讨论：

- **各家订阅额度接口均为非官方**（只暴露给自家 CLI），理论上违反平台 ToS。项目以保守轮询（Claude 10 分钟）+ 首启知情同意门禁降低风险。
- **compare-before-write 不是原子 CAS**：若官方 CLI 与 QuotaBar 同时刷新，以 CLI 的结果为准，QuotaBar 丢弃自己的轮换结果。
- **OAuth 待写回状态仅存在内存**：进程退出会丢弃（不会丢失你的登录态，只是放弃本次轮换）。

## 验证方式

不信任本文件也没关系——源码在此，可以自行编译；Release 均附带 `checksums*.txt`（SHA-256）与 GitHub 构建证明（Attestations），可核对产物确由 CI 从源码构建。
