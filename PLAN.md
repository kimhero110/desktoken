<!-- /autoplan restore point: C:\Users\Administrator/.gstack/projects/desktoken/no-branch-autoplan-restore-20260901-223911.md -->
# DeskToken — 多平台大模型额度桌面悬浮条

> 状态：/autoplan 评审完成，待最终审批 | 版本：v2（经 CEO/设计/工程/DX 四轮评审修订） | 日期：2026-09-01

## 1. 产品目标

一个常驻 Windows 桌面的迷你悬浮条，实时显示用户各 AI 编程订阅的额度状态：**5 小时滚动窗口用量、每周用量、距离重置时间**。解决"不知道自己还剩多少额度、什么时候重置，不敢放开用"的痛点。

### 目标用户
同时订阅多家 AI 编程服务（Claude、Kimi、Codex、GLM、Gemini……）的开发者。主 persona：中国大陆中文开发者（Windows，多订阅重度用户）；次 persona：国际用户。

### 核心价值
- 一眼看到所有订阅的剩余额度，不用逐个打开网页/CLI 查
- 重置倒计时帮助规划使用节奏
- 接近用尽时颜色告警 + toast + actionable 建议（"GLM 周用量仅 33%，或 18 分钟后重置"），避免突然被限流打断工作

### 已定调（用户拍板，2026-09-01）
- **定位**：个人工具，开源发布（GitHub）。不购签名证书；SmartScreen/杀软误报用 README 截图级文档化；版本检查做最小版（启动时查 Release + 横幅，并发双查 GitHub+镜像）
- **形态**：悬浮条为主 + 系统托盘
- **需求**：跳过 M0 验证，直接进入 M1
- **ToS**：README 与首次启动对话框逐字明示风险（非官方端点、封号风险、凭据写回行为），用户自负；Claude 轮询 ≥10min；响应头探测默认关闭需手动开启+一次性确认
- **开放框架（M1 后追加拍板）**：产品不止 5 家内置 provider——做**自定义监视引擎**：用户通过设置界面配置任意平台的"端点 URL + 认证头 + JSON 路径映射（used/limit/reset）+ 轮询间隔"即可接入新平台，无需改代码。内置 5 家 = 该引擎的 preset + 少数需要 OAuth 刷新的复杂 provider 保留 Rust 原生实现

## 2. 产品形态与交互

详细像素级规格见「设计规格」一节。要点：
- 半透明深色背景（`rgba(18,18,22,0.72)`+acrylic）、圆角 10px、置顶、整面拖动（记住位置，work-area clamp）、**单击无行为**、永不抢焦点（WS_EX_NOACTIVATE，须在 show() 后应用）
- ~~鼠标穿透~~ → **迷你模式**（Spike A 结论：真穿透在 WebView2 子窗口架构下五种方案全部失败——样式位变灰、子类化收不到命中测试、EnableWindow 吞点击；用户拍板降级为 280×22 状态细条，保留拖动/右键，托盘可恢复）
- 进度条颜色：绿（<70%）→ 黄（70-90%，加斜纹色盲通道）→ 红（>90%）；**整条边框环境告警**（任一窗口 ≥70% 边框转黄、≥90% 转红，余光 pop-out）
- `%` 语义 = 已用百分比；倒计时三档格式（43m / 2.1h / 3d4h / —）；无 emoji、无循环动画
- 状态点 + freshness 一级显示（"3 分钟前"），staleness 三档（严重过期数值降透明度 50%）
- 右键菜单九项固定：立即刷新(30s 冷却) / ─ / 迷你模式 ✓ / ─ / 复制诊断信息 / 检查更新 / 在 GitHub 报告问题 / 设置 / ─ / 退出
- 系统托盘：穿透模式下的唯一操作入口；toast 到达时托盘图标变色 + 窗口边框闪一次
- 设置窗口：独立 480×560，live-apply，provider 卡片四态，key 验证按钮（真实请求），key 打码+眼睛切换
- 首启 ToS 模态对话框（同意前零网络请求，含版本检查）→ 同意后自动发现凭据、**默认启用已发现的 provider** + 3s 通知 + 立即 fetch（magical moment）；空状态仅在零检测时出现

## 3. 数据通道（已调研验证，2026-09）

| 平台 | 接口 | 凭据来源 | 稳定性 |
|---|---|---|---|
| Kimi | `GET api.kimi.com/coding/v1/usages` | 自动读 `~/.kimi-code/credentials/kimi-code.json`；或用户粘 Console API key | ✅ Kimi CLI 自用；轮询 2min |
| GLM | `GET open.bigmodel.cn/api/monitor/usage/quota/limit`（国际版 `api.z.ai`） | 用户粘 API key（**裸 key，无 Bearer 前缀**；M2 开工前查证必须走 header 禁 query）；验证时自动试双端点并记住命中 | ✅ 智谱官方插件在用；轮询 2min |
| Codex | `GET chatgpt.com/backend-api/wham/usage` | 自动读 `~/.codex/auth.json`（OAuth，需刷新） | ✅ Codex CLI 自用，零额度消耗；轮询 2min |
| Claude | `GET api.anthropic.com/api/oauth/usage`；降级①：1-token Haiku 请求读 `anthropic-ratelimit-unified-*` 响应头（默认关，设置可开，每次耗 1 token）；降级②：用户粘 sessionKey（高级选项 + README 图文指引） | 自动读 `~/.claude/.credentials.json`（OAuth，刷新并写回） | ⚠️ 非官方 + 429 频控；轮询 ≥10min（设置钳制下限） |
| Gemini | `POST cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota`（先 loadCodeAssist 拿 project） | 自动读 `~/.gemini/oauth_creds.json`（OAuth 刷新） | ⚠️ 按模型分桶的日额度，非 5h/周；个人版 2026-06 起迁移 Antigravity，403 降级提示 + 设置可停用；轮询 5min（设置钳制下限） |

