// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Win32 program scanner — enumerates Start Menu shortcuts (.lnk, .exe).

use crate::{ProgramEntry, ProgramSource};
use std::path::{Path, PathBuf};

/// Scan Start Menu directories for .lnk and .exe files.
pub fn scan_start_menu() -> Vec<ProgramEntry> {
    let mut entries = Vec::new();

    let start_menu_dirs: Vec<PathBuf> = [
        std::env::var_os("APPDATA")
            .map(|p| PathBuf::from(p).join(r"Microsoft\Windows\Start Menu\Programs")),
        std::env::var_os("ProgramData")
            .map(|p| PathBuf::from(p).join(r"Microsoft\Windows\Start Menu\Programs")),
    ]
    .into_iter()
    .flatten()
    .collect();

    for dir in &start_menu_dirs {
        scan_directory(dir, &mut entries);
    }

    entries
}

/// Recursively scan a directory for program shortcuts.
fn scan_directory(dir: &Path, entries: &mut Vec<ProgramEntry>) {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_directory(&path, entries);
        } else if is_program_file(&path) {
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            // Skip empty names
            if name.is_empty() {
                continue;
            }

            entries.push(ProgramEntry {
                name,
                path: path.to_string_lossy().to_string(),
                source: ProgramSource::StartMenu,
            });
        }
    }
}

/// Check if a file is a recognized program shortcut/executable.
fn is_program_file(path: &Path) -> bool {
    let Some(ext) = path.extension() else {
        return false;
    };
    let ext_lower = ext.to_string_lossy().to_lowercase();
    matches!(ext_lower.as_str(), "lnk" | "exe" | "appref-ms")
}
