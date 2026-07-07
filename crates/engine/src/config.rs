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
    /// | `EASYSEARCH_CACHE_DIR` | Override cache directory (default: `%LOCALAPPDATA%\EasySearch\cache\flow\`) |
    /// | `EASYSEARCH_DRIVES` | Comma-separated drive letters to index (default: all fixed NTFS drives) |
    #[must_use]
    pub fn from_env() -> Self {
        let cache_dir = std::env::var_os("EASYSEARCH_CACHE_DIR")
            .map(PathBuf::from)
            .or_else(dirs_cache_dir);

        let auto_index_drives = match std::env::var("EASYSEARCH_DRIVES") {
            Ok(val) => val
                .split(',')
                .filter_map(|s| {
                    let trimmed = s.trim();
                    trimmed.chars().next().map(|c| c.to_ascii_uppercase())
                })
                .filter(|c| c.is_ascii_alphabetic())
                .collect(),
            Err(_) => detect_all_fixed_drives(),
        };

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

/// Detect all fixed drives on the system using the Windows API.
///
/// Returns drive letters (e.g. `['C', 'D', 'E']`) for all fixed (non-removable,
/// non-network) drives. Falls back to `['C']` on non-Windows or on failure.
#[cfg(windows)]
fn detect_all_fixed_drives() -> Vec<char> {
    use windows::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};

    /// DRIVE_FIXED = 3 (local hard disk)
    const DRIVE_FIXED: u32 = 3;

    let mask = unsafe { GetLogicalDrives() };
    if mask == 0 {
        return vec!['C'];
    }

    let mut drives = Vec::new();
    for i in 0u32..26 {
        if mask & (1 << i) != 0 {
            let letter = (b'A' + i as u8) as char;
            // Check if the drive is a fixed (local) drive
            let root: Vec<u16> = format!("{}:\\\0", letter).encode_utf16().collect();
            let drive_type = unsafe { GetDriveTypeW(windows::core::PCWSTR(root.as_ptr())) };
            if drive_type == DRIVE_FIXED {
                drives.push(letter);
            }
        }
    }

    if drives.is_empty() {
        vec!['C']
    } else {
        drives
    }
}

#[cfg(not(windows))]
fn detect_all_fixed_drives() -> Vec<char> {
    vec!['C']
}

/// Returns the default cache directory for `.flowcache` files:
/// `%LOCALAPPDATA%\EasySearch\cache\flow\`
fn dirs_cache_dir() -> Option<PathBuf> {
    Some(easysearch_core::paths::flow_cache_dir())
}
