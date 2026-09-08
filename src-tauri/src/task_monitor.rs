//! Passive, local-only hook receiver. Never returns an approval/stop decision.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    io::Read,
    path::Path,
    sync::{Mutex, OnceLock},
};
use tauri::Emitter;
use tauri_plugin_notification::NotificationExt;

const MAX_INPUT: u64 = 1_048_576;
const MAX_AGE: i64 = 86_400_000;
const STALE: i64 = 900_000;
fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
fn directory() -> std::path::PathBuf {
    crate::settings::app_data_dir().join("task-events")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Working,
    WaitingApproval,
    WaitingInput,
    Ended,
    Failed,
    Interrupted,
    Unknown,
}
impl State {
    fn attention(self) -> bool {
        matches!(
            self,
            Self::WaitingApproval
                | Self::WaitingInput
                | Self::Ended
                | Self::Failed
                | Self::Interrupted
        )
    }
    fn label(self) -> &'static str {
        match self {
            Self::Working => "正在工作",
            Self::WaitingApproval => "等待授权",
            Self::WaitingInput => "等待输入",
            Self::Ended => "本轮已停止",
            Self::Failed => "发生错误",
            Self::Interrupted => "已中断",
            Self::Unknown => "状态未知",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub tool: String,
    pub session_id: String,
    pub project: String,
    pub state: State,
    pub updated_at: i64,
    #[serde(default)]
    pub session_closed: bool,
    #[serde(default)]
    pub observed_at_ns: i64,
}
fn text(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}
fn clean(s: &str, limit: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(limit).collect()
}

/// Explicit allowlist: no prompt, tool arguments, transcript, error details or credentials leave stdin.
fn normalize(tool: &str, v: &Value, at: i64) -> Option<Task> {
    let event = text(v, "hook_event_name");
    let state = match tool {
        "codex" | "claude" => match event.as_str() {
            "UserPromptSubmit" | "PostToolUse" => State::Working,
            "PreToolUse"
                if matches!(
                    text(v, "tool_name").as_str(),
                    "AskUserQuestion" | "request_user_input"
                ) =>
            {
                State::WaitingInput
            }
            "PreToolUse" => State::Working,
            "PermissionRequest" => State::WaitingApproval,
            "Notification" => match text(v, "notification_type").as_str() {
                "permission_prompt" => State::WaitingApproval,
                "idle_prompt" | "elicitation_dialog" => State::WaitingInput,
                _ => return None,
            },
            "Stop" => State::Ended,
            "StopFailure" => State::Failed,
            "Interrupt" => State::Interrupted,
            "SessionStart" | "SessionEnd" => State::Unknown,
            _ => return None,
        },
        "opencode" => match event.as_str() {
            "busy" | "retry" | "permission.replied" | "question.replied" | "question.rejected" => {
                State::Working
            }
            "permission.asked" => State::WaitingApproval,
            "question.asked" => State::WaitingInput,
            "session.idle" => State::Ended,
            "session.error" => State::Failed,
            "session.deleted" | "unknown" => State::Unknown,
            _ => return None,
        },
        _ => return None,
    };
    let session_id = text(v, "session_id");
    if session_id.is_empty() || session_id.len() > 256 || session_id.chars().any(char::is_control) {
        return None;
    }
    let cwd = text(v, "cwd");
    let project = clean(
        cwd.trim_end_matches(['/', '\\'])
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or_default(),
        80,
    );
    Some(Task {
        tool: tool.into(),
        session_id,
        project,
        state,
        updated_at: at,
        session_closed: matches!(event.as_str(), "SessionEnd" | "session.deleted"),
        observed_at_ns: 0,
    })
}

fn write_event(dir: &Path, task: &Task) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    // The spool is bounded even while the desktop app is closed.
    let mut files: Vec<_> = std::fs::read_dir(dir)?
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort_by_key(|e| e.file_name());
    let excess = files.len().saturating_sub(255);
    for file in files.into_iter().take(excess) {
        let _ = std::fs::remove_file(file.path());
    }
    let name = format!("{}-{}", task.updated_at, std::process::id());
    let tmp = dir.join(format!("{name}.tmp"));
    std::fs::write(&tmp, serde_json::to_vec(task)?)?;
    std::fs::rename(tmp, dir.join(format!("{name}.json")))
}