### 关键实现要点
- **OAuth 刷新 6 步并发协议**（详见工程评审 #3）：每轮重读 → 临过期 5min 才刷新 → per-path Mutex 单飞+mtime 检查 → refresh → **compare-before-write**（写回前重读比对，已变则丢弃自己的 pair）→ rename 重试×6 / PendingWriteback 内存暂存。无 `.lock` 文件（会卡死官方 CLI 的 mkdir 目录锁——已 CUT）
- **凭据只读优先**：写回唯一通道 credentials.rs；手动 key 只进 keyring（≤2560B 约束，绝不存 OAuth blob）
- **Reference Implementations 绑定**：Claude→ClaudeBar/Claude Usage Tracker；Codex→CodexBar/headroom；Kimi→kimi-code-hud；GLM→cc-switch/opencode-glm-quota；Gemini→CodexBar GeminiStatusProbe；Rust 凭据文件读写模式→cc-switch。M3 验收加"与 CodexBar 同账号输出比对一致"
- **信任透明**：tooltip 固定显示"来源：官方接口/响应头估算/手动 key"（估算来源数值旁标"估算"）+ 套餐名 + "打开官方用量页面"链接

## 4. 技术架构

### 技术栈
- **Tauri 2 + Rust**：NSIS 安装器（内嵌 WebView2 bootstrapper）+ 绿色 exe 双产物；内存实际 60–120MB（WebView2 主导，README 不吹"极低"）
- **前端**：原生 HTML/CSS/JS（不引框架），事件契约 `quota://snapshot` / `quota://error`（payload 带 `v:1` 版本字段）
- **凭据存储**：`keyring` crate（Windows 凭据管理器）存手动 key；不可用时给"本次会话暂存（不保存）"选项，不静默降级明文
- **HTTP**：reqwest（rustls-tls）；connect 5s / total 15s 硬超时；响应体预检+1MB 截断；429 honor Retry-After + ±20% jitter + 封顶 8×周期/30min；退避状态持久化

### 模块结构
```
desktoken/
├── src-tauri/
│   └── src/
│       ├── main.rs          # 窗口生命周期/托盘/拖动/置顶/NOACTIVATE/位置 clamp
│       ├── poller.rs        # supervisor：每 provider 一 task，独立退避，spawn 吃 panic
│       ├── credentials.rs   # 凭据发现/读取/刷新/写回（唯一写回点）；CredState 状态机
│       ├── settings.rs      # JSON 原子写+损坏回退；Arc<RwLock<Settings>>+change 事件
│       ├── diagnostics.rs   # tracing 滚动日志 + 统一脱敏 Layer + 复制诊断
│       ├── updater_check.rs # ToS-gated；GitHub+镜像并发双查；5s 静默失败；缓存 24h
│       └── providers/
│           ├── mod.rs       # Provider trait + ProviderError + sanitize() + 注册表
│           ├── kimi.rs / glm.rs / codex.rs / claude.rs / gemini.rs
└── src/
    └── index.html           # 悬浮条 26 态渲染器；30s countdown tick + time-jump 检测
```

### 核心抽象
```rust
trait Provider {
    fn id(&self) -> &str;
    fn display_name(&self) -> &str;
    fn discover_credential(&self) -> Option<Credential>;
    async fn fetch(&self, cred: Credential, cred_mgr: &CredentialManager)
        -> Result<(QuotaSnapshot, Option<Credential>)>;  // 新 token 经此返回
}

enum ProviderError {  // 26 态 UI 的驱动源；映射表见设计规格
    RateLimited { retry_after: Option<u64> }, AuthExpired, CredentialMissing,
    CredentialCorrupt { torn: bool }, ParseFailed, Network, UnsupportedClient,
}

struct QuotaSnapshot {
    plan: Option<String>,        // 套餐名："Pro"/"plus"/...（tooltip 信任信号）
    windows: Vec<QuotaWindow>,   // 1~N 个额度窗口（>8 钳制，超出进 tooltip）
    source: QuotaSource,         // Official / HeaderEstimate / ManualKey
    fetched_at: DateTime<Utc>,
}

struct QuotaWindow {
    kind: WindowKind,            // FiveHour/Weekly/Daily/Monthly
    label: String,               // 受控词表："5h"/"周"/"今日"/"月"/"Pro 日"（≤4 字符）
    used_percent: f32,           // 钳 0-100
    resets_at: Option<DateTime<Utc>>,
}
```

**加新平台的两条路**（开放框架拍板后修订）：
1. **自定义 provider（零代码）**：设置界面填端点/认证头/JSON 路径映射/轮询间隔 → `CustomProvider` 存入 settings，key 进 keyring，与内置 provider 同渲染。适用于"一个 GET 返回 JSON 额度"的简单平台。
2. **Rust 原生 provider**：仅当需要 OAuth 刷新/多步握手/降级链（Claude/Codex/Gemini 这类）。加一个 provider 文件 + 注册一行。

路径映射用 jq-lite 点语法（`data.limits.0.percentage`，支持数组下标）；reset 字段支持 epoch 秒/毫秒与 RFC3339 自动识别。自定义 provider 与内置 provider 走同一个 `QuotaSnapshot` 事件契约与 sanitize 管线。

