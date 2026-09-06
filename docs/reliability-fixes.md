# 可靠性修复记录（v0.3.2，2026-09-06）

基线：v0.3.1 / 339ead47f45e8cbe3d49193a3f9a53466de40fc5。

## 修复行为

| 问题 | 当前行为 | 回归验证 |
|---|---|---|
| 全失败被显示为绿色 0% | 无有效数据为灰色「—」；部分不可用时汇总附「*」 | Node 汇总与实际页面事件处理测试 |
| 自定义间隔未进入调度 | 使用 poll_minutes，限制在 1–1440 分钟，配置保存唤醒现有任务 | Rust 周期边界测试；长周期快照新鲜度测试 |
| 发布 panic 导致整个进程退出 | release panic=unwind；平台 fetch 捕获 panic 后返回错误并继续轮询 | Rust panic/下一次请求恢复测试 |
| SQLite 初始化 expect 崩溃 | 打开/建表返回错误，降级为空历史，下次重试 | Rust 数据库路径不可用测试 |
| Retry-After 丢失 | GET/POST 保留秒数或 HTTP 日期；额度适配器传给调度器 | HTTP 契约、日期解析与退避下限测试 |
| 手动刷新绕过冷却 | 429 等待期间不监听手动刷新通知 | 调度分支代码检查 |
| OAuth 写回失败丢失轮换凭据 | 完整 token 对保存在进程内；下轮重试，CLI 变更优先 | 连续写回失败、恢复写回、CLI 覆盖、过期后用新 refresh token 测试 |
| Cargo.lock 版本落后 | 仅将根包 0.3.0 对齐到 0.3.1，依赖版本不变 | --locked 测试和构建 |
| 发布脚本再次漏更新锁文件 | 源仓库锁文件随版本同步，Rust 与 Node 测试均为发布门禁 | PowerShell 语法检查；未执行发布 |

额度快照新增 `stale_after_secs`：至少 600 秒，长轮询周期采用周期加 60 秒；前端兼容缺失字段的旧快照。来源字段保留兼容值 `official`，展示文案改为 CLI 同源接口。

## 本地验证

- `cargo test --locked --offline`：81 项通过。
- `cargo build --locked --offline`：Windows 开发构建通过。
- `node --test tests/*.test.cjs`：4 项通过，包含实际页面脚本的事件渲染与所有内联脚本语法检查。
- `git diff --check`：通过。
- `release.ps1`：PowerShell AST 语法检查通过。

Rust 测试设置/历史位于进程独立的临时目录，不使用真实的应用设置与历史文件。没有启动桌面应用、执行真实账号请求、运行发布脚本或上传产物。Node 渲染测试使用 DOM/Tauri 替身，不等同于 WebView2 真机交互验证。

## 仍存在的边界

- OAuth 刷新端点的 HTTP 429 目前映射为网络错误；额度 HTTP 请求使用 Retry-After 冷却。
- 未实现专门的系统唤醒错峰刷新，README 已移除该保证。
- OAuth 写前比较并非跨进程原子 CAS；进程退出时待写回内存状态消失。
- macOS 实机、Windows 通知/焦点行为、安装包与 release 二进制仍需发布前验证。
