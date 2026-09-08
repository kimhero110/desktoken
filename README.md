# QuotaBar

<p align="center"><img src="docs/icon-256.png" width="96" alt="QuotaBar logo" /></p>

> **EN**: A tiny acrylic bar that lives on your Windows desktop and tells you exactly how much of your Claude / Kimi / Codex / GLM / Gemini quota is left — before the 429 does. Open source (MIT), zero telemetry, credentials are stored locally and sent only to the configured service for authentication. [Quick Start](#安装).

![QuotaBar 悬浮条](docs/screenshot.png)

**每个同时订阅了五家 AI 的人，都值得拥有一条额度版「血条」。**

你经历过这种绝望吗：代码写到心流正酣，Claude 突然 429。你愣住，打开网页，登录，找到用量页——5 小时窗口用了 98%，重置还有 41 分钟。好，这 41 分钟你什么也干不了，只能盯着屏幕反思人生。

QuotaBar 就是为这个瞬间生的。它常驻桌面角落，把五家的 5 小时窗口、每周额度、重置倒计时摊在你眼前。**被限流之前，你先看到它。** 

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
11. [请作者喝咖啡](#请作者喝咖啡)

---

## 功能特性

- **悬浮条常驻桌面**：半透明 acrylic、置顶、整面拖动、**永不抢焦点**——在 VS Code 里打字时拖它，光标纹丝不动（这是血泪换来的 `WS_EX_NOACTIVATE`，不是吹的）
- **左键点一下，展开详情卡**：每窗口的已用/剩余、重置的准确时刻、套餐与数据来源、官方页面直达链接，外加 **7 日用量 sparkline**（本地 SQLite，哪都不去）
- **5 小时窗口 + 每周窗口 + 重置倒计时**：`43m / 2.1h / 3d4h`，扫一眼就知道现在该猛用还是该省着
- **颜色告警**：绿 <70% → 黄 70-90%（带斜纹，色盲友好）→ 红 >90%。整条边框还会跟着变色，眼角余光就能感知
- **额度 Toast 提醒**：跨过 90% 叫你一声「要没了」；窗口重置时叫你一声「已重置，放开用」。就这两句，不多嘴
- **迷你模式**：收成一条细血条，左边是最紧张的那家，右边轮播程序员段子；哪家超 90% 时段子自动让位成指路建议（「Kimi 周才 36%，先去那边写」）；深夜自动切换成「该睡了」
- **本机任务状态（预览）**：它是在埋头干活，还是停下来等你授权？切到「本机任务」看一眼。支持显示工作、等待授权、等待输入、本轮停止和异常，也可单独开启桌面通知。[接入说明](docs/task-monitor.md)
- **凭据自动发现**：装了官方 CLI 就零配置出数。它自己读凭据、自己刷新 token、自己写回——和官方 CLI 一个姿势，不搞特殊
- **自定义监视引擎**：任何「一个 GET 返回 JSON」的平台，设置里填四个框就能接入，不用写一行代码
- **零遥测**：不联网汇报任何东西。它唯一会「说出去」的，是你手动复制的诊断信息（而且先过脱敏层）
- **Windows + macOS**：Windows 10/11 为第一公民；macOS 版从源码构建或等 dmg（CI 已出包）

## 支持平台

| 平台 | 凭据来源 | 轮询 | 接口性质 | 备注 |
|---|---|---|---|---|
| **Kimi** | 自动读 `~/.kimi-code/credentials/kimi-code.json`，或 Console API key | 2min | CLI 同源 | Kimi CLI 自家接口；兼容 5h 窗口的剩余额度返回格式（[接口说明](docs/kimi-quota-fix.md)） |
| **GLM 智谱** | 手动粘 API key（进 Windows 凭据管理器） | 2min | 半官方 | 智谱自家 coding 插件同源；国内/国际双端点自动试，积分制套餐也认 |
| **Codex** | 自动读 `~/.codex/auth.json`（OAuth 刷新） | 2min | CLI 同源 | Codex CLI 自用接口 |
| **Claude** | 自动读 `~/.claude/.credentials.json`（OAuth 刷新写回） | **10min** | 非官方 | 频控敏感，轮询慢是故意的，别催 |
| **Gemini** | 自动读 `~/.gemini/oauth_creds.json`，或 **Antigravity IDE** 的本地语言服务器（IDE 开着时数字和它的面板一模一样） | 5min | CLI 同源 / IDE 本地 | 两条通道自动选 |
| **自定义**（含官方模板） | 设置页配置 | ≥1min | 取决于端点 | 见下文开放框架；内置 Moonshot 官方余额模板 |

> **关于「官方接口」**：各家**订阅额度**（5h 窗口/周窗口）都没有公开 API——只暴露给自家 CLI，这就是上表「CLI 同源」的含义，也是保守轮询 + 知情同意存在的原因。**API 按量付费**用户请走官方通道：设置页内置了 Moonshot 开放平台余额模板（官方文档接口），Anthropic/OpenAI 的组织计费 API 面向 org 管理员且需要动态日期参数，不适合血条形态，未做模板。

没装的 CLI 那一行不会出现；哪家接口挂了，只有那行变灰，别家照常——**不把鸡蛋的崩溃放在一个篮子里**。

## 安装

### 下载（推荐）

[GitHub Releases](https://github.com/kimhero110/desktoken/releases)（国内慢就用 [Gitee 镜像仓](https://gitee.com/xu512/quotabar/releases)）：

- **Windows**：`QuotaBar_x64-setup.exe`（安装包，自动处理 WebView2）或 `quotabar.exe`（绿色单文件，扔哪儿跑哪儿）
- **macOS**：`QuotaBar_x64.dmg`（拖进 Applications 即可）

国内下载慢：链接前面拼你常用的 GitHub 加速镜像即可，文件在 Release 附件里，镜像站通用。

**SmartScreen 那一拦**：没买几百刀一年的签名证书，所以 Windows 会装模作样地保护你一下。点「更多信息」→「仍要运行」。不放心的同学：每个 Release 带 `checksums.txt`（SHA-256）和 GitHub 官方构建证明（Attestations），可核对这文件确实是 CI 从源码编的，不是谁半夜传的。

想试「本机任务」，请选 [v0.4.0-beta.1 预览版](https://github.com/kimhero110/desktoken/releases/tag/v0.4.0-beta.1)；v0.3.2 稳定版只管额度。ChatGPT 桌面端的 QuotaBar 插件目前需从源码单独打包，尚未随发布包提供，见 [插件接入说明](docs/codex-plugin.md)。

### 从源码构建

```bash
git clone https://github.com/kimhero110/desktoken.git
cd desktoken/src-tauri
cargo install tauri-cli --version "^2" --locked
cargo tauri dev      # 开发模式
cargo tauri build -- --locked    # 出安装包
```

要求：Rust stable（至少 1.89，安装器使用标准库文件锁）、Tauri CLI；Windows 构建还需要 MSVC C++ Build Tools 与 WebView2。前端运行和构建不依赖 Node；运行前端回归测试需要 Node.js 20+。

## 首次运行

第一次启动会弹一个**知情同意**对话框。在你点「同意」之前，程序**不发任何网络请求**——欢迎开抓包工具监督。

大意是：它用本机 CLI 的登录凭据查询各家内部用量接口；token 过期会自动刷新并写回。设置与历史保存在本机，认证凭据会发送到对应服务端，自定义监视则使用你配置的端点；没有遥测。具体使用限制以对应平台条款为准。

### 想知道 AI 是在工作，还是在等你？

额度能自动发现，任务状态还需要接上工具的钩子——相当于让它在开工、等授权和停下时，给 QuotaBar 捎个信。

ChatGPT 桌面端有两种接法：QuotaBar 插件，或配置窗口生成的用户钩子。**二选一就好**；已经用了插件，就别再点「安装接入」，免得一件事报两遍。Claude Code、OpenCode 的接法见 [本机任务接入说明](docs/task-monitor.md)。

**装好还差一步：审核并信任。**在 ChatGPT 桌面端打开 **设置 → Coding → Hooks**：

- 插件接入：找到 **QuotaBar**，审核并信任其中的钩子。
- 用户钩子接入：在 **用户配置** 中找到 QuotaBar 对应条目，审核并信任。

开关变蓝不等于已经信任。**审核前，对应钩子不会上报任务状态**；以后钩子定义有变化，也可能需要重新审核。QuotaBar 不会替你点信任，更不会替你批准 AI 的操作。

完成后，按接入说明重启对应工具、开始一条任务，再到「本机任务」看状态。想让它主动叫你，另行开启「桌面通知」并发送一次测试通知；系统勿扰模式也得放行。

本机任务仍是预览功能，ChatGPT 桌面插件还在接入验证阶段。不同版本、模式的支持范围与已知限制，见 [接入说明](docs/task-monitor.md)和[插件验证记录](docs/codex-plugin.md)。

## 凭据行为清单（安全核心）

这章不好笑，因为凭据不是笑话。逐条列清，**欢迎抓包验证**：

| 行为 | 明细 |
|---|---|
| **读取的文件** | `~/.kimi-code/credentials/kimi-code.json`、`~/.codex/auth.json`、`~/.claude/.credentials.json`、`~/.gemini/oauth_creds.json` |
| **读取的凭据管理器条目** | `gemini:antigravity`（Antigravity 存的 Google 凭据，**只读**，从不写入） |
| **唯一写回场景** | OAuth token 过期时换新并写回**同一个文件**。写回前做 compare-before-write：若检测到官方 CLI 已更改文件，采用它的；写前比较不能完全消除跨进程竞争窗口 |
| **手动 API key** | 只进 Windows 凭据管理器（服务名 `quotabar`），绝不落盘 |
| **触达域名全表** | `api.kimi.com`、`auth.kimi.com`、`open.bigmodel.cn`、`api.z.ai`、`chatgpt.com`、`auth.openai.com`、`api.anthropic.com`、`console.anthropic.com`、`cloudcode-pa.googleapis.com`、`daily-cloudcode-pa.googleapis.com`、`oauth2.googleapis.com`、`api.github.com`、`github.com`（版本检查）。**多一个都没有**。自定义监视/官方模板触达的域名由你自己的配置决定（如选用 Moonshot 模板则为 `api.moonshot.cn` 或 `api.moonshot.ai`） |
| **网络认证与遥测** | 凭据本地存储，仅向对应服务端发送用于认证；自定义监视发送到用户配置的端点。没有分析、崩溃上报或遥测 |

日志在 `%APPDATA%\quotabar\spike.log`，落盘前过统一脱敏层。你要是还不放心——源码就在这儿，编译它。

## 工作原理

- **轮询**：每家一个独立任务，启动即取数，然后按上表节奏；自定义间隔为 1–1440 分钟，保存后唤醒任务重新读取配置；限流冷却中的任务仍等待到期。尚未实现专门的系统唤醒错峰调度
- **任务状态**：由工具钩子主动上报到本地事件文件，QuotaBar 每轮间隔 750 毫秒读取，再刷新面板；不靠 CPU 高低或窗口动静猜它有没有在干活。只保留工具、会话、项目目录名、状态和时间等元数据，不保存提示词或聊天正文
- **失败隔离**：平台请求/解析异常只影响该行，Rust unwind panic 转为错误并继续重试；历史数据库打不开时跳过历史记录，下次再尝试。无有效数据时迷你条显示灰色「—」，部分平台不可用时有效汇总附带「*」
- **429 退避**：额度 HTTP 429 保留 Retry-After（秒数或 HTTP 日期）；本地指数退避基数封顶 8 倍周期并加 ±20% 抖动，最终等待不少于配置周期与服务端要求。手动刷新不会绕过当前限流冷却。OAuth 刷新端点的 429 目前仍按网络失败处理
- **宽容解析**：接口字段缺了、类型变了，能解就解；解不了就老实说「接口变更，请检查更新」，而不是显示一堆 NaN
- **OAuth 写回六步协议**：每轮在单飞锁内重读 → 临过期 5 分钟刷新 → 写前比较 → 原子改名重试 6 次。写回失败时在进程内保留完整的新 token 对，下轮先重试写回；CLI 文件发生有效变更或删除时放弃待写回数据。进程退出后内存状态不保留；跨进程写前比较仍不是原子 CAS

## 自定义监视引擎（开放框架）

设置 → 自定义监视 → 四个框：

- 端点 URL（一个 GET 返回 JSON 额度）
- 认证头名 + 前缀（如 `Authorization` + `Bearer `）
- 窗口映射：`标签 | used 路径 | limit 路径或数字 | reset 路径(可选) | invert(可选)`，点语法 `data.usage.used`，支持数组下标
- 轮询间隔（1–1440 分钟，超出范围自动限制）

reset 字段自动识别 epoch 秒/毫秒/RFC3339。接进来就和内置五家同等待遇：同样的渲染、同样的告警、同样的失败隔离。

两个进阶能力（为官方余额类接口准备）：

- **limit 填数字**：只报「已用/余额」不报总额的接口（如余额 API），limit 直接写参考额度（如 `100`）
- **invert 低水位模式**：行尾第 5 段加 `invert`，百分比反转——余额越剩越少，条越红，低于参考额 10% 触发 toast（和普通额度的「快用完」同一套告警）

设置页内置**官方接口模板**（一键填表，key 自己粘）：Moonshot 开放平台余额（国内 `api.moonshot.cn` / 国际 `api.moonshot.ai`，官方文档接口，`GET /v1/users/me/balance`）。注意两站的 key 不通用。

## 卸载与清理

天下没有不散的额度。用过本机任务接入的，先在对应工具中移除 QuotaBar 插件，或在 QuotaBar 设置中移除自己安装的用户钩子，再删除程序，免得工具继续调用一个已经搬走的邻居。

其余本地数据按需清理：

- 设置与状态：`%APPDATA%\quotabar\settings.json`（macOS：`~/Library/Application Support/quotabar/`）
- 日志：同目录下 `spike.log`；用量历史：同目录下 `history.db`
- 手动 key：Windows 凭据管理器（服务名 `quotabar`）/ macOS Keychain
- 开机自启：设置里关（注册表 `HKCU\...\Run\QuotaBar`）

删完这些就干净了。Antigravity 的凭据条目我们不动——那是人家的东西，借读已是承情。

## FAQ

**Q: 数字准吗？**
悬浮条 tooltip 里标了来源。Kimi 对照 CLI `/usage`，Codex 对照 CodexBar，Gemini 对照 Antigravity 面板（同源数据，一个字不差）。轮询有间隔，几分钟内的差异属于物理学。

**Q: 任务开始了，面板为什么没马上动？**
先确认钩子已审核信任，再确认工具已加载新配置。750 毫秒只是本地读取间隔，还要加上工具调度和接收程序启动时间，不是从点「发送」到显示的总延迟。持续不更新时，去设置里检查接入；该检查只核对默认用户配置，不能代替插件信任审核。[排查说明](docs/task-monitor.md#codex-已安装但没有状态)。

**Q:「本轮已停止」就是做完了吗？为什么任务名看着不对？**
停笔不等于交卷。「本轮已停止」只表示这一轮停了，不证明目标完成，也不代表计划任务以后不再运行。列表里的「项目」目前是工作目录名，不是 ChatGPT 侧栏的任务标题；真实标题和后续调度信息还没接入。见 [显示规则](docs/task-display-semantics.md)。

**Q: 杀软报毒？**
未签名 + 读凭据文件，启发式引擎难免紧张。Defender 实测通过。遇到报毒：对 `checksums.txt`，或者自己从源码编一个——这是最彻底的信任。

**Q: 怎么更新？**
右键 → 检查更新。有新版本时悬浮条顶部出黄色横幅，可以「跳过此版本」。绿色 exe 下载替换即可，安装包覆盖安装保留设置。

**Q: Claude 为什么 10 分钟才刷一次？**
因为它的用量接口是非官方的、频控敏感的。慢是功能，不是 bug。

**Q: Gemini 显示「Antigravity 未运行」？**
打开 Antigravity IDE 就行。它读的是 IDE 本地语言服务器的实时数据，IDE 不在线就没有可信数字——我们选择诚实，不给你编一个。

**Q: 迷你模式右边那些话是什么？**
程序员段子库 + 时段关怀。深夜会劝你睡觉，周五下午会劝你别开新坑。别嫌弃，它比你的项目经理关心你。

## 它不是什么（非目标）

为防止范围蔓延（和自己的手贱），以下永远不做：

- 内置遥测/崩溃上报
- 账号切换器（那是 cc-switch 的地盘）
- Web 版 / 浏览器扩展
- 凭据跨工具双向同步

## 请作者喝咖啡

如果这个工具帮你躲过了一次「心流被 429 掐死」的绝望瞬间，可以考虑请作者喝杯咖啡（悬浮条右键菜单里也有入口）：

<!-- 赞赏码唯一事实源：src/sponsor.jpg（微信赞赏码，已裁剪至码本体）。
     更换时同步更新 src-tauri/src/main.rs 测试里的 EXPECTED_SPONSOR_SHA256，
     否则 cargo test 会红（这是防打包遗漏/防偷换的钉子）。 -->
<p align="center">
  <img src="src/sponsor.jpg" width="280" alt="微信赞赏码" />
</p>

<p align="center"><i>给码农买杯咖啡，是他的福报。</i></p>

完全自愿，不给也能用全部功能——开源软件不兴赎金那一套。

---

## Contributing

欢迎 issue 和 PR。两条规矩：

1. **提交前检查**：在 `src-tauri` 目录运行 `cargo test --locked`，在仓库根目录运行 `node --test tests/*.test.cjs`。
2. **发版用 `release.ps1`**：`powershell -File release.ps1 patch`（bump → 测试 → tag → CI 一条龙，别手工同步版本号，会乱）

License: [MIT](LICENSE)
