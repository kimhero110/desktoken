# PLAN（现状版）

## 当前规划重点

ChatGPT 桌面端仍未接通：本机身份可以核实，但外部只读订阅信号和真实四态都还缺实测证据，正式监控不启动。交互原型只是合成数据的草稿，不能拿它的检查结果推定桌面接入可用。

> 本文件是**活文档**，描述 QuotaBar 当前的实际形态与近期方向。
> 2026-09-01 的四轮评审原始计划已归档至 [docs/PLAN-history.md](docs/PLAN-history.md)——
> 其中的设计规格（26 态 UI、10px 圆角、wiremock 测试层等）部分已被后续实测推翻或延后，以本文件为准。

## 产品一句话

常驻 Windows 桌面的迷你悬浮条，同屏显示本机**所有** AI 编程订阅额度（5h 滚动窗/周窗/日窗）与重置倒计时——包括同一平台的多个账号（CLI / opencode / 手动 key 各自成行）。

## 当前架构（2026-09-06）

接入流程已增加 `task_install.rs`：默认路径安装/移除、原文件备份、接入记录、配置冲突检测；界面先展示生成配置，再开放安装按钮。接入信任仍由工具自身管理。

开发版方向：在现有应用中迭代本机 AI 工作状态，保留额度功能。当前增加 `task_monitor.rs`（本地事件接收、状态和通知）、`task_integration.rs`（配置生成）及 `integrations/opencode.js`（V1 适配）。不扩展远程监控。具体接入和未验证边界见 [本机任务文档](docs/task-monitor.md)。

```
src-tauri/src/
├── main.rs           # 窗口生命周期/托盘/菜单/命令/自启动/日志滚动
├── poller.rs         # 实例驱动轮询 supervisor：sync() 热重载（abort/拉起）、
│                     #   E5 toast 状态机（滞回去抖/持久化去重）
├── oauth.rs          # 六步刷新并发协议（单飞/compare-before-write/rename 重试）
├── credentials.rs    # 凭据发现：CLI 文件 + opencode auth.json + keyring + 外域只读
├── history.rs        # E8 用量历史（SQLite，7 天）
├── settings.rs       # 全局写锁 + edit()/try_edit() 原子读改写
├── atomic_file.rs    # 保留目标权限/DACL 的原子替换（凭据、设置、接入配置共用）
├── diagnostics.rs    # 统一脱敏层 + 复制诊断
├── updater_check.rs  # E1 版本检查（24h 缓存/双通道）
└── providers/        # 每平台一个文件 + mod.rs 的 fetch_instance 统一分发
```

## 与原始计划的差异（及原因）

| 计划 | 现实 | 原因 |
|---|---|---|
| 悬浮条圆角 10px | 方角 | 圆角会露出方形窗口的 acrylic 灰角（实测）；DWM 圆角在 Server/Win10 静默失效 |
| 鼠标穿透 | 迷你模式 | spike A 五案全败，用户拍板降级 |
| wiremock 4 场景测试 | 已有 HTTP 契约测试 | 包含状态码、Retry-After、超时与响应截断 |
| Gemini 走 Code Assist 官方流 | 走 Antigravity 本地 LS | 官方通道对消费账号 403（实测），LS 通道与 IDE 面板同源 |
| 单账号/provider | 多实例同屏（方案 B） | 用户多账号是真实形态（CLI + opencode） |

## 可靠性修复（已发布 v0.3.2）

- 前端额度汇总只使用有效且未过期的快照；全未知为灰色「—」，部分未知附「*」。
- 自定义间隔读取 poll_minutes（1–1440 分钟），已有任务在配置保存后唤醒；429 冷却不被刷新绕过。
- HTTP 响应携带 Retry-After；本地退避有抖动，最终等待以配置周期和服务端下限为准。
- 平台 fetch 使用 unwind 捕获边界；SQLite 打开/建表失败返回空历史并在下次重试。
- OAuth 待写回状态仅存在内存，包含轮换后的 token 对；CLI 有效文件更新优先。
- 测试设置/历史存放临时目录；Rust 测试/构建锁定依赖，Node 内置 test runner 测试额度汇总。
- 暂未提供唤醒错峰调度；OAuth 刷新端点 429 的独立冷却仍待补充。

## 已知欠账

- [x] wiremock 集成测试层（429+Retry-After/超时/1MB 截断）— 2026-09-05 完成：
      fetch.rs 拆 `*_via(client,...)` 注入测试 client，glm/custom 端点参数化，
      15 个 HTTP 契约测试（`mod http`）落在各文件 `#[cfg(test)]` 内
- [ ] 前端纯函数测试（fmtCountdown/suggestion）
- [ ] 72h soak / 国产杀软实测 / Win10 实体 toast
- [ ] Claude 通道真机出数验证（需登录）
- [ ] macOS dmg 实机验证（已出包未跑过）
- [ ] 设置窗口 i18n + 高度自适应（实例多时溢出）
- [ ] 手动多把命名 key（kimi/二号）

发布边界（2026-09-07）：ChatGPT 桌面端未接入，Codex 桌面端未验证。Codex CLI hook 配置与桌面端监控不能混为一谈。v0.4.0-beta.1 为预览版，v0.3.2 保持稳定版。
