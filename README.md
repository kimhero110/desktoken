# QuotaBar

> **EN**: A lightweight, non-intrusive Windows desktop floating bar that shows real-time AI coding subscription quotas (5-hour rolling window, weekly quota, reset countdown) for Kimi, GLM, Codex, Claude and Gemini — all in one glance. Open source (MIT), zero telemetry, credentials stay 100% local. Jump to [Quick Start](#安装).

![QuotaBar 悬浮条](docs/screenshot.png)

**常驻桌面的迷你悬浮条，一眼看清你所有 AI 编程订阅的额度和重置倒计时。** 不再因为不知道还剩多少额度而不敢放开用，也不再被突然的 429 限流打断思路。

---

## 目录

1. [功能特性](#功能特性)
2. [支持平台](#支持平台)
3. [安装](#安装)
4. [首次运行](#首次运行)
5. [凭据行为清单](#凭据行为清单安全核心)
6. [工作原理](#工作原理)
7. [自定义监视引擎](#自定义监视引擎开放框架)
8. [卸载与清理](#卸载与清理)
9. [FAQ](#faq)
10. [它不是什么](#它不是什么非目标)

---

## 功能特性

- **悬浮条常驻桌面**：半透明深色 acrylic、置顶、整面拖动、**永不抢键盘焦点**（`WS_EX_NOACTIVATE`，在 VS Code / 终端里打字时拖动它也不会丢光标）
- **5 小时滚动窗口 + 每周窗口 + 重置倒计时**：`43m / 2.1h / 3d4h` 三档倒计时格式
- **颜色告警**：绿 <70% → 黄 70-90%（带斜纹，色盲可辨）→ 红 >90%；任一窗口超阈值时整条边框变色，余光即可感知
- **Windows 原生 Toast**：用量跨过 90% 提醒一次；窗口重置时「已重置，放开用」
- **迷你模式**：收缩成 280×22 状态细条，托盘图标可恢复
- **凭据自动发现**：装了官方 CLI 就零配置出数；也支持手动粘贴 API key
- **自定义监视引擎**：任何「一个 GET 返回 JSON 额度」的平台，在设置里填端点 + 认证头 + JSON 路径映射即可接入，无需改代码
- **零遥测**：没有任何数据上报，诊断信息靠手动复制（自动脱敏）

## 支持平台

| 平台 | 凭据来源 | 轮询间隔 | 备注 |
|---|---|---|---|
| **Kimi** | 自动读 `~/.kimi-code/credentials/kimi-code.json`（OAuth 自动刷新写回），或 Console API key | 2 分钟 | Kimi CLI 自用接口 |
| **GLM 智谱** | 手动粘贴 API key（存 Windows 凭据管理器） | 2 分钟 | 国内/国际双端点自动试；积分制套餐兼容 |
| **Codex** | 自动读 `~/.codex/auth.json`（OAuth 刷新） | 2 分钟 | Codex CLI 自用接口 |
| **Claude** | 自动读 `~/.claude/.credentials.json`（OAuth 刷新写回） | **10 分钟**（保守，防频控） | 非官方接口，429 严格退避 |
| **Gemini** | 自动读 `~/.gemini/oauth_creds.json`，或 **Antigravity IDE 凭据**（凭据管理器 `gemini:antigravity`） | 5 分钟 | Antigravity 通道按模型家族聚合日额度 |
| **自定义** | 设置页配置 | 自定义（≥1 分钟） | 见下文开放框架 |

> 没有安装对应 CLI 的平台那一行不会出现；某家接口挂了就那一行变灰，互不影响。

## 安装

### 下载预编译包（推荐）

到 [GitHub Releases](https://github.com/kimhero110/desktoken/releases) 下载：

- **`QuotaBar_x64-setup.exe`** —— NSIS 安装包（自动处理 WebView2 依赖）
- **`quotabar.exe`** —— 绿色单文件，下载即用

国内网络慢的话：把下载链接前面拼上你常用的 GitHub 加速镜像即可（文件在 Release 附件里，镜像站通用）。

**SmartScreen 提示**：本项目未购买商业代码签名证书，Windows 可能提示「已保护你的电脑」。点「更多信息」→「仍要运行」。每个 Release 都附 `checksums.txt`（SHA-256）和 GitHub 官方构建证明（Attestations），可核对文件确实由本仓库 CI 构建。

### 从源码构建

```bash
git clone https://github.com/kimhero110/desktoken.git
cd desktoken/src-tauri
cargo tauri dev      # 开发模式
cargo tauri build    # 产出安装包 + 绿色 exe
```

要求：Rust stable + Windows 10/11（WebView2，安装包会自动处理）。

## 首次运行

第一次启动会弹出**知情同意对话框**（同意前程序不发出任何网络请求，可以抓包验证）：

1. QuotaBar 使用本机官方 CLI 的登录凭据，轮询各平台**非官方用量接口**
2. 这可能违反平台服务条款，理论上存在账号被限制的风险（保守轮询已尽可能降低）
3. token 过期时会自动刷新并**写回凭据文件**（与官方 CLI 行为一致）；所有数据仅存本机，绝不上传

同意后自动发现已安装的 CLI 凭据并立即出数。不同意则直接退出，不留任何后台行为。

## 凭据行为清单（安全核心）

这个工具触碰你的登录凭据，所以边界必须写清楚。**欢迎抓包验证**：

| 行为 | 明细 |
|---|---|
| **读取的文件** | `~/.kimi-code/credentials/kimi-code.json`、`~/.codex/auth.json`、`~/.claude/.credentials.json`、`~/.gemini/oauth_creds.json` |
| **读取的凭据管理器条目** | `gemini:antigravity`（Antigravity IDE 存的 Google 凭据，**只读**，我们从不写入） |
| **唯一写回场景** | OAuth access token 过期时，用 refresh_token 换新后写回**同一个文件**（与官方 CLI 相同的行为；写回前做 compare-before-write，若官方 CLI 刚好也刷新了，采用它的、丢弃我们的） |
| **手动 API key 存哪** | Windows 凭据管理器（服务名 `quotabar`），绝不写入任何文件 |
| **触达的域名** | `api.kimi.com`、`auth.kimi.com`、`open.bigmodel.cn`、`api.z.ai`、`chatgpt.com`、`auth.openai.com`、`api.anthropic.com`、`console.anthropic.com`、`cloudcode-pa.googleapis.com`、`daily-cloudcode-pa.googleapis.com`、`oauth2.googleapis.com`、`api.github.com`、`github.com`（版本检查）——**仅此而已** |
| **绝不发送** | 你的凭据永远不会发往上表之外的任何地方；没有分析、没有崩溃上报、没有遥测 |

日志文件在 `%APPDATA%\quotabar\spike.log`，写入前经统一脱敏层处理（token 模式 + 已知 key 字面替换）。「复制诊断信息」输出同样过这层脱敏。

## 工作原理

- **轮询**：每家平台独立任务，启动即取数，之后按上表间隔；系统休眠唤醒后自动错峰全量刷新
- **失败隔离**：某家解析失败/超时/429 只影响那一行（灰显 + 错误文案），其他家照常
- **429 退避**：遵从 Retry-After，指数退避（×2，封顶 8 倍周期），带 ±20% 抖动
- **宽容解析**：字段缺失/类型混用都尽量解析；响应统一过 sanitize 管线（窗口数、字符串长度、百分比钳制）
- **OAuth 写回协议**：重读 → 临过期 5 分钟才刷新 → 单飞锁 → compare-before-write → 原子改名重试；写不进就用内存 token，绝不丢可用状态

## 自定义监视引擎（开放框架）

设置 → 自定义监视 → 填四样东西即可接入新平台：

- 端点 URL（一个 GET 返回 JSON 额度）
- 认证头名 + 前缀（如 `Authorization` + `Bearer `）
- 窗口映射：`标签 | used 路径 | limit 路径 | reset 路径(可选)`，点语法如 `data.usage.used`，支持数组下标 `data.limits.0.percentage`
- 轮询间隔

reset 字段自动识别 epoch 秒/毫秒/RFC3339。与内置平台同渲染、同告警、同失败隔离。

## 卸载与清理

- **设置文件**：`%APPDATA%\quotabar\settings.json`（窗口位置、启用的平台、toast 状态）
- **日志**：`%APPDATA%\quotabar\spike.log`
- **手动 key**：Windows 凭据管理器中服务名 `quotabar` 的条目
- **开机自启**：设置里关闭，或卸载前取消勾选（注册表 `HKCU\...\Run\QuotaBar`）

卸载后删除上述目录和凭据条目即无任何残留。**我们不会替你读 Antigravity 的凭据条目做删除——那是它的东西。**

## FAQ

**Q: 数字和官方显示对得上吗？**
悬浮条 tooltip 里标了数据来源（官方接口/手动 key）。Kimi 可对照 CLI `/usage`，Codex 可对照 CodexBar，Claude 对照官方用量页。轮询有间隔，几分钟内的差异正常。

**Q: 会被杀软报毒吗？**
未签名 + 读取凭据文件的行为确实容易触发启发式告警。我们在 Defender 上实测通过；若你的杀软报毒，请核对 `checksums.txt` 确认是 CI 构建产物，然后从源码自构建（最安心）或加白名单。

**Q: 怎么更新？**
右键 → 检查更新；有新版本时悬浮条顶部会出现黄色横幅（可「跳过此版本」）。绿色 exe 直接下载替换；安装包覆盖安装保留设置。

**Q: 为什么 Claude 十分钟才刷新一次？**
Claude 的用量接口是非官方的，频控敏感。保守轮询是刻意的（PLAN 里写明的 ToS 决策），10 分钟是下限，不可调低。

**Q: Gemini 个人版提示已迁移 Antigravity？**
装 Antigravity IDE 并登录即可——QuotaBar 会读它的凭据走 Antigravity 通道出数（只读，不动它的凭据）。

## 它不是什么（非目标）

为防止范围蔓延，以下永远不做：

- 内置遥测/崩溃上报（与隐私定位冲突）
- 账号切换器（那是 cc-switch 的领域）
- Web 版 / 浏览器扩展
- 凭据跨工具双向同步

---

## Contributing

欢迎 issue 和 PR。**提交前请跑 `cargo test`**（src-tauri 目录，45+ 个 fixture/协议测试）。

License: [MIT](LICENSE)
