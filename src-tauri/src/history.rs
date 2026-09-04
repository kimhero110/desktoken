// QuotaBar — E8 local usage history. SQLite ring buffer, 7-day retention.
// Every successful snapshot is recorded; the detail card renders 7-day
// sparklines from it. Local-only, never leaves the machine (PLAN: 零遥测).
use crate::providers::QuotaSnapshot;
use rusqlite::Connection;
use std::sync::Mutex;

static DB: std::sync::OnceLock<Mutex<Connection>> = std::sync::OnceLock::new();

fn db_path() -> std::path::PathBuf {
    let dir = crate::settings::app_data_dir();
    let _ = std::fs::create_dir_all(&dir);
    dir.join("history.db")
}

fn db() -> &'static Mutex<Connection> {
    DB.get_or_init(|| {
        let conn = Connection::open(db_path()).expect("open history.db");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS samples (
                provider TEXT NOT NULL,
                label    TEXT NOT NULL,
                ts       INTEGER NOT NULL,
                used_pct REAL NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_samples ON samples(provider, label, ts);",
        )
        .expect("create samples table");
        Mutex::new(conn)
    })
}

/// Record one snapshot (called on every successful poll).
pub fn record(snap: &QuotaSnapshot) {
    if snap.error.is_some() || snap.windows.is_empty() {
        return;
    }
    let Ok(conn) = db().lock() else { return };
    let ts = snap.fetched_at;
    for w in &snap.windows {
        let _ = conn.execute(
            "INSERT INTO samples (provider, label, ts, used_pct) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![snap.provider_id, w.label, ts, w.used_percent],
        );
    }
    // 7-day retention
    let cutoff = ts - 7 * 86400;
    let _ = conn.execute("DELETE FROM samples WHERE ts < ?1", [cutoff]);
}

/// All window series for one provider: { label: [(ts, pct)] }.
pub fn provider_history(provider: &str) -> std::collections::BTreeMap<String, Vec<(i64, f64)>> {
    let mut out = std::collections::BTreeMap::new();
    let Ok(conn) = db().lock() else { return out };
    let cutoff = crate::providers::now_secs() - 7 * 86400;
    let mut stmt = match conn.prepare(
        "SELECT label, ts, used_pct FROM samples WHERE provider = ?1 AND ts >= ?2 ORDER BY ts",
    ) {
        Ok(s) => s,
        Err(_) => return out,
    };
    let rows = stmt
        .query_map(rusqlite::params![provider, cutoff], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, f64>(2)?,
            ))
        })
        .map(|r| r.flatten().collect::<Vec<_>>())
        .unwrap_or_default();
    for (label, ts, pct) in rows {
        out.entry(label).or_insert_with(Vec::new).push((ts, pct));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::QuotaWindow;

    fn snap_for(id: &str, pct: f64) -> QuotaSnapshot {
        QuotaSnapshot::ok(
            id,
            id,
            None,
            vec![QuotaWindow {
                label: "5h".into(),
                used_percent: pct,
                resets_at: None,
            }],
            "official",
        )
    }

    #[test]
    fn record_and_read_back() {
        // unique provider id per run: tests share the same db file
        let id = format!("test-{}", crate::providers::now_secs());
        record(&snap_for(&id, 12.0));
        record(&snap_for(&id, 34.0));
        let h = provider_history(&id);
        let s = h.get("5h").expect("5h series");
        assert_eq!(s.len(), 2);
        assert!((s[0].1 - 12.0).abs() < 0.01);
        assert!((s[1].1 - 34.0).abs() < 0.01);
    }

    #[test]
    fn error_snapshots_not_recorded() {
        let id = format!("test-err-{}", crate::providers::now_secs());
        let e = QuotaSnapshot::err(&id, &id, &crate::providers::ProviderError::Network);
        record(&e);
        assert!(provider_history(&id).is_empty());
    }
}
