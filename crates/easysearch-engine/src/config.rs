// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Configuration for the search engine.

use std::path::PathBuf;

/// Configuration for [`SearchEngine`](crate::SearchEngine).
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Cache directory for `.flowcache` files.
    pub cache_dir: Option<PathBuf>,
    /// Drive letters to index automatically on startup.
    pub auto_index_drives: Vec<char>,
}

impl EngineConfig {
    /// Load configuration from environment variables.
    ///
    /// | Env var | Description |
    /// |---------|-------------|
    /// | `EASYSEARCH_CACHE_DIR` | Override cache directory |
    /// | `EASYSEARCH_DRIVES` | Comma-separated drive letters to index (default: `C`) |
    #[must_use]
    pub fn from_env() -> Self {
        let cache_dir = std::env::var_os("EASYSEARCH_CACHE_DIR").map(PathBuf::from);

        let auto_index_drives = std::env::var("EASYSEARCH_DRIVES")
            .unwrap_or_else(|_| String::from("C"))
            .split(',')
            .filter_map(|s| {
                let trimmed = s.trim();
                trimmed.chars().next().map(|c| c.to_ascii_uppercase())
            })
            .filter(|c| c.is_ascii_alphabetic())
            .collect();

        Self {
            cache_dir,
            auto_index_drives,
        }
    }

    /// Create a configuration with the specified drives and default cache directory.
    #[must_use]
    pub fn with_drives(drives: &[char]) -> Self {
        let cache_dir = dirs_cache_dir();
        Self {
            cache_dir,
            auto_index_drives: drives.iter().map(|c| c.to_ascii_uppercase()).collect(),
        }
    }
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

/// Returns the default cache directory: `%LOCALAPPDATA%\EasySearch\cache`
fn dirs_cache_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|base| {
        let mut p = PathBuf::from(base);
        p.push("EasySearch");
        p.push("cache");
        p
    })
}