### 设计原则
- **容错解析 + 统一消毒**：宽容解析（字段可缺、字符串/数字混用）+ providers/mod.rs 公共 `sanitize()`（windows ≤8、字符串 ≤256、percent 钳 0-100、NaN fixture 钉死），provider 不可绕过
- **失败隔离**：provider panic 只死 JoinHandle，行灰 + 日志；一家挂了不影响其他家
- **轮询永不暂停**（隐藏/穿透只停 UI 渲染——隐藏时恰是 toast 价值时刻）；启动/启用时错峰全量 fetch；唤醒靠前端 time-jump 检测（>60s 跳变→错峰全量刷新），interval 用 MissedTickBehavior::Delay 防补发风暴
- **阻塞隔离**：keyring/文件 I/O 全走 `spawn_blocking` / `tokio::fs`
- **torn read**：解析失败静默重试 3 次（150ms）；连续 3 周期失败才上报，文案区分"读取冲突（自动恢复）"与"确实损坏"

## 5. 安全考虑

- 所有凭据仅存本地；手动 key 走 OS 凭据管理器；**keyring 条目 ≤2560B，禁存 OAuth blob**
- 不向外发送任何用户数据，只请求各平台官方域名；**永无 telemetry/崩溃上报**（诊断靠 E6 复制诊断信息）
- **日志脱敏统一在 tracing Layer**（不靠 provider 自觉）：`Bearer\s+\S+`、`sk-\S+`、JSON token 值、已知 key 字面替换；诊断导出 `%USERPROFILE%`→`~`、key 只显示前后各 3 字符；**配套泄漏断言测试**
- OAuth 写回：6 步协议（含 compare-before-write）；rename 后 ACL 继承差异已知（re-apply 留 TODOS）
- 杀软误报对策：凭据访问集中 credentials.rs 单模块+注释；CI 产出 checksums.txt + build provenance attestation；README 置顶报毒说明+自构建指引；M5 实测 Defender+两款国产杀软
- 设置界面 key 默认打码；toast 文案不含任何凭据信息

## 6. 里程碑

| 阶段 | 内容 | 验收标准 |
|---|---|---|
| M1 ✅ | Tauri 窗口骨架 + **spike A**（NOACTIVATE+acrylic+transparent+drag 四件套同框；记事本打字中拖动/右键不丢焦点；ToS 对话框临时激活）+ settings + 托盘 | 悬浮条可拖动、半透明、置顶、不抢焦点 |
| M2 ✅ | Provider 框架（trait+ProviderError+sanitize）+ Kimi + GLM（glaze 双端点自动试） | 两家真实数据显示；设置页粘 key+验证；每 provider ≥3 golden fixture（含 1 畸形）+ wiremock 4 场景通过 |
| M3 ✅ | OAuth 公共路径（6 步协议，oauth.rs）+ Codex + Claude + **spike B**（CLI 读写对抗：单飞/compare-before-write/invalid_grant 重读/rename 故障注入/原子写风暴） | 装了 CLI 的机器自动出数据；refresh 状态机每边一测+rename 故障注入；与 CodexBar 输出比对一致（CodexBar 未装，字段级一致） |
| M4 ✅ | Gemini（双通道：gemini-cli 经典流 + Antigravity 凭据管理器只读 + daily fetchAvailableModels；403 降级提示） | 五家全部可用（真机：Kimi/GLM/Codex/Gemini 出数，Claude 待登录） |
| M5 | 打磨+分发：告警色/ toast（AUMID+实机实测）/ 右键菜单 / 开机自启 / GitHub Actions tauri-action NSIS 管线 + checksums + provenance / README 十章 / 杀软实测 / 72h soak（内存增量 <50MB）/ 15 项手工 QA 清单 | 可日常使用的发布物 |

## 7. 风险与对策

| 风险 | 概率 | 对策 |
|---|---|---|
| Claude oauth/usage 接口被限流/关闭 | 高 | 三级降级链；轮询 ≥10min 钳制；探测默认关 |
| 非官方接口结构变更 | 中 | 宽容解析+sanitize；解析失败文案引导检查更新；E1 并发双查版本检查 |
| OAuth 刷新竞争顶掉官方 CLI 会话 | 中 | 6 步协议（compare-before-write + PendingWriteback）；单实例锁 |
| Gemini 个人版 403（迁移 Antigravity） | 已发生 | 提示+可停用；Antigravity provider 进 TODOS |
| 用户没装对应 CLI 也无 API key | 中 | 空状态 onboarding + 设置卡片"去哪拿 key"直达链接 |
| 平台反滥用/封号（ToS） | 低-中 | 明示告知用户自负；保守轮询钳制；探测可关 |
| 杀软/SmartScreen 拦截 | 高 | README 截图级绕过+报毒说明+checksums+provenance；M5 实测 |
| 国内下载/版本检查不可达 | 高 | 下载双通道（镜像+Gitee 附件）；版本检查并发双查+自定义源兜底 |

## 8. 非目标（v1 不做）

- 移动端 / macOS / Linux（架构预留，先只做 Windows）
- 用量历史图表、用量预测（E8/E11 进 TODOS）
- 多账号、云同步设置
- Antigravity、Qwen、DeepSeek 等新 provider（留扩展口）
- 浅色主题 v1、折叠/迷你模式、自绘 tooltip、provider 拖拽排序
- toast 点击交互（插件不支持，已 CUT）；`.lock` 文件锁（有害，已 CUT）；WM_POWERBROADCAST FFI（time-jump 替代）
- **内置 telemetry/崩溃上报（永不加）**；重造 cc-switch 核心域；Web 版/浏览器扩展；凭据跨工具双向同步；完整签名 updater（TODOS）

---

# CEO 评审结果（Phase 1，/autoplan，SELECTIVE EXPANSION）

> 声音：Claude 主评审 + Claude 独立声音；Codex CLI 本机不可用 → 单模型模式。
> 共识表（6 维）：前提有效=DISAGREE(有条件) / 问题正确=DISAGREE(形态存疑，用户已拍板维持) / 范围校准=CONFIRMED / 备选充分=DISAGREE(已补四案对比) / 竞争风险=DISAGREE(已补 cc-switch 分析) / 6 月轨迹=DISAGREE(已补更新通道)。

