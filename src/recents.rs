use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::config::config_dir;
use crate::db::QueryResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentEntry {
    pub query: String,
    pub connection: Option<String>,
    pub timestamp: u64,
    pub duration_ms: u64,
    pub result: Option<QueryResult>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Recents {
    pub entries: Vec<RecentEntry>,
}

impl Recents {
    pub fn load() -> Self {
        let path = config_dir().join("recents.json");
        let Ok(content) = fs::read_to_string(&path) else {
            return Self::default();
        };
        match serde_json::from_str(&content) {
            Ok(recents) => recents,
            Err(e) => {
                warn!("failed to parse recents.json, starting fresh: {e}");
                Self::default()
            }
        }
    }

    pub fn save(&self) {
        let path = config_dir().join("recents.json");
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = fs::write(&path, json) {
                    warn!("failed to write recents.json: {e}");
                }
            }
            Err(e) => warn!("failed to serialize recents: {e}"),
        }
    }

    pub fn add(&mut self, entry: RecentEntry, max_entries: usize) {
        self.entries.insert(0, entry);
        self.entries.truncate(max_entries);
    }
}

pub fn format_relative_time(epoch_secs: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let delta = now.saturating_sub(epoch_secs);
    if delta < 60 {
        "just now".to_string()
    } else if delta < 3600 {
        format!("{}m ago", delta / 60)
    } else if delta < 86400 {
        format!("{}h ago", delta / 3600)
    } else {
        format!("{}d ago", delta / 86400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Value;

    fn make_entry(query: &str, has_result: bool, error: Option<&str>) -> RecentEntry {
        RecentEntry {
            query: query.to_string(),
            connection: Some("testdb".to_string()),
            timestamp: 1700000000,
            duration_ms: 42,
            result: if has_result {
                Some(QueryResult {
                    columns: vec!["id".into(), "name".into()],
                    rows: vec![vec![Value::Int(1), Value::Text("alice".into())]],
                })
            } else {
                None
            },
            error: error.map(String::from),
        }
    }

    #[test]
    fn add_entry_prepends() {
        let mut recents = Recents::default();
        recents.add(make_entry("SELECT 1", true, None), 10);
        recents.add(make_entry("SELECT 2", true, None), 10);
        assert_eq!(recents.entries[0].query, "SELECT 2");
        assert_eq!(recents.entries[1].query, "SELECT 1");
    }

    #[test]
    fn add_entry_enforces_max() {
        let mut recents = Recents::default();
        recents.add(make_entry("SELECT 1", true, None), 2);
        recents.add(make_entry("SELECT 2", true, None), 2);
        recents.add(make_entry("SELECT 3", true, None), 2);
        assert_eq!(recents.entries.len(), 2);
        assert_eq!(recents.entries[0].query, "SELECT 3");
        assert_eq!(recents.entries[1].query, "SELECT 2");
    }

    #[test]
    fn serialize_roundtrip_success() {
        let entry = make_entry("SELECT * FROM users", true, None);
        let recents = Recents { entries: vec![entry] };
        let json = serde_json::to_string(&recents).unwrap();
        let back: Recents = serde_json::from_str(&json).unwrap();
        assert_eq!(back.entries.len(), 1);
        assert_eq!(back.entries[0].query, "SELECT * FROM users");
        assert!(back.entries[0].result.is_some());
        assert!(back.entries[0].error.is_none());
    }

    #[test]
    fn serialize_roundtrip_error() {
        let entry = make_entry("BAD SQL", false, Some("syntax error"));
        let recents = Recents { entries: vec![entry] };
        let json = serde_json::to_string(&recents).unwrap();
        let back: Recents = serde_json::from_str(&json).unwrap();
        assert_eq!(back.entries[0].error.as_deref(), Some("syntax error"));
        assert!(back.entries[0].result.is_none());
    }

    #[test]
    fn deserialize_corrupt_returns_none() {
        let result = serde_json::from_str::<Recents>("not json");
        assert!(result.is_err());
    }

    #[test]
    fn format_relative_time_just_now() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert_eq!(format_relative_time(now), "just now");
    }

    #[test]
    fn format_relative_time_minutes() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert_eq!(format_relative_time(now - 120), "2m ago");
    }

    #[test]
    fn format_relative_time_hours() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert_eq!(format_relative_time(now - 7200), "2h ago");
    }

    #[test]
    fn format_relative_time_days() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert_eq!(format_relative_time(now - 172800), "2d ago");
    }
}
