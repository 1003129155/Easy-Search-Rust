// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Usage history tracking for boosting frequently used items.
//!
//! Persists a simple JSON file at `%LOCALAPPDATA%\EasySearch\history.json`.
//! Each entry records how many times an action was executed, plus full metadata
//! for the home-screen recent-items panel.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Maximum number of recent items kept in persistent storage.
const MAX_RECENT: usize = 20;

/// Lightweight snapshot of a recently executed item, stored for the home screen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentItem {
    pub title: String,
    pub subtitle: String,
    pub icon: String,
    pub action_key: String,
    #[serde(default)]
    pub is_directory: bool,
}

/// History store mapping action keys to execution counts,
/// plus recent items with full display metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct History {
    entries: HashMap<String, u32>,
    /// Recently used items with display metadata, sorted by count desc.
    #[serde(default)]
    recent: Vec<RecentItem>,
}

impl History {
    /// Load history from the default file path, or return empty on failure.
    pub fn load() -> Self {
        let path = history_file_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Record an execution of an action (identified by `key`).
    pub fn record(&mut self, key: &str) {
        let count = self.entries.entry(key.to_string()).or_insert(0);
        *count = count.saturating_add(1);
    }

    /// Record a full item execution with display metadata for the home screen.
    /// Items are kept in chronological order (newest last in storage,
    /// reversed on display so newest appears first).
    pub fn record_full(
        &mut self,
        key: &str,
        title: &str,
        subtitle: &str,
        icon: &str,
        is_directory: bool,
    ) {
        self.record(key);

        // Remove existing entry for this action key (dedup by key).
        self.recent.retain(|r| r.action_key != key);

        // Push as the newest entry (at the end).
        self.recent.push(RecentItem {
            title: title.to_string(),
            subtitle: subtitle.to_string(),
            icon: icon.to_string(),
            action_key: key.to_string(),
            is_directory,
        });

        // Drop oldest entries from the front if over capacity.
        while self.recent.len() > MAX_RECENT {
            self.recent.remove(0);
        }
    }

    /// Top N recent items, newest first (for the home screen).
    #[must_use]
    pub fn top_recent(&self, n: usize) -> Vec<&RecentItem> {
        self.recent.iter().rev().take(n).collect()
    }

    /// Get the usage count for a key.
    pub fn count(&self, key: &str) -> u32 {
        self.entries.get(key).copied().unwrap_or(0)
    }

    /// Save history to disk (best-effort, non-fatal).
    pub fn save(&self) {
        let path = history_file_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }

    /// Compute a boost score for an item based on usage frequency.
    /// Returns 0 for never-used items, up to 100 for heavily used items.
    pub fn boost_score(&self, key: &str) -> u32 {
        let count = self.count(key);
        // Logarithmic scaling: 1→20, 3→40, 10→60, 30→80, 100+→100
        match count {
            0 => 0,
            1..=2 => 20,
            3..=9 => 40,
            10..=29 => 60,
            30..=99 => 80,
            _ => 100,
        }
    }
}

/// Returns the path to `%LOCALAPPDATA%\EasySearch\history.json`.
fn history_file_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("EasySearch").join("history.json")
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_increments_count() {
        let mut h = History::default();
        assert_eq!(h.count("open:a"), 0);
        h.record("open:a");
        h.record("open:a");
        assert_eq!(h.count("open:a"), 2);
        assert_eq!(h.count("open:b"), 0);
    }

    #[test]
    fn boost_score_buckets() {
        let mut h = History::default();
        assert_eq!(h.boost_score("k"), 0);

        h.record("k"); // 1
        assert_eq!(h.boost_score("k"), 20);

        h.record("k"); // 2
        assert_eq!(h.boost_score("k"), 20);

        h.record("k"); // 3
        assert_eq!(h.boost_score("k"), 40);

        for _ in 0..7 {
            h.record("k"); // 10
        }
        assert_eq!(h.boost_score("k"), 60);

        for _ in 0..20 {
            h.record("k"); // 30
        }
        assert_eq!(h.boost_score("k"), 80);

        for _ in 0..70 {
            h.record("k"); // 100
        }
        assert_eq!(h.boost_score("k"), 100);
    }

    #[test]
    fn record_full_dedups_by_key_and_keeps_newest() {
        let mut h = History::default();
        h.record_full("open:a", "A", "sub", "icon", false);
        h.record_full("open:b", "B", "sub", "icon", false);
        // Re-record "a" — should dedup, move it to newest, and bump count.
        h.record_full("open:a", "A2", "sub2", "icon2", true);

        let recent = h.top_recent(10);
        // Newest first: a (updated), then b.
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].action_key, "open:a");
        assert_eq!(recent[0].title, "A2");
        assert_eq!(recent[0].is_directory, true);
        assert_eq!(recent[1].action_key, "open:b");
        // Count reflects both records of "a".
        assert_eq!(h.count("open:a"), 2);
    }

    #[test]
    fn top_recent_returns_newest_first_and_limits() {
        let mut h = History::default();
        for i in 0..5 {
            h.record_full(&format!("open:{i}"), &format!("T{i}"), "", "", false);
        }
        let recent = h.top_recent(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].action_key, "open:4");
        assert_eq!(recent[1].action_key, "open:3");
        assert_eq!(recent[2].action_key, "open:2");
    }

    #[test]
    fn recent_capacity_is_bounded() {
        let mut h = History::default();
        for i in 0..(MAX_RECENT + 10) {
            h.record_full(&format!("open:{i}"), "T", "", "", false);
        }
        // Never exceeds MAX_RECENT stored entries.
        assert_eq!(h.top_recent(usize::MAX).len(), MAX_RECENT);
        // The newest entry survives; the oldest are dropped.
        let recent = h.top_recent(1);
        assert_eq!(recent[0].action_key, format!("open:{}", MAX_RECENT + 9));
    }

    #[test]
    fn serde_roundtrip_preserves_state() {
        let mut h = History::default();
        h.record_full("open:a", "A", "subA", "iconA", true);
        h.record("open:a");
        let json = serde_json::to_string(&h).unwrap();
        let restored: History = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.count("open:a"), 2);
        let recent = restored.top_recent(1);
        assert_eq!(recent[0].title, "A");
        assert_eq!(recent[0].is_directory, true);
    }

    #[test]
    fn deserialize_partial_json_fills_defaults() {
        // Old format with only `entries` — recent should default.
        let json = r#"{"entries":{"open:x":5}}"#;
        let h: History = serde_json::from_str(json).unwrap();
        assert_eq!(h.count("open:x"), 5);
        assert!(h.top_recent(10).is_empty());
    }

    #[test]
    fn deserialize_legacy_pinned_data_ignores_removed_field() {
        let json = r#"{"entries":{"open:x":5},"pinned":{"query":["open:x"]}}"#;
        let h: History = serde_json::from_str(json).unwrap();
        assert_eq!(h.count("open:x"), 5);
    }
}
