// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Settings persistence: load/save user configuration as JSON with atomic writes.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ─── Default value helpers ──────────────────────────────────────────────────

fn default_hotkey() -> String {
    "Alt+Space".to_string()
}

fn default_theme() -> String {
    "System".to_string()
}

fn default_language() -> String {
    String::new() // empty = auto-detect from OS locale
}

fn default_log_level() -> String {
    "warn".to_string()
}

// ─── Settings struct ────────────────────────────────────────────────────────

/// User-facing application settings, persisted as JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// Global hotkey binding string (e.g. "Alt+Space")
    #[serde(default = "default_hotkey")]
    pub hotkey: String,

    /// Active theme name
    #[serde(default = "default_theme")]
    pub theme: String,

    /// Language locale code (e.g. "en", "zh-CN", "ja"). Empty means auto-detect.
    #[serde(default = "default_language")]
    pub language: String,

    /// Minimum application log level: debug, info, warn, or error.
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Last window X position (None = center on screen)
    #[serde(default)]
    pub window_x: Option<i32>,

    /// Last window Y position (None = center on screen)
    #[serde(default)]
    pub window_y: Option<i32>,

    /// Plugin enabled/disabled state, keyed by plugin id
    #[serde(default)]
    pub plugins_enabled: HashMap<String, bool>,

    /// Whether to start with Windows
    #[serde(default)]
    pub autostart: bool,

    /// Drive letters to index (e.g. ["C", "D"]). Empty means scan all fixed drives.
    #[serde(default)]
    pub index_drives: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: default_hotkey(),
            theme: default_theme(),
            language: default_language(),
            log_level: default_log_level(),
            window_x: None,
            window_y: None,
            plugins_enabled: HashMap::new(),
            autostart: false,
            index_drives: Vec::new(),
        }
    }
}

// ─── SettingsStore ──────────────────────────────────────────────────────────

/// Handles loading and saving [`Settings`] to/from a JSON file.
pub struct SettingsStore;

impl SettingsStore {
    /// Load settings from the given path.
    ///
    /// - If the file does not exist, returns [`Settings::default()`].
    /// - If the file exists but contains invalid JSON, returns [`Settings::default()`].
    /// - Missing fields in JSON are filled with their default values via serde.
    pub fn load(path: &Path) -> Settings {
        match std::fs::read_to_string(path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Settings::default(),
        }
    }

    /// Save settings to the given path using atomic write.
    ///
    /// Strategy: serialize → write to `<path>.tmp` → rename over original.
    /// If the rename fails the original file is left untouched and the temp file
    /// is cleaned up on a best-effort basis.
    pub fn save(path: &Path, settings: &Settings) -> Result<(), String> {
        let json = serde_json::to_string_pretty(settings)
            .map_err(|e| format!("Failed to serialize settings: {e}"))?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create settings directory: {e}"))?;
        }

        // Atomic write: write to .tmp then rename
        let tmp_path = path.with_extension("json.tmp");

        std::fs::write(&tmp_path, json.as_bytes())
            .map_err(|e| format!("Failed to write temp file: {e}"))?;

        std::fs::rename(&tmp_path, path).map_err(|e| {
            // Clean up temp file on rename failure
            let _ = std::fs::remove_file(&tmp_path);
            format!("Failed to rename temp file: {e}")
        })?;

        Ok(())
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Helper: create a unique temp directory for test isolation.
    fn test_dir(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("easysearch_test_{}_{name}", std::process::id()));
        // Clean up any previous run
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn load_missing_file_returns_default() {
        let dir = test_dir("load_missing");
        let path = dir.join("nonexistent.json");
        let settings = SettingsStore::load(&path);
        assert_eq!(settings, Settings::default());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_corrupted_file_returns_default() {
        let dir = test_dir("load_corrupted");
        let path = dir.join("corrupted.json");
        fs::write(&path, "not valid json {{{").unwrap();

        let settings = SettingsStore::load(&path);
        assert_eq!(settings, Settings::default());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_partial_json_fills_defaults() {
        let dir = test_dir("load_partial");
        let path = dir.join("partial.json");
        fs::write(&path, r#"{"hotkey": "Ctrl+Space"}"#).unwrap();

        let settings = SettingsStore::load(&path);
        assert_eq!(settings.hotkey, "Ctrl+Space");
        assert_eq!(settings.theme, "System"); // default
        assert_eq!(settings.language, ""); // default
        assert_eq!(settings.autostart, false); // default

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = test_dir("save_roundtrip");
        let path = dir.join("settings.json");

        let mut settings = Settings::default();
        settings.hotkey = "Ctrl+Shift+S".to_string();
        settings.theme = "Win11Dark".to_string();
        settings.language = "zh-CN".to_string();
        settings.window_x = Some(100);
        settings.window_y = Some(200);
        settings.plugins_enabled.insert("calc".to_string(), true);
        settings.plugins_enabled.insert("web".to_string(), false);
        settings.autostart = true;

        SettingsStore::save(&path, &settings).unwrap();

        let loaded = SettingsStore::load(&path);
        assert_eq!(loaded, settings);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_creates_parent_directories() {
        let dir = test_dir("save_creates_dirs");
        let path = dir.join("nested").join("deep").join("settings.json");

        let settings = Settings::default();
        SettingsStore::save(&path, &settings).unwrap();

        assert!(path.exists());
        let loaded = SettingsStore::load(&path);
        assert_eq!(loaded, settings);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_atomic_does_not_leave_tmp_on_success() {
        let dir = test_dir("save_atomic");
        let path = dir.join("settings.json");
        let tmp_path = path.with_extension("json.tmp");

        let settings = Settings::default();
        SettingsStore::save(&path, &settings).unwrap();

        assert!(path.exists());
        assert!(!tmp_path.exists());

        let _ = fs::remove_dir_all(&dir);
    }
}
