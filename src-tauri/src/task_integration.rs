//! Generate reviewable snippets; never overwrite existing tool configuration.
use serde::Serialize;
use serde_json::json;

#[derive(Serialize)]
pub struct Integration {
    target: String,
    pub(super) content: String,
    note: String,
}

/// Characters that make a Windows hook command line unsafe to quote: NUL and
/// control characters can truncate the line; `"` breaks out of the quoting;
/// `%` is expanded by cmd.exe even inside double quotes (an install path under
/// e.g. C:\%APPDATA%-style dirs could be rewritten into a different command).
/// All other filename-legal characters are inert inside double quotes.
fn windows_exe_path_is_safe(exe: &str) -> Result<(), String> {
    if exe
        .chars()
        .any(|c| c.is_control() || c == '"' || c == '%')
    {
        return Err(format!(
            "应用路径包含 Windows 命令行不安全字符（引号/百分号/控制字符），已拒绝生成 hook 命令：{exe}"
        ));
    }
    Ok(())
}

fn command(exe: &str, tool: &str, windows: bool) -> Result<String, String> {
    if windows {
        windows_exe_path_is_safe(exe)?;
        // Direct native invocation: the host shell runs the quoted executable,
        // which reads the hook JSON from stdin itself (no PowerShell wrapper,
        // no encoded commands). Host stdin/stdout pass through unchanged.
        Ok(format!("\"{exe}\" task-event {tool}"))
    } else {
        Ok(format!("'{}' task-event {}", exe.replace('\'', "'\"'\"'"), tool))
    }
}

fn generate(exe: &str, tool: &str, windows: bool) -> Result<Integration, String> {
    if tool == "opencode" {
        return Ok(Integration {
            target: "~/.config/opencode/plugins/quotabar-tasks.js".into(),
            content: include_str!("../../integrations/opencode.js").replace(
                "__QUOTABAR_EXE_JSON__",
                &serde_json::to_string(exe).map_err(|e| e.to_string())?,
            ),
            note: "OpenCode V1 插件接口；保存后重启 OpenCode。V2 接口尚未接入。".into(),
        });
    }
    if !matches!(tool, "codex" | "claude") {
        return Err("该工具尚未接入".into());
    }
    let mut hooks = serde_json::Map::new();
    let common = [
        "UserPromptSubmit",
        "PreToolUse",
        "PostToolUse",
        "PermissionRequest",
        "Stop",
        "SessionStart",
        "SessionEnd",
    ];
    let extra: &[&str] = if tool == "codex" {
        &["Interrupt"]
    } else {
        &["Notification", "StopFailure"]
    };
    for name in common.iter().chain(extra.iter()) {
        // Official Claude hooks reference (code.claude.com/docs/en/hooks):
        // when `args` is present, `command` is the executable spawned
        // DIRECTLY (no shell). A raw path works with spaces/Unicode and
        // avoids host-shell divergence (Windows no-args hosts use Git Bash or
        // PowerShell, NOT cmd — do not assume cmd quoting there).
        let mut hook = if tool == "claude" {
            json!({"type":"command","command":exe,"args":["task-event",tool],"timeout":3})
        } else {
            // Codex: command string form (cmd semantics on Windows).
            json!({"type":"command","command":command(exe, tool, windows)?,"timeout":3})
        };
        if tool == "codex" {
            // 官方可选字段，仅用于在 hook 详情中标识来源；不能重命名 Hook 索引中的行。
            hook["statusMessage"] = json!(format!("QuotaBar local task status: {name}"));
            if windows {
                hook["commandWindows"] = json!(command(exe, tool, windows)?);
            }
        }
        hooks.insert((*name).into(), json!([{"hooks":[hook]}]));
    }
    Ok(Integration {
        target: if tool == "codex" { "~/.codex/hooks.json" } else { "~/.claude/settings.json" }.into(),
        content: serde_json::to_string_pretty(&json!({"hooks":hooks})).map_err(|e| e.to_string())?,
        note: "可点击安装接入，或手动合并 hooks 条目；不要覆盖已有配置。重启工具；Codex 还需信任此 hook。仅适用于支持这些事件的版本。".into(),
    })
}

#[tauri::command]
pub fn task_integration_config(tool: String) -> Result<Integration, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    generate(&exe.to_string_lossy(), &tool, cfg!(windows))
}

pub fn default_path(tool: &str) -> Result<std::path::PathBuf, String> {
    let relative = match tool {
        "codex" => ".codex/hooks.json", "claude" => ".claude/settings.json", "opencode" => ".config/opencode/plugins/quotabar-tasks.js",
        _ => return Err("该工具尚未接入".into()),
    };
    let home_dir = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).ok_or("无法确定用户目录")?;
    Ok(std::path::PathBuf::from(home_dir).join(relative))
}