## 前提清单（Premise Registry）

| # | 前提 | 错了概率 | 错了会怎样 |
|---|------|---------|-----------|
| P1 | 五家非官方接口 12 个月内可持续宽容解析 | 高（Claude/Gemini 几乎必变一次） | 该行变灰；靠 E1 版本检查通道续命 |
| P2 | 用户机器已装官方 CLI 且已登录 | 中 | Claude/Codex/Gemini 无手动 key 兜底（sessionKey 为高级例外）；未装 CLI = 该行不出现 |
| P3 | OAuth refresh 写回与官方 CLI 并发可控 | 中高 | 6 步协议+compare-before-write 封死；spike B 实证 |
| P4 | 同时订阅 ≥3 家的 Windows 开发者足够多 | 未验证（用户拍板跳过 M0） | 个人工具定位下不影响自用价值 |
| P5 | "5h+周"两行能统一表达所有 provider | 部分必然错 | QuotaWindow 异构抽象 + label 受控词表已解 |
| P6 | 轮询非官方端点不触发平台反滥用/封号 | 低-中 | 明示告知用户自负；保守轮询钳制 |

## 备选方案对比（0C-bis）

| | A. Tauri 2+Rust（选定） | B. Electron | C. Fork/共建 cc-switch | D. statusline 插件 |
|---|---|---|---|---|
| 工作量 | M-L | M | S-M | S |
| 风险 | Rust 窗口边角（DPI/多屏）；provider 逻辑需翻译 | 150MB/几百 MB 内存与轻量常驻定位冲突 | 上游路线图不受控；定位是"切换"非"监控" | 聚合价值消失；依赖终端开着 |
| 复用 | 概念参考+翻译 OSS 逻辑 | 代码级复用 JS 项目 | 代码+坑位双重复用 | — |

结论：坚持 A；reference implementations 绑定已写入 §3（翻译已验证逻辑，M3 风险减半）。statusline 形态挑战经前提门由用户拍板维持悬浮条。

## 接受的扩张（E1–E7）

E1 版本检查（最小版：启动并发双查+横幅；签名 updater 延期）/ E2 托盘（修穿透退出死锁）/ E3 单实例锁 / E4 空状态 onboarding（DX 升级为自动启用）/ E5 Toast（仅 90%+重置两类，滞回去抖）/ E6 诊断+本地日志（统一脱敏）/ E7 启动+启用即全量 fetch。
延期：E8 历史、E9 cc-switch 联动、E10 插件文档、E11 预测 → TODOS.md。

## Error & Rescue Registry（关键行）

| 路径 | 故障 | 用户看到 | 恢复 |
|---|---|---|---|
| 任意 fetch | 离线/超时/5xx/429 | 灰行或 freshness 变黄"限流中·N分后重试" | 自动重试；time-jump 唤醒刷新；退避持久化 |
| 任意 fetch | 结构漂移→解析失败 | "接口变更 · 右键检查更新" | E1 版本检查 + E6 诊断 + 置顶 issue |
| discover | 凭据缺失/损坏 | 空状态引导 / "凭据损坏请重新登录 X CLI"+可复制命令 | torn read 3 次重试防误报 |
| refresh | 轮转竞争 invalid_grant | 行灰 | 6 步协议（先重读再判定） |
| refresh 写回 | Windows 文件锁 | 无感（内存暂存） | rename 重试×6 + PendingWriteback |
| keyring | 凭据管理器不可用 | "无法安全存储 key" + 本次会话暂存选项 | 不降级明文 |
| Gemini | 403 Antigravity | "已迁移，可在设置中停用" | 等后续 provider |

## CEO 完成摘要
评分 7/10。优势：数据通道调研前置完成、QuotaWindow 异构抽象、失败隔离+诚实 UI 内建。风险 Top3：①refresh 竞争（已封）②接口漂移×更新通道（已补）③Claude 通道结构性脆弱（最痛的一家恰是数据最差的一家——差异化落在"Windows+国产三家做得最深"）。

---

# 设计规格（Phase 2，/autoplan 设计评审产出）

> 声音：Claude 主评审（4/10→8/10）+ Claude 独立声音。Codex N/A。
> 冲突 2 项（密度、深浅色）按 P5/P1 自动决策，标记 taste 留最终门。

## Design Tokens（写死，实现者零自由发挥）

- 背景：`rgba(18,18,22,0.72)` + acrylic blur；blur 不可用时 opacity 0.88 保底；设置透明度下限 60%
- 圆角：窗口 10px，进度条 3px；1px 边框 `rgba(255,255,255,0.08)`；无 box-shadow
- 字体栈：`"Segoe UI Variable Text", "Segoe UI", sans-serif`；数字 `tabular-nums`
- 字号三级：12px/600（provider 名）、11px/400（% 与倒计时）、10px/400（标签/次级 `rgba(255,255,255,0.55)`）
- 告警色：green `#3FB950` / yellow `#D29922` / red `#F85149` / stale `#8B949E`
- 进度条：高 6px，track `rgba(255,255,255,0.12)`；黄色档加斜纹（色盲第二通道）
- **v1 深色固定**（taste #T1 留最终门）；无 emoji；无循环动画；填充变化 300ms ease-out；变色 150ms；倒计时文本 30s 刷新

## 布局几何（280px，逻辑像素）

```
容器：宽 280 固定（设置可调 240–360 步进 20），padding 10/8，块间距 10px（留白即分隔）

Provider header row (18px):
[● 8px 状态点] 6px [Name 12px/600]  flex  [freshness 10px grey 右对齐]

Window row (16px, 行间 4px):  ← 行数 = windows.length，前端零 provider 感知
[label 44px 10px grey] 6px [bar flex] 6px [pct 32px 右对齐] 6px [countdown 44px 右对齐 grey]
```

