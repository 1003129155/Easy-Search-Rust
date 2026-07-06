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
/// plus a set of pinned (top-most) items per query,
/// plus recent items with full display metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct History {
    entries: HashMap<String, u32>,
    /// Pinned items: maps query (lowercased) → list of action keys that are pinned.
    #[serde(default)]
    pinned: HashMap<String, Vec<String>>,
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
    pub fn record_full(&mut self, key: &str, title: &str, subtitle: &str, icon: &str, is_directory: bool) {
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

    /// Pin an item for a specific query (top-most).
    pub fn pin(&mut self, query: &str, action_key: &str) {
        let q = query.to_lowercase();
        let list = self.pinned.entry(q).or_default();
        if !list.contains(&action_key.to_string()) {
            list.push(action_key.to_string());
        }
    }

    /// Unpin an item.
    pub fn unpin(&mut self, query: &str, action_key: &str) {
        let q = query.to_lowercase();
        if let Some(list) = self.pinned.get_mut(&q) {
            list.retain(|k| k != action_key);
        }
    }

    /// Check if an item is pinned for a given query.
    pub fn is_pinned(&self, query: &str, action_key: &str) -> bool {
        let q = query.to_lowercase();
        self.pinned
            .get(&q)
            .map_or(false, |list| list.contains(&action_key.to_string()))
    }

    /// Get pinned position (0-based) for an item, or None if not pinned.
    pub fn pinned_position(&self, query: &str, action_key: &str) -> Option<usize> {
        let q = query.to_lowercase();
        self.pinned
            .get(&q)
            .and_then(|list| list.iter().position(|k| k == action_key))
    }
}

/// Returns the path to `%LOCALAPPDATA%\EasySearch\history.json`.
fn history_file_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("EasySearch").join("history.json")
}