#[derive(Serialize)]
pub struct IntegrationStatus {
    tool: String,
    path: String,
    config: String,
    last_event_at: Option<i64>,
}

fn inspect(path: &std::path::Path, expected: &Integration, tool: &str) -> &'static str {
    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return "missing",
        Err(_) => return "unreadable",
    };
    if tool == "opencode" {
        return if raw.replace("\r\n", "\n") == expected.content.replace("\r\n", "\n") { "matched" } else { "different" };
    }
    let Ok(actual) = serde_json::from_str::<serde_json::Value>(&raw) else { return "invalid"; };
    if !actual.is_object() { return "invalid"; }
    if let Some(hooks) = actual.get("hooks") {
        let Some(events) = hooks.as_object() else { return "invalid"; };
        for groups in events.values() {
            let Some(groups) = groups.as_array() else { return "invalid"; };
            for group in groups {
                if group.get("matcher").is_some_and(|m| !m.is_string()) || group.get("hooks").and_then(|h| h.as_array()).is_none() { return "invalid"; }
            }
        }
    }
    let wanted: serde_json::Value = serde_json::from_str(&expected.content).unwrap_or_default();
    let Some(events) = wanted["hooks"].as_object() else { return "invalid"; };
    let count = events.iter().filter(|(event, groups)| {
        let expected_hook = &groups[0]["hooks"][0];
        actual["hooks"].get(*event).and_then(|v| v.as_array()).is_some_and(|groups| groups.iter().any(|g| {
            // A restrictive matcher can silently miss events, even with the correct command.
            let all = g.get("matcher").and_then(|v| v.as_str()).is_none_or(|m| m.is_empty() || m == "*");
            all && g["hooks"].as_array().is_some_and(|hooks| hooks.iter().any(|h| {
                h["type"] == "command" && h["command"] == expected_hook["command"]
                    && h.get("commandWindows") == expected_hook.get("commandWindows")
                    && h.get("args") == expected_hook.get("args")
            }))
        }))
    }).count();
    if count == events.len() { "matched" } else if count > 0 { "partial" } else { "different" }
}

#[tauri::command]
pub fn task_integration_status() -> Result<Vec<IntegrationStatus>, String> {
    let home_dir = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).ok_or("无法确定用户目录")?;
    let root = std::path::PathBuf::from(home_dir);
    let tasks = crate::task_monitor::list_tasks();
    let rows = [ ("codex", ".codex/hooks.json"), ("claude", ".claude/settings.json"), ("opencode", ".config/opencode/plugins/quotabar-tasks.js") ]
        .into_iter().map(|(tool, relative)| {
            let path = root.join(relative);
            // A per-tool failure disables that row only. Collecting into a
            // Result meant one rejected path (the codex command guard refuses
            // a `%`, which cmd expands even inside quotes) hid the claude and
            // opencode rows too, though neither uses that command form.
            let config = match task_integration_config(tool.into()) {
                Ok(expected) => inspect(&path, &expected, tool).to_string(),
                Err(e) => e,
            };
            IntegrationStatus {
                tool: tool.into(), path: path.to_string_lossy().into_owned(), config,
                last_event_at: tasks.iter().filter(|t| t.tool == tool).map(|t| t.updated_at).max(),
            }
        }).collect();
    Ok(rows)
}

