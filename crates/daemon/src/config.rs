// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Runtime configuration for `easysearch`.

use std::path::PathBuf;

use crate::ipc::pipe_name_for_current_user;

/// Configuration loaded at process startup.
#[derive(Debug, Clone)]
pub(crate) struct EsConfig {
    /// Cache directory for `.flowcache` files.
    pub(crate) cache_dir: Option<PathBuf>,
    /// Named pipe path that the client should connect to.
    pub(crate) pipe_name: String,
    /// Drive letters to index automatically on startup.
    pub(crate) auto_index_drives: Vec<char>,
}

impl EsConfig {
    /// Load configuration from environment variables.
    ///
    /// | Env var | Description |
    /// |---------|-------------|
    /// | `EASYSEARCH_CACHE_DIR` | Override cache directory |
    /// | `EASYSEARCH_DRIVES` | Comma-separated drive letters to index (default: `C`) |
    /// | `EASYSEARCH_PIPE` | Override the named-pipe path (default: per-user hashed name) |
    #[must_use]
    pub(crate) fn from_env() -> Self {
        let cache_dir = std::env::var_os("EASYSEARCH_CACHE_DIR").map(PathBuf::from);

        let pipe_name = std::env::var("EASYSEARCH_PIPE")
            .ok()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(pipe_name_for_current_user);

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
            pipe_name,
            auto_index_drives,
        }
    }
}
