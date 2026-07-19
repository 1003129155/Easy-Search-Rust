// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Filesystem action helpers for search results.

#[cfg(windows)]
use std::path::{Path, PathBuf};

#[cfg(windows)]
pub fn open_containing_folder(path: &str) {
    let _ = std::process::Command::new("explorer.exe")
        .args(["/select,", path])
        .spawn();
}

#[cfg(windows)]
pub fn open_parent_folder(path: &str) {
    let target = if Path::new(path).is_dir() {
        PathBuf::from(path)
    } else {
        Path::new(path)
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(path))
    };

    let _ = std::process::Command::new("explorer.exe")
        .arg(target)
        .spawn();
}

#[cfg(windows)]
pub fn is_directory(path: &str) -> bool {
    Path::new(path).is_dir()
}