pub fn handle_cli() -> bool {
    let args: Vec<_> = std::env::args().collect();
    if matches!(args.get(1).map(String::as_str), Some("task-install" | "task-remove")) {
        let result = (|| -> Result<crate::task_install::InstallResult,String> {
            if args.len() != 4 { return Err("用法：task-install|task-remove codex|claude|opencode 接收程序绝对路径".into()); }
            let receiver=std::path::Path::new(&args[3]);
            if !receiver.is_absolute() || !receiver.is_file() { return Err("接收程序必须是已存在文件的绝对路径".into()); }
            let path=default_path(&args[2])?;
            let config=generate(&args[3],&args[2],cfg!(windows))?;
            crate::task_install::change(&path,&args[2],&config.content,args[1]=="task-install")
        })();
        match result {
            Ok(report)=>println!("{}",serde_json::to_string(&report).unwrap_or_default()),
            Err(error)=>{eprintln!("{error}");std::process::exit(1);}
        }
        return true;
    }
    if args.get(1).map(String::as_str) != Some("task-config") { return false; }
    match args.get(2).ok_or_else(|| "需要工具名".to_string()).and_then(|t| task_integration_config(t.clone())) {
        Ok(config) => println!("{}", serde_json::to_string(&config).unwrap_or_default()),
        Err(error) => { eprintln!("{error}"); std::process::exit(1); }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnosis_distinguishes_missing_invalid_partial_and_restrictive_matchers() {
        let dir = crate::settings::app_data_dir().join("integration-diagnosis");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        let expected = generate("test.exe", "codex", true).unwrap();
        assert_eq!(inspect(&dir.join("missing.json"), &expected, "codex"), "missing");
        std::fs::write(&path,"{").unwrap();
        assert_eq!(inspect(&path,&expected,"codex"),"invalid");
        std::fs::write(&path,&expected.content).unwrap();
        assert_eq!(inspect(&path,&expected,"codex"),"matched");
        let mut malformed:serde_json::Value=serde_json::from_str(&expected.content).unwrap();
        malformed["hooks"]["Stop"][0]["matcher"]=json!(42);
        std::fs::write(&path,malformed.to_string()).unwrap();
        assert_eq!(inspect(&path,&expected,"codex"),"invalid");
        let mut v:serde_json::Value=serde_json::from_str(&expected.content).unwrap();
        v["hooks"]["PreToolUse"][0]["matcher"]=json!("OnlyOneTool");
        std::fs::write(&path,v.to_string()).unwrap();
        assert_eq!(inspect(&path,&expected,"codex"),"partial");
        let old = generate("moved.exe", "codex", true).unwrap();
        std::fs::write(&path,&old.content).unwrap();
        assert_eq!(inspect(&path,&expected,"codex"),"different");
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn config_is_passive_and_handles_path_metacharacters() {
        let config = generate("C:\\a b\\O'Brien $x\\quotabar.exe", "codex", true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&config.content).unwrap();
        assert!(v["hooks"]["Stop"][0]["hooks"][0].get("command_windows").is_none());
        assert!(v["hooks"]["Stop"][0]["hooks"][0].get("commandWindows").is_some());
        let cmd = v["hooks"]["Stop"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        // Direct quoted native executable, no PowerShell, no encoding
        assert_eq!(cmd, "\"C:\\a b\\O'Brien $x\\quotabar.exe\" task-event codex");
        assert!(!cmd.to_lowercase().contains("powershell"));
        assert!(!cmd.contains("-EncodedCommand"));
        // unsafe path characters are rejected instead of mis-quoting
        assert!(generate("C:\\bad%name\\quotabar.exe", "codex", true).is_err());
        assert!(generate("C:\\bad\"quote\\quotabar.exe", "codex", true).is_err());
        assert!(generate("C:\\bad\u{0}nul\\quotabar.exe", "codex", true).is_err());
        assert!(!config.content.contains("decision"));
        // POSIX claude uses the same direct-spawn args form
        let claude: serde_json::Value =
            serde_json::from_str(&generate("/a'b/app", "claude", false).unwrap().content).unwrap();
        assert_eq!(
            claude["hooks"]["Stop"][0]["hooks"][0]["args"],
            json!(["task-event", "claude"])
        );
        assert!(generate("app", "kimi", false).is_err());
    }
    #[test]
    fn codex_hooks_carry_provenance_and_command_unchanged() {
        let config = generate("test.exe", "codex", true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&config.content).unwrap();
        let events = v["hooks"].as_object().unwrap();
        assert_eq!(events.len(), 8);
        for (event, groups) in events {
            let hook = &groups[0]["hooks"][0];
            assert_eq!(
                hook["statusMessage"].as_str().unwrap(),
                format!("QuotaBar local task status: {event}")
            );
            assert_eq!(hook["type"], "command");
            assert_eq!(hook["timeout"], 3);
            assert_eq!(
                hook["command"].as_str().unwrap(),
                "\"test.exe\" task-event codex"
            );
            assert_eq!(hook["command"], hook["commandWindows"]);
            assert!(hook.get("name").is_none());
            assert!(hook.get("args").is_none());
        }
    }
    /// Official Claude semantics: `command` + `args` are spawned directly,
    /// no shell — the raw (unquoted) executable path is used verbatim.
    #[test]
    fn claude_hooks_use_direct_spawn_command_and_args() {
        let config = generate("C:\\a b\\目录\\QuotaBar 拷 贝.exe", "claude", true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&config.content).unwrap();
        let events = v["hooks"].as_object().unwrap();
        assert_eq!(events.len(), 9); // 7 common + Notification + StopFailure
        for (event, groups) in events {
            let hook = &groups[0]["hooks"][0];
            assert_eq!(
                hook["command"].as_str().unwrap(),
                "C:\\a b\\目录\\QuotaBar 拷 贝.exe",
                "raw path, event {event}"
            );
            assert_eq!(hook["args"], json!(["task-event", "claude"]));
            assert_eq!(hook["type"], "command");
            assert_eq!(hook["timeout"], 3);
            assert!(hook.get("commandWindows").is_none());
            assert!(hook.get("statusMessage").is_none());
        }
    }
    #[test]
    fn plugin_path_is_json_escaped() {
        let config = generate("C:\\a b\\quotabar.exe", "opencode", true).unwrap();
        assert!(config.content.contains(r#""C:\\a b\\quotabar.exe""#));
        assert!(!config.content.contains("__QUOTABAR_EXE_JSON__"));
    }
}
