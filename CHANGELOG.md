# Changelog

## v0.3.1 (未发布——main)

- **修复：自定义监视在多实例改造中丢失分发**（providers::fetch_instance 补 custom 分支）
- **修复：settings.json 并发写竞争**——所有写入收口到全局写锁 + `settings::edit()` 原子读改写（poller toast 去重 / UI 开关 / 位置持久化 / 版本缓存）
- **修复：spike.log 无限增长**——超 2MB 自动截断保留尾部 2000 行
- **CI：main 分支 push/PR 跑 cargo test**（此前只在发版时测试）
- 清理死代码（history::series）

## v0.3.0 (2026-09-03)

- 真图标（quota 三柱 logo）+ 左键详情卡：每窗口剩余量、重置绝对时刻、官方页面链接、7 日 sparkline
- E8 用量历史：本地 SQLite，7 天保留
- 微交互：新 provider 加载占位、窗口重置闪绿、≥90% 跨账号指路建议、错误行一次性 hint
- 迷你模式重做：宽度贴合内容、程序员段子/时段关怀轮播（超阈值自动变指路）
- 设置页版本页脚 + 检查更新；CONTRIBUTING；GitHub Pages 落地页
- macOS 移植基础 + CI dmg 产物（Apple Silicon）
- CI 修复：transparent() 平台门控、icon.png、checksums 分平台

## v0.2.3 (2026-09-03)

- 四角灰斑根治（方角铺满；SetWindowRgn 方案实测更糟已回滚）
- 迷你模式 22px + 菜单勾勾与状态联动
- release.ps1 发版一条龙（bump→测试→tag→双推→盯 CI→Gitee 发行版）

## v0.2.0 (2026-09-03)

- GUI 子系统修复（消灭 console 黑窗）+ DWM 圆角（Win11）
- M5 主体：Toast 告警（≥90% 滞回去抖/重置瞬间）、复制诊断信息（统一脱敏+泄漏断言）、版本检查横幅、开机自启、右键菜单全接线
- release 流水线：checksums + build provenance attestation
- README 十章重写 + 真实截图 + 凭据行为清单

## v0.1.0 (2026-09-02)

- M1-M4：窗口骨架（NOACTIVATE/acrylic/拖动/托盘/单实例/迷你）、Kimi/GLM/Codex/Claude/Gemini 五家 provider
- oauth.rs 六步刷新并发协议（单飞/compare-before-write/rename 重试）+ spike B 对抗测试
- 首启 ToS 门禁（同意前零网络）
- Antigravity 通道逆向（本地 LS ConnectRPC + UA 门禁 + csrf token）
