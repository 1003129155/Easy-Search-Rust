// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Unified application data paths for EasySearch.
//!
//! All persistent files (settings, caches, user data) live under a single
//! root directory:
//!
//! ```text
//! %LOCALAPPDATA%\EasySearch\          (Windows)
//! ~/Library/Application Support/EasySearch/  (macOS)
//! $XDG_DATA_HOME/EasySearch/          (Linux, default ~/.local/share/EasySearch/)
//! ```
//!
//! # Directory Layout
//!
//! ```text
//! %LOCALAPPDATA%\EasySearch\
//! ├── settings.json
//! ├── cache\
//! │   ├── index\            ← MFT .uffs index files
//! │   ├── flow\             ← .flowcache files
//! │   └── plugins\
//! │       └── program\
//! │           └── cache.json
//! └── data\
//!     └── quick_launch.json
//! ```

use std::path::PathBuf;

/// Brand name used as the top-level directory.
const APP_NAME: &str = "EasySearch";

// ────────────────────────────────────────────────────────────────────────────
// Root
// ────────────────────────────────────────────────────────────────────────────

/// Returns the application root directory.
///
/// - **Windows**: `%LOCALAPPDATA%\EasySearch\`
/// - **macOS**: `~/Library/Application Support/EasySearch/`
/// - **Linux**: `$XDG_DATA_HOME/EasySearch/`
///
/// Falls back to `.` if the platform directory cannot be determined.
#[must_use]
pub fn app_root_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_NAME)
}

// ────────────────────────────────────────────────────────────────────────────
// Settings
// ────────────────────────────────────────────────────────────────────────────

/// Path to the main settings file: `<root>/settings.json`
#[must_use]
pub fn settings_file() -> PathBuf {
    app_root_dir().join("settings.json")
}

// ────────────────────────────────────────────────────────────────────────────
// Cache — Index
// ────────────────────────────────────────────────────────────────────────────

/// Directory for MFT `.uffs` index cache files: `<root>/cache/index/`
#[must_use]
pub fn index_cache_dir() -> PathBuf {
    app_root_dir().join("cache").join("index")
}

// ────────────────────────────────────────────────────────────────────────────
// Cache — FlowCache
// ────────────────────────────────────────────────────────────────────────────

/// Directory for `.flowcache` files: `<root>/cache/flow/`
#[must_use]
pub fn flow_cache_dir() -> PathBuf {
    app_root_dir().join("cache").join("flow")
}

// ────────────────────────────────────────────────────────────────────────────
// Cache — Plugins
// ────────────────────────────────────────────────────────────────────────────

/// Directory for plugin cache files: `<root>/cache/plugins/<plugin_name>/`
#[must_use]
pub fn plugin_cache_dir(plugin_name: &str) -> PathBuf {
    app_root_dir()
        .join("cache")
        .join("plugins")
        .join(plugin_name)
}

// ────────────────────────────────────────────────────────────────────────────
// Data
// ────────────────────────────────────────────────────────────────────────────

/// Directory for user data files: `<root>/data/`
#[must_use]
pub fn data_dir() -> PathBuf {
    app_root_dir().join("data")
}

/// Path to the quick launch store file: `<root>/data/quick_launch.json`
#[must_use]
pub fn quick_launch_file() -> PathBuf {
    data_dir().join("quick_launch.json")
}

// ────────────────────────────────────────────────────────────────────────────
// Legacy paths (for migration)
// ────────────────────────────────────────────────────────────────────────────

/// Legacy MFT index cache directory: `%LOCALAPPDATA%\uffs\cache\`
///
/// Used only for one-time migration to the unified location.
#[must_use]
pub fn legacy_mft_cache_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("uffs")
        .join("cache")
}

/// Legacy program plugin cache: `%APPDATA%\EasySearch\plugins\program\cache.json`
///
/// Used only for one-time migration to the unified location.
#[must_use]
pub fn legacy_program_cache_file() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("EasySearch")
        .join("plugins")
        .join("program")
        .join("cache.json")
}

/// Legacy quick launch data: `%APPDATA%\EasySearch\quick_launch.json`
///
/// Used only for one-time migration to the unified location.
#[must_use]
pub fn legacy_quick_launch_file() -> PathBuf {
    dirs::data_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("EasySearch")
        .join("quick_launch.json")
}

// ────────────────────────────────────────────────────────────────────────────
// Migration Helpers
// ────────────────────────────────────────────────────────────────────────────

/// Migrate a single file from `src` to `dst` if `src` exists and `dst` does not.
///
/// Creates parent directories for `dst` as needed. Best-effort, returns
/// `true` on success.
pub fn migrate_file(src: &std::path::Path, dst: &std::path::Path) -> bool {
    if !src.exists() || dst.exists() {
        return false;
    }
    if let Some(parent) = dst.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return false;
        }
    }
    // Try rename first (fast, same filesystem), fall back to copy+remove
    if std::fs::rename(src, dst).is_ok() {
        return true;
    }
    if let Ok(data) = std::fs::read(src) {
        if std::fs::write(dst, &data).is_ok() {
            let _ = std::fs::remove_file(src);
            return true;
        }
    }
    false
}

/// Migrate all files with a given extension from `src_dir` to `dst_dir`.
///
/// Best-effort; returns the number of files successfully migrated.
pub fn migrate_dir_files(src_dir: &std::path::Path, dst_dir: &std::path::Path, ext: &str) -> u32 {
    if !src_dir.is_dir() {
        return 0;
    }
    let Ok(entries) = std::fs::read_dir(src_dir) else {
        return 0;
    };
    let mut count = 0u32;
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().ends_with(ext) {
            if migrate_file(&entry.path(), &dst_dir.join(&name)) {
                count += 1;
            }
        }
    }
    // Remove src_dir if now empty
    let _ = std::fs::remove_dir(src_dir);
    count
}
