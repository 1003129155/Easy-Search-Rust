// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Usage history tracking for boosting frequently used items.
//!
//! Persists a simple JSON file at `%LOCALAPPDATA%\EasySearch\history.json`.
//! Each entry records how many times an action was executed.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// History store mapping action keys to execution counts,
/// plus a set of pinned (top-most) items per query.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct History {
    entries: HashMap<String, u32>,
    /// Pinned items: maps query (lowercased) → list of action keys that are pinned.
    #[serde(default)]
    pinned: HashMap<String, Vec<String>>,
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