- label = 后端受控词表（`5h`/`周`/`今日`/`月`/`Pro 日`，≤4 字符）；Gemini 分桶 >2 时只显示最差的 + tooltip 列全部
- `%` = 已用百分比（高=红=危险），tooltip 给剩余；倒计时 `<1h→43m` / `<24h→2.1h` / `≥24h→3d4h` / null→`—`；resets_at 过期未回落显示"等待刷新…"（禁 0:00/负数）
- provider 排序 = 勾选顺序，禁自动重排

## 密度与整条告警（taste #T2 留最终门）

- 自动决策：v1 固定展开显示所有启用 provider，不做折叠/hover 展开；**整条环境告警**：任一窗口 ≥70% 边框转黄、≥90% 转红（余光 pop-out；条内颜色降级为"确认是谁"）
- 被拒绝方案留 TODOS v2：默认单行摘要"最差窗口"+hover 三级展开

## 状态清单（26 态关键规格）

| 状态 | 规格 |
|---|---|
| 首启 ToS | 模态（创建时临时允许激活，关闭恢复 NOACTIVATE）；逐字文案：①"DeskToken 使用本机官方 CLI 的登录凭据，轮询各平台非官方用量接口，并自动读取已检测到的 CLI 凭据"②"这可能违反平台服务条款，理论上存在账号被限制的风险（保守轮询已降低该风险）"③"token 过期时会自动刷新并写回凭据文件（与官方 CLI 行为一致）。所有数据仅存本机，绝不上传。继续即表示你了解并接受"；同意前零网络请求（含版本检查） |
| 加载 | 每 provider 独立三态；空 track+`--`+"获取中…"，禁 spinner；部分加载不等齐 |
| 空状态 | "还没有配置任何平台" + "未检测到 CLI 凭据" + 主按钮"打开设置"（自动启用后此为纯兜底） |
| 自动启用通知 | 顶部 3s 临时条："已自动启用 Claude · Codex，可在设置中关闭"（复用穿透提示模式） |
| 错误行 | 单行 ≤28 字符 ellipsis；tooltip=problem+cause+fix+动作（映射表见下）；恢复时绿色对勾闪 2s；首次出现错误行一次性 hint"右键查看选项" |
| 429 backoff | 行不置灰；freshness 变黄"限流中·N分后重试" |
| 全离线 | 顶部"已离线 · 最后更新 14:32"；各行 opacity 0.5 |
| staleness 三档 | ≤1.5×间隔正常；≤4× 圆点灰+时间戳；>4× 数值 opacity 降 50% |
| 版本横幅 | 顶部 22px 黄底："有新版本 v1.3 · 查看 · 跳过此版本 · ×"；tooltip 带镜像链接；查看→release 页 |
| Toast | Windows 原生 toast；滞回：<90→≥90 触发、<85 重置、同窗口重置前只报一次（持久化去重）；到达时窗口边框闪一次+托盘图标变色（**无点击交互**，插件不支持） |
| 重置瞬间 | "等待刷新…"→300ms 过渡归零+该行闪绿+toast（文案可有人格："Claude 已重置，放开用"） |
| ≥90% actionable | tooltip 追加建议行："GLM 周用量仅 33%，或 18 分钟后 Claude 重置"（零新数据通道） |
| 穿透模式 | 开启 3s 提示"已开启穿透 · 托盘图标可退出"；菜单 checkmark+hint |
| 位置恢复 | 逻辑像素+显示器名+归一化坐标持久化；启动/定时懒 clamp 到 work area，越界回退主屏右上角 (16,16) |
| 双实例 | 二次启动：取消穿透+边框闪一次，不抢键盘焦点，静默退出 |

## 错误文案映射表（ProviderError → 人话）

| ProviderError | 行内（≤28 字符） | tooltip（problem+cause+fix+动作） |
|---|---|---|
| AuthExpired | "凭据失效，请重新登录" | 附可复制命令（`claude`/`codex login`/`gemini`/`kimi login`）；Claude 行追加"或在设置开启响应头探测（每次约耗 1 token）" |
| ParseFailed | "接口变更 · 右键检查更新" | "接口可能已变更；检查更新/复制诊断信息/在 GitHub 报告问题"；链置顶 issue |
| RateLimited | "限流中·N分后重试" | 自动恢复，无需操作 |
| Network | "无法连接 <域名>" | "检查网络/代理；自动重试中" |
| CredentialMissing | （该行不出现，进设置引导） | — |
| CredentialCorrupt{torn} | （静默重试，旧数据照常） | 连续 3 周期后："凭据损坏，请重新登录 X CLI"+命令 |
| UnsupportedClient | "已迁移 Antigravity" | "暂不支持，可在设置中停用 Gemini · 关注更新" |

## 交互规格

- 拖动：整面（root mousedown→startDragging），无 grip；**单击无行为**
- hover tooltip（400ms，原生 title）：`Claude · 5h 窗口 / 已用 42%（剩余约 58%）/ 重置: 今天 17:30（2h6m 后）/ 更新于 14:32 · 下次轮询 14:42 / 来源: 官方接口 · Pro`；≥90% 追加建议行
- 右键菜单九项固定（见 §2）
- 设置窗口：独立 480×560 非置顶非模态；live-apply；provider 卡片四态（未配置引导/已配置-自动显示来源路径+"断开"/已配置-手动 key 打码+眼睛/验证失败+重测）；key 粘贴规范化（trim+剥离 Bearer）；GLM 验证自动试双端点；轮询间隔钳制（Claude≥10min、Gemini≥5min，就地说明）；响应头探测首次开启一次性确认；"重置外观"
- 焦点：widget `WS_EX_NOACTIVATE` 不进 Tab 链；每行 `aria-label`；设置窗口全键盘可达；菜单不抢焦（必要时原生 TPM_NONOTIFY）

