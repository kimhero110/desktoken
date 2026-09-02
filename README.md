<div align="center">

# 📊 QuotaBar

**The lightweight, non-intrusive desktop floating status bar for monitoring AI coding quotas & rate limits.**

[![GitHub stars](https://img.shields.io/github/stars/kimhero110/desktoken?style=social)](https://github.com/kimhero110/desktoken)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Platform: Windows](https://img.shields.io/badge/Platform-Windows%2010%20%2F%2011-0078D6?logo=windows)](https://github.com/kimhero110/desktoken/releases)
[![Built with: Tauri 2 + Rust](https://img.shields.io/badge/Built%20with-Tauri%202%20%2B%20Rust-FFC131?logo=tauri)](https://tauri.app)

[English](#-key-features) | [中文说明](#-中文介绍)

</div>

---

## ⚡ Why QuotaBar?

When you are deep in the zone coding with **Claude Code, Cursor, Codex, Kimi, or GLM**, the biggest workflow killer is unexpectedly hitting a **5-hour rolling rate limit (HTTP 429)** or exhausting your weekly allowance without warning.

**QuotaBar** lives quietly in the corner of your screen as a sleek, translucent Acrylic widget. It continuously monitors your real-time token quotas, countdown resets, and rate limits across multiple LLM providers—**without stealing keyboard focus or interrupting your typing.**

---

## 🌟 Key Features

* **🪟 Non-Intrusive Acrylic Overlay (`WS_EX_NOACTIVATE`)**:
  Always-on-top translucent floating bar that **never steals keyboard focus** from VS Code, Cursor, or Terminal. Click through and drag anywhere.
* **⏱️ Real-Time 5-Hour & Weekly Rolling Window Tracking**:
  Visual countdown timers (e.g. `43m`, `2.1h`, `3d4h`) and color-coded progress bars (Green < 70% ➔ Amber 70-90% ➔ Red > 90%).
* **🤖 Multi-Provider Native Support**:
  * **Claude**: Real-time OAuth usage / rate-limit header monitoring (auto-reads `~/.claude/.credentials.json` with atomic concurrency protection).
  * **Kimi (Moonshot)**: Real-time coding quotas (`~/.kimi-code/credentials/kimi-code.json`).
  * **Codex (OpenAI)**: Direct token headroom tracking (`~/.codex/auth.json`).
  * **GLM (Zhipu AI)**: Enterprise & Developer coding quotas.
  * **Gemini (Google Code Assist)**: Multi-bucket quota telemetry.
* **🧩 Universal Custom Endpoint Engine**:
  Easily add *any* custom OpenAI-compatible proxy or internal company gateway by defining URL, Auth Header, and JSON Path mappings (`used/limit/reset`) directly in the Settings UI—zero coding required.
* **🛡️ Zero-Telemetry & 100% Private**:
  No tracking, no analytics, no external cloud server dependencies. All API keys and tokens are stored securely inside your local **Windows Credential Manager (Keyring)**.
* **⚡ Ultra-Lightweight (Tauri 2 + Rust)**:
  Native Rust backend with pure zero-dependency Web frontend. Minimal CPU (<1%) and memory footprint.

---

## 🚀 Quick Start (Installation)

### Option 1: Download Pre-built Binary (Recommended)
1. Go to **[GitHub Releases](https://github.com/kimhero110/desktoken/releases)**.
2. Download `QuotaBar-Setup.exe` (or the portable `QuotaBar.exe`).
3. Run the application. On first launch, review the privacy policy and connect your AI developer keys.

> **💡 Windows SmartScreen Notice**:
> Since QuotaBar is a free open-source tool without an expensive commercial EV Code Signing Certificate, Windows Defender / SmartScreen might display *"Windows protected your PC"*.
> Click **"More info" (更多信息)** ➔ **"Run anyway" (仍要运行)** to launch safely.

### Option 2: Build from Source
Ensure you have **Rust** and **Node.js** installed:

```bash
# 1. Clone repository
git clone https://github.com/kimhero110/desktoken.git
cd desktoken

# 2. Run in development mode
cd src-tauri
cargo tauri dev

# 3. Build release executable
cargo tauri build
```

---

## 🇨🇳 中文介绍

**QuotaBar（原 DeskToken）** 是专为重度 AI 编程开发者打造的 **Windows 桌面极客透明悬浮条**。

### 痛点解决：
* 在 Cursor / Claude Code / Windsurf 写代码时，经常因未知剩余配额而突然遭遇 429 限流掐断思路；
* **QuotaBar** 常驻桌面角落，一眼看清各家大模型的 **5 小时滑动窗口用量、每周用量与倒计时重置时间**；
* 采用 Windows 原生 `WS_EX_NOACTIVATE` 机制，**窗口置顶但绝不抢占任何键盘光标输入**。

---

## 🤝 Contributing & Community

Contributions, issues, and feature requests are welcome!
Feel free to check [issues page](https://github.com/kimhero110/desktoken/issues) or submit a Pull Request.

Sister Project: 🌐 [FreeTokens.info](https://freetokens.info) — Global Free LLM API & Compute Intelligence Radar.

---

## 📜 License

Distributed under the **MIT License**. See `LICENSE` for more information.
