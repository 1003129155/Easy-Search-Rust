// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Disk cache for the program index.
//!
//! Saves the scanned program list as JSON. On startup, the plugin loads
//! from cache for instant results, then rebuilds in background if stale.

use crate::ProgramEntry;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// Maximum age before the cache is considered stale (30 minutes).
const CACHE_MAX_AGE: Duration = Duration::from_secs(30 * 60);

/// In-memory representation of a loaded cache.
pub struct ProgramCache {
    pub entries: Vec<ProgramEntry>,
    /// When the cache file was last modified.
    pub modified: Option<SystemTime>,
}

impl ProgramCache {
    /// Load cache from disk. Returns `None` if cache doesn't exist or is unreadable.
    pub fn load() -> Option<Self> {
        let path = cache_path();
        let metadata = std::fs::metadata(&path).ok()?;
        let modified = metadata.modified().ok();
        let content = std::fs::read_to_string(&path).ok()?;
        let entries: Vec<ProgramEntry> = serde_json::from_str(&content).ok()?;

        Some(Self { entries, modified })
    }

    /// Whether the cache is older than the refresh interval.
    pub fn is_stale(&self) -> bool {
        let Some(modified) = self.modified else {
            return true;
        };
        let Ok(elapsed) = modified.elapsed() else {
            return true;
        };
        elapsed > CACHE_MAX_AGE
    }

    /// Save a program list to the cache file.
    pub fn save(entries: &[ProgramEntry]) {
        let path = cache_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string(entries) {
            let _ = std::fs::write(&path, json);
        }
    }
}

/// Cache file location: %APPDATA%/EasySearch/plugins/program/cache.json
fn cache_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("EasySearch")
        .join("plugins")
        .join("program")
        .join("cache.json")
}