## 设计完成摘要
4/10 → 8/10。新增非目标：折叠/迷你模式、浅色主题 v1、自绘 tooltip、拖拽排序、趋势预警。"75% 预警"提法已删（依赖延期的 E8；告警=70/90 静态阈值+toast 90%）。

---

# 工程评审结果（Phase 3，/autoplan，FULL_REVIEW）

> 声音：Claude 主评审（7/10→8.5/10，20 findings）+ Claude 独立声音（2 critical/8 high）。Codex N/A。
> 共识表：架构=CONFIRMED(trait 升级) / 测试=共识(补 wiremock 层) / 性能=CONFIRMED / 安全=CONFIRMED(补 sanitize+脱敏具体化) / 错误路径=共识(补 compare-before-write) / 部署=CONFIRMED(补 WebView2/杀软/分发管线)。

## 关键裁决（16 项，已全部回写正文）

1. **CUT toast 点击交互**（P0 可行性）：插件不支持回调 → toast 仅告知+边框闪+托盘变色
2. **CUT `.lock` sidecar**（P0 有害）：会卡死官方 CLI 的 mkdir 目录锁 → 单实例+per-path Mutex+6 步协议
3. **6 步 refresh 协议**：每轮重读 → 临过期 5min 才刷新 → Mutex 内重读+mtime 检查 → refresh → **compare-before-write**（rename 前重读比对 mtime+size+hash，已变则丢弃自己的 pair 采用文件的）→ rename 重试 6 次(100ms×2^n, ~3s)，全失败 → `CredState::PendingWriteback`（内存暂存、本周期用、每轮重试、退出 500ms flush、绝不落第三副本）。invalid_grant→先重读再判定。锁序：仅一类锁单任务最多持一把→死锁构造上不可能
4. **trait 升级**：owned Credential + 返回 `Option<Credential>` + ProviderError taxonomy
5. **HTTP 规格**：5s/15s 超时、1MB 截断、Retry-After、退避持久化
6. **统一 sanitize()**：windows≤8、字符串≤256、percent 钳制、NaN fixture
7. **spawn_blocking 隔离** keyring/文件 I/O
8. **torn read**：3 次静默重试+3 周期才上报
9. **唤醒**：Delay 防补发+time-jump 检测+错峰 ≥2s；无电源 FFI
10. **脱敏 Layer 具体化**+泄漏断言测试；M2 前查证 GLM key 走 header
11. **E1 ToS-gated**+5s 静默+缓存 24h+并发双查+跳预发布
12. **NSIS 管线**（WebView2 bootstrapper）+绿色 exe 检测指引+README 内存校准
13. **Toast AUMID** 开始菜单快捷方式+Win10/11 实机实测
14. **杀软实测**两款国产+Defender+false-positive 报备通道
15. **小项**：立即刷新 30s 冷却、菜单不抢焦、二次启动行为、位置归一化坐标、事件契约 v:1
16. **Spike 先行**：M1 spike A（四件套+焦点+ToS 对话框激活）、M3 spike B（CLI 读写对抗+锁行为实证）

## 测试策略

golden fixture（每 provider ≥3+1 畸形）+ **wiremock 集成层 4 场景**（正常/429+Retry-After/超时/2GB 截断）+ refresh 状态机每边一测（含 rename 故障注入、CLI 先写回模拟）+ 脱敏泄漏断言 + toast 滞回单测 + 倒计时纯函数 + 虚拟时钟退避 + M5 72h soak（<50MB）+ 15 项手工 QA。Test plan artifact：`~/.gstack/projects/desktoken/desktoken-no-branch-test-plan-20260901.md`。

## 并行实施通道

Lane A（M1 骨架+spike A+settings+托盘）∥ Lane D（前端 26 态，mock 事件）→ Lane B（provider 框架+Kimi+GLM）→ Lane C（OAuth+Codex/Claude/Gemini+spike B）。

## 工程完成摘要
7/10→8.5/10。2am Friday 风险全部封死或兜底：refresh 竞态（协议）/ rename 连败（PendingWriteback）/ toast 哑弹（AUMID 实测）/ 丢窗（懒 clamp）/ acrylic 失效（0.88 兜底）。

---

# DX 评审结果（Phase 3.5，/autoplan，DX POLISH）

> 声音：Claude 主评审（4.5/10）+ 独立声音（5.5-6/10）。Codex N/A。
> 共识：运行时 DX 已 8 分；断点全在"下载→首值"前 10 分钟与升级闭环。修复后 7.5-8/10。

## 关键裁决（11 项，已全部回写正文）

1. 语言：v1 中文 UI + 中文 README 优先（顶部英文摘要）
2. **Magical moment 兑现**：ToS 同意→自动发现→默认启用→3s 通知→立即 fetch；"断开"逃生口
3. **下载双通道**（critical）：ghproxy 类加速链 + Gitee 同步 release 附件；E1 并发双查
4. **升级闭环**：NSIS 覆盖安装保留设置+退出运行实例（M5 手工 QA）；"跳过此版本"；绿色 exe=下载替换（FAQ）
5. **信任透明**：tooltip 来源/估算标记+套餐名+"打开官方用量页面"链接
6. **错误文案映射表**（进设计规格）
7. **设置人体工学**：key 规范化、GLM 双端点自动试、轮询钳制、探测确认、重置外观
8. **sessionKey 矛盾消除**：保留为高级兜底+README 图文指引
9. **README 十章规格**（M5 验收逐章节存在；含凭据行为清单：读取路径/唯一写回场景/触达域名/零遥测/可抓包验证）
10. **分发可信**：checksums.txt+build provenance；两款国产杀软实测；卸载器询问删除 key 与设置；右键"在 GitHub 报告问题"+bug_report.yml 模板
11. **首启 ToS**：进 M1 spike 验收；逐字文案含写回披露

