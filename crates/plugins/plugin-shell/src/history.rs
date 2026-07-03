// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Command history — tracks execution counts, persisted to JSON.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Persisted command history with execution counts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandHistory {
    /// Map of command string -> execution count.
    commands: HashMap<String, u32>,
}

impl CommandHistory {
    /// Create an empty history.
    #[must_use]
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    /// Load history from disk, or return empty if not found.
    #[must_use]
    pub fn load() -> Self {
        let path = Self::history_path();
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(history) = serde_json::from_str(&content) {
                    return history;
                }
            }
        }
        Self::new()
    }

    /// Save history to disk.
    pub fn save(&self) {
        let path = Self::history_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }

    /// Record a command execution (increments count).
    pub fn record(&mut self, cmd: &str) {
        let cmd = cmd.trim().to_string();
        if cmd.is_empty() {
            return;
        }
        *self.commands.entry(cmd).or_insert(0) += 1;
        self.save();
    }

    /// Get the execution count for a command.
    #[must_use]
    pub fn count(&self, cmd: &str) -> u32 {
        self.commands.get(cmd).copied().unwrap_or(0)
    }

    /// Iterate over all (command, count) pairs.
    pub fn entries(&self) -> impl Iterator<Item = (&String, &u32)> {
        self.commands.iter()
    }

    /// Check if history is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Number of unique commands in history.
    #[must_use]
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Remove a command from history.
    pub fn remove(&mut self, cmd: &str) {
        self.commands.remove(cmd);
        self.save();
    }

    /// Clear all history.
    pub fn clear(&mut self) {
        self.commands.clear();
        self.save();
    }

    /// History file path: %APPDATA%/EasySearch/plugins/shell/history.json
    fn history_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("EasySearch")
            .join("plugins")
            .join("shell")
            .join("history.json")
    }
}

impl Default for CommandHistory {
    fn default() -> Self {
        Self::new()
    }
}
