// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Usage history tracking for boosting frequently used items.
//!
//! Persists a simple JSON file at `%LOCALAPPDATA%\EasySearch\history.json`.
//! Each entry records how many times an action was executed.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// History store mapping action keys to execution counts.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct History {
    entries: HashMap<String, u32>,
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
}

/// Returns the path to `%LOCALAPPDATA%\EasySearch\history.json`.
fn history_file_path() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("EasySearch").join("history.json")
}
