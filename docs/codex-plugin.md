# QuotaBar Codex Plugin Packaging (Prototype)

`scripts/build-codex-plugin.cjs` packages QuotaBar's local task-status receiver
as a bounded, passive, local-only Codex plugin directory.

**Status: prototype.** The generated package has **not** been tested against
Codex desktop. Treat it as a scaffold for experimentation, not a finished
integration.

## Usage

```bash
node scripts/build-codex-plugin.cjs <receiverExe> <outputDir>
```

- `receiverExe` — absolute path to an existing QuotaBar receiver executable
  (e.g. `src-tauri/target/release/quotabar.exe`).
- `outputDir` — plugin scaffold directory outside this repository (e.g.
  `%LOCALAPPDATA%\codex-plugins\quotabar`), basename `quotabar`. It must
  **already contain** `.codex-plugin/plugin.json` with `name: "quotabar"`;
  the script refuses to create a plugin on an unknown path.

The script:

1. Invokes the receiver once with `task-config codex` via `spawnSync`
   (no shell, 15s bounded timeout).
2. Parses the outer JSON envelope and requires `content` to be a JSON
   string, then the hook config inside it.
3. Requires the **exact canonical nested shape** produced by the Rust
   generator (`src-tauri/src/task_integration.rs`):
   `{ "hooks": { "<Event>": [ { "hooks": [ handler ] } ] } }` with exactly
   the 8 passive events (`UserPromptSubmit`, `PreToolUse`, `PostToolUse`,
   `PermissionRequest`, `Stop`, `SessionStart`, `SessionEnd`, `Interrupt`),
   each exactly one group holding exactly one `command` handler. Any other
   event (trust, config, user hooks) is rejected.
4. Preserves each group and handler verbatim — including `timeout` and
   `commandWindows` — and only ensures `statusMessage:
   "QuotaBar local task status: <event>"` is present.
5. Writes readable UTF-8 JSON:
   - `hooks/hooks.json` — same nesting as the generator, 8 passive events.
   - `.codex-plugin/plugin.json` — a manifest in the **exact shape accepted
     by the official plugin validator**: `name` (`quotabar`), `version`
     (0.1.0), `description`, `author` (`{ name: "kimhero110" }`),
     `repository` (https://github.com/kimhero110/desktoken), `license`
     (MIT), and `interface` with `displayName` (`QuotaBar`),
     `shortDescription`, `longDescription`, `developerName`, `category`
     (`Productivity`), `capabilities: []`, and `defaultPrompt: []`. Empty
     `defaultPrompt`/`capabilities` mean the plugin is passive — no action
     suggestions. There is no root `hooks` field (default discovery via
     `hooks/hooks.json`), and no root `displayName`/`developerName`/
     `category`/`capabilities`.

## Safety rules

- **No global edits.** The script never touches Codex global config, the
  marketplace, the trust store, or user hooks files; it only reads the
  receiver output and the target plugin directory.
- **No shell / no network / no dependencies.** Only Node built-ins; the
  receiver runs with `shell: false`.
- **Output outside the repo.** The generated files embed a local absolute
  path to the receiver exe, so they must never be committed. The script
  refuses output directories inside this repository (resolved via realpath
  to handle junctions/symlinks) and rejects unsafe paths.
- **No silent overwrite.** `hooks/hooks.json` is written with exclusive
  `wx` semantics; if it already exists, the script refuses to run rather
  than overwrite installed hook definitions.
- **Known scaffold required.** The output directory must already be a
  matching QuotaBar scaffold (`.codex-plugin/plugin.json` with
  `name: "quotabar"`); the script never invents a plugin on a new path.

## Installation and migration (local only)

Migration order matters — user hooks and the plugin **must not both
execute**, or every event fires twice and task status is double-reported:

1. Build the scaffold into a directory outside the repo (see Usage).
2. Register/copy the directory through Codex's normal local plugin flow.
3. **Before trusting the plugin**, remove or disable your own user-level
   QuotaBar hooks (e.g. `~/.codex/hooks.json` entries from the receiver's
   user-hook install). Do not trust the plugin while user hooks are still
   active.
4. Approve the plugin through Codex's **normal trust/approval flow** —
   the plugin never bypasses trust requirements.
5. Your previous user hooks stay preserved (the receiver and this script
   never overwrite them) until you are ready to switch; if the plugin
   misbehaves, disable it and re-enable your user hooks to return to the
   working setup.

## Known desktop limitations (prototype caveats)

- The manifest matches the official plugin validator's accepted shape, and
  the hooks.json payload mirrors the receiver's own generated nested shape —
  both are structurally validated by this script's checks and unit tests.
  Runtime discovery is verified below; real desktop execution and visual confirmation remain pending.
- In the desktop's per-row display, hook rows are shown as "Hook N" and
  **cannot be renamed**. Grouping rows under a plugin source (so they appear
  as one QuotaBar plugin block instead of 8 anonymous "Hook N" rows) is the
  desired presentation, which is why this is packaged as a plugin rather
  than as user-level hook rows.

## Tests

```bash
node --test tests/codex-plugin.test.cjs
```

Uses a fixture shaped exactly like the Rust generator (outer envelope with
`content` JSON string, nested groups, `timeout`, `commandWindows`). Covers
roundtrip preservation of Windows command/`commandWindows`/`timeout`,
rejection of malformed/multi/missing/extra events, manifest shape, path
safety, exclusive hooks writes, and scaffold-manifest requirement. No
receiver binary is required.
## 2026-09-08 本机安装与迁移证据

- 使用 plugin-creator 官方脚手架建立个人市场条目，插件 ID 为 `quotabar@personal`，显示名为 `QuotaBar`。
- `validate_plugin.py` 对实际生成插件校验通过；完整 Node 测试 43 项通过。
- 使用桌面端相同运行时的正常 `plugin add` 安装成功。只读 `hooks/list` 确认 8 个事件均具有 `pluginId=quotabar@personal`；这是运行时来源证据，不冒充桌面截图或实际执行证据。
- 安装缓存内全部 8 个处理器的命令、Windows 命令及超时，与现有接收程序生成值逐项一致；仅增加来源状态说明。
- 使用现有 QuotaBar `task-remove` 安装器备份并移除旧用户级 QuotaBar 条目。迁移后只读检查为 8 个启用、8 个待信任，不再有两套配置。未写入信任记录。
- 当前处于切换待审核状态：桌面设置 → Coding → Hooks 中审核并信任 QuotaBar 插件的钩子后才能执行。在审核前，新接入不会上报；旧用户钩子已备份。不要再从旧版本设置重复安装用户钩子。
- 可回退：先移除 QuotaBar 插件，再用现有 QuotaBar 安装器恢复用户接入并检查信任；不要把备份直接覆盖后来产生的其他用户配置。
- 未创建新应用版本或替换桌面 QuotaBar 二进制；插件仍调用同一接收程序。

官方机制依据：[插件附带的钩子与信任规则](https://learn.chatgpt.com/zh-Hans/docs/hooks)。