## DX 完成摘要
TTHW：CLI 自动路径 5-15min→~4min（Competitive 头部）；手动 key 路径 ~9-12min→~6min。接受项：无 telemetry（原则）、社区低配（个人工具上限）。

---

## 梦想态 Delta

```
CURRENT                THIS PLAN (v1)                    12-MONTH IDEAL
5 个订阅 5 个查询入口   280px 悬浮条聚合 5 家             跨平台 + 历史趋势
被动挨打（限流才发现）  5h/周/日窗口 + 重置倒计时          + 用量预测 + 主动通知
                       颜色告警 + toast + actionable 建议  + 快打满一键切家（cc-switch 联动）
                       凭据自动发现零配置                  + provider 插件生态
                       失败隔离 + 诚实 UI                  + 自更新通道（接口漂移 48h 热修）
```

## 跨阶段主题（3 阶段独立命中 = 高置信信号）

1. **"接口漂移 × 更新/分发通道 = 生存器官"**：CEO(E1)、Eng(分发管线+国内可达性)、DX(下载镜像+双查) → 已合并
2. **"信任是产品"**：CEO(refresh 竞争)、Eng(compare-before-write/脱敏 Layer)、DX(来源透明/凭据清单/写回披露) → 已合并
3. **"规格缺失会复活已否决的设计"**：CEO(异构未兑现)、设计(mockup 未改)、DX(sessionKey 矛盾) → 教训：每个决策落为可执行规格，本计划已回写

---

<!-- AUTONOMOUS DECISION LOG -->
## Decision Audit Trail

