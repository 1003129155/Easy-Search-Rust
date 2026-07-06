// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Program plugin settings — persisted to JSON.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Plugin settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramSettings {
    /// Maximum number of results to return.
    pub max_results: u32,

    /// Whether to filter out uninstaller shortcuts.
    pub hide_uninstallers: bool,
}

impl Default for ProgramSettings {
    fn default() -> Self {
        Self {
            max_results: 8,
            hide_uninstallers: true,
        }
    }
}

impl ProgramSettings {
    /// Load settings from the config file, or return defaults.
    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(settings) = serde_json::from_str(&content) {
                    return settings;
                }
            }
        }
        Self::default()
    }

    /// Save settings to the config file.
    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }

    /// Config file path: %LOCALAPPDATA%/EasySearch/cache/plugins/program/settings.json
    fn config_path() -> PathBuf {
        easysearch_core::paths::plugin_cache_dir("program").join("settings.json")
    }
}
