# Contributing to QuotaBar

欢迎！两条硬规矩，其余随意。

## 开发环境

- Windows 10/11 + Rust stable
- 前端零依赖：原生 HTML/CSS/JS，不需要 Node

```bash
git clone https://github.com/kimhero110/desktoken.git
cd desktoken/src-tauri
cargo tauri dev     # 跑起来
cargo test          # 提交前必须全绿（49+ 个 fixture/协议/对抗测试）
```

## 硬规矩

1. **提交前 `cargo test` 全绿。** 我们解析的全是别人的非官方接口，fixture 是命根子。
2. **发版只用 `release.ps1`**：`powershell -File release.ps1 patch`。版本号字段、tag、产物三处的同步只有这一条通道。Gitee 发版也包含在内（token 走 `GITEE_TOKEN` 环境变量或 `~/.quotabar-gitee-token`）。

## 加一个新平台（两条路）

1. **零代码**：设置 → 自定义监视，填端点 + 认证头 + JSON 路径映射。适合「一个 GET 返回 JSON 额度」的平台。先试这条。
2. **Rust 原生 provider**（仅当需要 OAuth 刷新/多步握手/降级链）：`src-tauri/src/providers/` 下加一个文件，参照 `gemini.rs`（双通道）或 `kimi.rs`（最简）。要求：
   - 解析必须宽容（字段可缺、字符串/数字混用），输出必须过 `sanitize()`
   - ≥3 个 golden fixture 测试，其中至少 1 个畸形样本
   - 轮询间隔有下限意识（别给用户惹频控麻烦）

## 永不接受的 PR（产品定位红线）

- 任何形式的遥测/崩溃上报/行为统计
- 账号切换功能（那是 cc-switch 的领域）
- Web 版 / 浏览器扩展
- 凭据跨工具双向同步
- 把凭据写出到 CLI 文件之外的任何地方

## 安全相关

- 日志与诊断输出必须过 `diagnostics.rs` 的脱敏层，别自己 `println!` token
- 凭据读取集中走 `credentials.rs` / `oauth.rs`；新增外域凭据读取请在 PR 里单独说明

## 讨论

- Bug：[Issues](https://github.com/kimhero110/desktoken/issues/new/choose)（应用内「右键 → 复制诊断信息」可直接粘贴）
- 功能想法：开 Issue 先聊，别直接甩 PR