| # | Phase | Decision | Classification | Principle | Rationale | Rejected |
|---|-------|----------|-----------|-----------|----------|----------|
| 1 | CEO | E2 托盘图标 INCLUDE | Mechanical | P1 | 修复穿透模式退出死锁 | 仅右键菜单 |
| 2 | CEO | E3 单实例锁 INCLUDE | Mechanical | P2 | 一个官方 plugin，消除双倍轮询/竞争 | 不做 |
| 3 | CEO | E7 启动即全量 fetch INCLUDE | Mechanical | P1 | 消灭首值空窗 | 等调度周期 |
| 4 | CEO | E4 空状态 onboarding INCLUDE | Mechanical | P1 | hour-1 留存决定点 | 仅设置页引导 |
| 5 | CEO | E6 诊断+本地日志 INCLUDE | Mechanical | P1 | 无 telemetry 产品的唯一报障通道 | 无日志 |
| 6 | CEO | E5 Toast 通知（90%+重置）INCLUDE | Taste(小) | P1 | 完成"避免被限流打断"闭环；裁为两类 | 全事件通知 |
| 7 | CEO | E1 最小版本检查 INCLUDE，签名 updater DEFER | Taste | P3 | 生存器官先做最小版；签名 infra 超 1 日 | 完整 updater 进 v1 |
| 8 | CEO | E8-E11 DEFER 至 TODOS.md | Mechanical | P3 | 超 v1 blast radius | 进 v1 |
| 9 | CEO | OAuth 刷新并发协议写入 §3 | Mechanical | P1 | 信任毁灭型 bug 的完整对策 | 仅原子写 |
| 10 | CEO | mockup/UI 原生表达异构窗口 | Mechanical | P5 | QuotaWindow 抽象的 UI 兑现 | 统一两行幻觉 |
| 11 | CEO | golden fixture 测试策略进里程碑 | Mechanical | P1 | 非官方 API 解析器必须 fixture 化 | 无测试 |
| 12 | CEO | Gemini 轮询 1→5min；去"实时"文案 | Mechanical | P3 | 降低反滥用暴露面 | 维持 1min |
| 13 | CEO | Reference implementations 绑定 §3 | Mechanical | P5 | 翻译已验证逻辑，M3 风险减半 | 从零逆向 |
| 14 | CEO | 新增 5 项非目标（telemetry 永不加等） | Mechanical | P4 | 防范围蔓延、守隐私定位 | — |
| 15 | CEO | DX 评审阶段启用 | Mechanical | P1 | TTHW/错误信息/文档维度适用 | 跳过 |
| 16 | CEO | 形态挑战（statusline vs 悬浮条）→ 用户 | Taste/User-dir | — | 挑战用户已选方向 | — |
| 17 | CEO | 定位（个人工具 vs 产品）→ 用户 | Taste/User-dir | — | 决定签名/ToS/更新承诺 | — |
| 18 | CEO | 需求验证 M0 → 用户 | Taste/User-dir | — | P4 未验证 | — |
| 19 | CEO | 定位 = 个人工具（开源发布）（用户拍板） | User Challenge 回应 | — | 跳过签名证书；最小版本检查；明示 ToS | 对外分发产品 |
| 20 | CEO | 形态 = 悬浮条+托盘（用户拍板，驳回 statusline） | User Challenge 回应 | — | 用户明确偏好常驻可见 | statusline 优先 |
| 21 | CEO | 跳过 M0 需求验证（用户拍板） | User Challenge 回应 | — | 自用工具不需 TAM 论证 | 48h 社区验证 |
| 22 | CEO | ToS 风险 = 明示告知、用户自负（用户拍板） | User Challenge 回应 | — | README+首启提示；保守轮询；探测可关 | 砍掉高风险 provider |
| 23 | Eng | CUT toast 点击交互，改边框闪+托盘变色 | Mechanical | P5 | 插件不支持点击回调（可行性） | 自绘窗口内 toast |
| 24 | Eng | CUT `.lock` sidecar 文件锁 | Mechanical | P5 | 会破坏 CLI 的 mkdir 目录锁 | 保留+TTL |
| 25 | Eng | refresh 协议加 compare-before-write | Mechanical | P1 | 封死 refresh 在途竞态 | 仅靠刷新前 mtime |
| 26 | Eng | 删"隐藏时暂停轮询"，轮询永不暂停 | Mechanical | P1 | 隐藏时恰是 toast 价值时刻 | 原优化 |
| 27 | Eng | trait 升级 + ProviderError taxonomy | Mechanical | P5 | 26 态 UI 需要驱动源；防 M3 返工 | 纯消费签名 |
| 28 | Eng | HTTP 规格（超时/截断/Retry-After/退避持久化） | Mechanical | P1 | 静默卡死是 staleness 救不了的死法 | 无规格 |
| 29 | Eng | 统一 sanitize() 响应消毒 | Mechanical | P1 | 恶意/异常响应防御 | 各 provider 自觉 |
| 30 | Eng | spawn_blocking 隔离 keyring/文件 I/O | Mechanical | P3 | 防 executor 污染 | 直接调 |
| 31 | Eng | torn read 重试 3 次+3 周期才报错 | Mechanical | P1 | 防误报"重新登录"制造真竞态 | 立即报错 |
| 32 | Eng | time-jump 检测替代电源 FFI | Mechanical | P5 | 零 FFI 覆盖同场景 | WM_POWERBROADCAST |
| 33 | Eng | 脱敏 Layer 具体化 + 泄漏断言测试 | Mechanical | P1 | 自觉必漏 | 一句话原则 |
| 34 | Eng | E1 版本检查 ToS-gated + 静默失败 + 缓存 24h | Mechanical | P1 | 修"同意前零网络"矛盾 | 启动即查 |
| 35 | Eng | NSIS 安装器 + WebView2 bootstrapper 进 M5 | Mechanical | P1 | 绿色 exe 在 LTSC/精简系统破产 | 纯绿色 exe |
| 36 | Eng | Toast AUMID + 杀软实测进 M5 验收 | Mechanical | P1 | E5 可能整个是哑的 | 假设 plugin 处理 |
| 37 | Eng | M1 spike A / M3 spike B 先行 | Mechanical | P1 | 四件套与锁行为必须实证 | 直接写实现 |
| 38 | Eng | 立即刷新 30s 冷却 + 菜单不抢焦等 6 小项 | Mechanical | P3 | 防 Claude 频控预算被打光等 | — |
| 39 | DX | v1 中文 UI + 中文 README 优先 | Mechanical | P3 | 主 persona 信任信号 | 双语同步 |
| 40 | DX | 发现的 provider 默认启用 + 3s 通知 + 断开逃生口 | Mechanical | P1 | magical moment 兑现 | 仅提示手动启用 |
| 41 | DX | 下载双通道（镜像+Gitee）+ E1 并发双查 | Mechanical | P1 | 主 persona 决定性摩擦 | 仅 GitHub 原链 |
| 42 | DX | 升级闭环规格 + "跳过此版本" | Mechanical | P1 | "信任续费"时刻不能断 | 只跳 release 页 |
| 43 | DX | 来源透明+官方核对链接+套餐名 | Mechanical | P1 | 数字权威性是对 dashboard 的唯一短板 | 不标注 |
| 44 | DX | ProviderError→人话映射表进设计规格 | Mechanical | P1 | 5 FAIL+3 未写文案 | 逐案自由发挥 |
| 45 | DX | keyring 不可用给"本次会话暂存"选项 | Taste(小) | P1 | 内存暂存比磁盘更安全，只是麻烦 | 死路一条 |
| 46 | DX | key 规范化 + GLM 双端点自动试 + 轮询钳制 + 重置外观 | Mechanical | P3 | 消灭 100% 可预见的假故障 | — |
| 47 | DX | sessionKey 保留为高级兜底 + README 指引 | Mechanical | P1 | 消除计划内部矛盾 | 删掉该档 |
| 48 | DX | README 十章 + checksums + provenance + 卸载清凭据询问 | Mechanical | P1 | 未签名 exe 的信任闭环 | — |
| 49 | DX | ToS 对话框进 M1 spike + 逐字文案含写回披露 | Mechanical | P1 | 藏写回行为的代价远大于说出来 | — |

## GSTACK REVIEW REPORT

- Pipeline: /autoplan（CEO → Design → Eng → DX，顺序执行，自动决策）
- Voices: Codex CLI 不可用 → 单模型模式（Claude 主评审 + Claude 独立声音 × 4 阶段）
- Scores: CEO 7/10 · Design 4/10→8/10 · Eng 7/10→8.5/10 · DX 4.5/10→7.5-8/10
- Decisions: 49 total（44 auto-decided · 3 taste(小) · 4 user challenges 已在前提门拍板 · 2 taste 留最终门：#T1 深浅色、#T2 默认密度）
- Premise gate: PASSED 2026-09-01（个人工具/悬浮条+托盘/跳过 M0/ToS 明示）
- Artifacts: PLAN.md（本文件）· TODOS.md · test plan → ~/.gstack/projects/desktoken/desktoken-no-branch-test-plan-20260901.md · restore point → ~/.gstack/projects/desktoken/no-branch-autoplan-restore-20260901-223911.md
- Final gate: **APPROVED 2026-09-01**（全部接受推荐：#T1 深色固定 v1、#T2 固定展开+环境告警）
- Status: **APPROVED — 可开工 M1（Lane A ∥ Lane D）**