pub fn handle_cli() -> bool {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("task-watch") {
        let seconds = args.get(2).and_then(|s| s.parse::<u64>().ok()).unwrap_or(30).clamp(1, 120);
        let started = now();
        let mut state = Store::default();
        let mut previous = String::new();
        while now() - started < (seconds * 1000) as i64 {
            let at = now();
            for task in drain(&directory()) { let live = task.updated_at >= started; state.apply(task, at, live); }
            let alerts = state.tick(at);
            let snapshot = state.snapshot();
            let serialized = serde_json::to_string(&snapshot).unwrap_or_default();
            if serialized != previous || !alerts.is_empty() {
                println!("{}", serde_json::json!({"tasks":snapshot,"notification_candidates":alerts}));
                previous = serialized;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        return true;
    }
    if args.get(1).map(String::as_str) != Some("task-event") {
        return false;
    }
    let observed = chrono::Utc::now();
    let at = observed.timestamp_millis();
    let mut raw = Vec::new();
    if std::io::stdin()
        .take(MAX_INPUT + 1)
        .read_to_end(&mut raw)
        .is_ok()
        && raw.len() as u64 <= MAX_INPUT
    {
        if let (Some(tool), Ok(v)) = (args.get(2), serde_json::from_slice::<Value>(&raw)) {
            if let Some(mut task) = normalize(tool, &v, at) {
                task.observed_at_ns = observed.timestamp_nanos_opt().unwrap_or_default();
                let _ = write_event(&directory(), &task);
            }
        }
    }
    println!("{{}}"); // passive success, including malformed/unsupported input
    true
}

#[derive(Default)]
struct Store {
    tasks: HashMap<String, Task>,
    pending: HashMap<String, i64>,
}
impl Store {
    fn apply(&mut self, mut task: Task, at: i64, live: bool) {
        if !matches!(task.tool.as_str(), "codex" | "claude" | "opencode")
            || task.session_id.is_empty()
            || task.session_id.len() > 256
            || task.project.chars().count() > 80
            || (task.observed_at_ns != 0 && task.observed_at_ns.div_euclid(1_000_000) != task.updated_at)
            || task.updated_at > at + 5000
            || at.saturating_sub(task.updated_at) > MAX_AGE
        {
            return;
        }
        let key = format!("{}:{}", task.tool, task.session_id);
        if self
            .tasks
            .get(&key)
            .is_some_and(|old| (old.updated_at, old.observed_at_ns) >= (task.updated_at, task.observed_at_ns))
        {
            return;
        }
        // Some runtimes emit idle after error. Preserve the error until actual new work.
        if task.session_closed {
            task.state = self.tasks.get(&key).filter(|t| matches!(t.state, State::Ended | State::Failed | State::Interrupted))
                .map_or(State::Unknown, |t| t.state);
        }
        if task.state == State::Ended {
            if let Some(old) = self
                .tasks
                .get(&key)
                .filter(|t| matches!(t.state, State::Failed | State::Interrupted))
            {
                task.state = old.state;
            }
        }
        let changed = self
            .tasks
            .get(&key)
            .is_none_or(|old| old.state != task.state);
        if changed {
            self.pending.remove(&key);
            if live && task.state.attention() && at.saturating_sub(task.updated_at) < 10_000 {
                self.pending.insert(key.clone(), at + 2000);
            }
        }
        self.tasks.insert(key, task);
        if self.tasks.len() > 100 {
            if let Some(key) = self
                .tasks
                .iter()
                .min_by_key(|(_, t)| t.updated_at)
                .map(|(k, _)| k.clone())
            {
                self.tasks.remove(&key);
                self.pending.remove(&key);
            }
        }
    }
    fn tick(&mut self, at: i64) -> Vec<Task> {
        self.tasks.retain(|_, t| at - t.updated_at <= MAX_AGE);
        for task in self.tasks.values_mut() {
            if at - task.updated_at > STALE
                && matches!(
                    task.state,
                    State::Working | State::WaitingApproval | State::WaitingInput
                )
            {
                task.state = State::Unknown;
            }
        }
        let mut alerts = Vec::new();
        self.pending.retain(|key, due| {
            if *due > at {
                return true;
            }
            if let Some(task) = self.tasks.get(key).filter(|t| t.state.attention()) {
                alerts.push(task.clone());
            }
            false
        });
        alerts
    }
    fn snapshot(&self) -> Vec<Task> {
        let mut tasks: Vec<_> = self.tasks.values().cloned().collect();
        tasks.sort_by_key(|t| {
            (
                !matches!(t.state, State::WaitingApproval | State::WaitingInput),
                std::cmp::Reverse(t.updated_at),
            )
        });
        tasks
    }
}
static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
fn store() -> &'static Mutex<Store> {
    STORE.get_or_init(Default::default)
}

#[tauri::command]
pub fn list_tasks() -> Vec<Task> {
    store().lock().unwrap_or_else(|e| e.into_inner()).snapshot()
}

#[tauri::command]
pub fn set_task_notifications(enabled: bool) -> Result<(), String> {
    crate::settings::try_edit(|s| s.task_notifications = enabled).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn test_task_notification(app: tauri::AppHandle) -> Result<(), String> {
    app.notification().builder().title("QuotaBar · 测试通知")
        .body("这是一条测试通知，不代表实际任务结束。能看到它就说明当前系统允许显示通知。")
        .show().map_err(|e| e.to_string())
}

fn drain(dir: &Path) -> Vec<Task> {
    let mut incoming = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten().filter(|e| e.path().extension().is_some_and(|x| x == "json")).take(512) {
            let path = entry.path();
            // Read errors are transient (Windows sharing locks, antivirus). Retry next tick.
            // Bound the actual read, not a metadata snapshot that can change before opening.
            let Ok(file) = std::fs::File::open(&path) else { continue; };
            let mut raw = Vec::new();
            if file.take(4097).read_to_end(&mut raw).is_err() { continue; }
            if raw.len() <= 4096 { if let Ok(task) = serde_json::from_slice::<Task>(&raw) { incoming.push(task); } }
            let _ = std::fs::remove_file(path);
        }
    }
    incoming.sort_by_key(|t| (t.updated_at, t.observed_at_ns));
    incoming
}

pub fn start(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let started_at = now();
        let mut previous = String::new();
        loop {
            let at = now();
            let incoming = drain(&directory());
            let (snapshot, alerts) = {
                let mut store = store().lock().unwrap_or_else(|e| e.into_inner());
                for task in incoming {
                    let live = task.updated_at >= started_at;
                    store.apply(task, at, live);
                }
                let alerts = store.tick(at);
                (store.snapshot(), alerts)
            };
            let serialized = serde_json::to_string(&snapshot).unwrap_or_default();
            if serialized != previous {
                let _ = app.emit("tasks://snapshot", snapshot);
                previous = serialized;
            }
            if !alerts.is_empty() && {
                let settings = crate::settings::load();
                settings.tos_accepted && settings.task_notifications
            } {
                for task in alerts {
                    let _ = app
                        .notification()
                        .builder()
                        .title("QuotaBar · 本机任务")
                        .body(format!(
                            "{} · {}：{}",
                            task.tool,
                            task.project,
                            task.state.label()
                        ))
                        .show();
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(750));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_close_must_not_cancel_completed_turn_notification() {
        let mut s = Store::default();
        s.apply(event("Stop", 100), 100, true);
        s.apply(event("SessionEnd", 101), 101, true);
        assert_eq!(s.snapshot()[0].state, State::Ended);
        assert_eq!(s.tick(2200).len(), 1);
        for terminal in ["StopFailure", "Interrupt"] {
            let mut state=Store::default();
            let ended=event(terminal,100);
            state.apply(ended.clone(),100,true);
            state.apply(event("SessionEnd",101),101,true);
            assert_eq!(state.snapshot()[0].state,ended.state);
            assert_eq!(state.tick(2200).len(),1);
        }
        let mut interrupted_without_evidence=Store::default();
        interrupted_without_evidence.apply(event("UserPromptSubmit",100),100,true);
        interrupted_without_evidence.apply(event("SessionEnd",101),101,true);
        assert_eq!(interrupted_without_evidence.snapshot()[0].state,State::Unknown);
        assert!(interrupted_without_evidence.tick(2200).is_empty());
    }
    #[test]
    fn separate_events_in_same_millisecond_must_not_be_treated_as_duplicates() {
        let mut s=Store::default();
        let mut first=event("UserPromptSubmit",100);first.observed_at_ns=100_000_001;
        let mut second=event("Stop",100);second.observed_at_ns=100_000_002;
        s.apply(first,100,true);s.apply(second,100,true);
        assert_eq!(s.snapshot()[0].state,State::Ended);
        assert_eq!(s.tick(2200).len(),1);
    }
    #[cfg(windows)]
    #[test]
    fn temporarily_unreadable_event_must_survive_drain() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir=crate::settings::app_data_dir().join("locked-event");
        write_event(&dir,&event("Stop",100)).unwrap();
        let path=dir.join(format!("100-{}.json",std::process::id()));
        let locked=std::fs::OpenOptions::new().read(true).share_mode(4).open(&path).unwrap();
        assert!(drain(&dir).is_empty());
        drop(locked);
        assert!(path.exists(), "transient read failure must not destroy the event");
        assert_eq!(drain(&dir).len(),1);
    }
    fn event(name: &str, at: i64) -> Task {
        normalize("codex", &serde_json::json!({"session_id":"one", "cwd":"C:\\secret\\demo", "hook_event_name":name, "prompt":"SECRET"}), at).unwrap()
    }
    #[test]
    fn metadata_only_and_explicit_events() {
        let t = event("Stop", 100);
        assert_eq!(t.project, "demo");
        assert_eq!(t.state, State::Ended);
        assert!(!serde_json::to_string(&t).unwrap().contains("SECRET"));
        assert!(normalize("kimi", &serde_json::json!({}), 0).is_none());
        assert!(normalize("codex", &serde_json::json!({"hook_event_name":"Stop"}), 0).is_none());
    }
    #[test]
    fn approval_resolution_cancels_debounced_alert() {
        let mut s = Store::default();
        s.apply(event("PermissionRequest", 100), 100, true);
        s.apply(event("PostToolUse", 200), 200, true);
        assert!(s.tick(3000).is_empty());
        s.apply(event("PermissionRequest", 4000), 4000, true);
        assert_eq!(s.tick(6100).len(), 1);
        assert!(s.tick(7000).is_empty());
    }
    #[test]
    fn replay_old_events_and_silence_never_mean_success() {
        let mut s = Store::default();
        s.apply(event("UserPromptSubmit", 100), 100, false);
        s.apply(event("Stop", 99), 100, true);
        s.tick(STALE + 101);
        assert_eq!(s.snapshot()[0].state, State::Unknown);
        s.apply(event("Stop", STALE + 200), STALE + 200, false);
        assert!(s.tick(STALE + 4000).is_empty());
    }
    #[test]
    fn independent_sessions_and_disk_roundtrip() {
        let mut s = Store::default();
        let a = event("Stop", 100);
        let mut b = a.clone();
        b.session_id = "two".into();
        s.apply(a.clone(), 100, false);
        s.apply(b, 100, false);
        assert_eq!(s.snapshot().len(), 2);
        let dir = crate::settings::app_data_dir().join("spool-test");
        write_event(&dir, &a).unwrap();
        let raw = std::fs::read(dir.join(format!("100-{}.json", std::process::id()))).unwrap();
        assert_eq!(
            serde_json::from_slice::<Task>(&raw).unwrap().state,
            State::Ended
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn error_survives_idle_and_new_work_allows_next_end() {
        let mut s = Store::default();
        s.apply(event("StopFailure", 10), 10, true);
        s.apply(event("Stop", 20), 20, true);
        assert_eq!(s.snapshot()[0].state, State::Failed);
        assert_eq!(s.tick(3000).len(), 1);
        s.apply(event("UserPromptSubmit", 3100), 3100, true);
        s.apply(event("Stop", 3200), 3200, true);
        assert_eq!(s.tick(6000)[0].state, State::Ended);
        s.apply(event("Stop", 3200), 6100, true);
        assert!(s.tick(9000).is_empty());
    }
    #[test]
    fn untrusted_timestamp_and_session_limit() {
        let mut s = Store::default();
        s.apply(event("Stop", i64::MIN), 1000, true);
        assert!(s.snapshot().is_empty());
        for i in 1..=110 {
            let mut t = event("Stop", i); t.session_id = i.to_string();
            s.apply(t, 1000, false);
        }
        assert_eq!(s.snapshot().len(), 100);
        assert!(s.snapshot().iter().all(|t| t.updated_at >= 11));
    }
}
