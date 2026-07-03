// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Program launcher plugin.
//! Scans Start Menu shortcuts and PATH executables.

use easysearch_core::{Action, Plugin, PluginResult};
use std::path::PathBuf;

pub struct ProgramPlugin {
    programs: Vec<ProgramEntry>,
}

#[derive(Debug, Clone)]
struct ProgramEntry {
    name: String,
    path: String,
}

impl ProgramPlugin {
    #[must_use]
    pub fn new() -> Self {
        let programs = scan_programs();
        Self { programs }
    }
}

impl Default for ProgramPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ProgramPlugin {
    fn default_keyword(&self) -> Option<&str> {
        None // programs match by content
    }

    fn matches(&self, _query: &str) -> bool {
        true
    }

    fn query(&self, query: &str) -> Vec<PluginResult> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }

        self.programs
            .iter()
            .filter(|p| p.name.to_lowercase().contains(&q))
            .take(8)
            .enumerate()
            .map(|(i, p)| PluginResult {
                title: p.name.clone(),
                subtitle: p.path.clone(),
                icon: p.path.clone(),
                action: Action::Open(p.path.clone()),
                score: 600 - i as u32,
            })
            .collect()
    }

    fn name(&self) -> &str {
        "Program"
    }
}

fn scan_programs() -> Vec<ProgramEntry> {
    let mut entries = Vec::new();

    let start_menu_dirs: Vec<PathBuf> = [
        std::env::var_os("APPDATA")
            .map(|p| PathBuf::from(p).join("Microsoft/Windows/Start Menu/Programs")),
        std::env::var_os("ProgramData")
            .map(|p| PathBuf::from(p).join("Microsoft/Windows/Start Menu/Programs")),
    ]
    .into_iter()
    .flatten()
    .collect();

    for dir in start_menu_dirs {
        scan_directory_for_links(&dir, &mut entries);
    }

    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    entries.dedup_by(|a, b| a.name.to_lowercase() == b.name.to_lowercase());

    entries
}

fn scan_directory_for_links(dir: &PathBuf, entries: &mut Vec<ProgramEntry>) {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_directory_for_links(&path, entries);
        } else if let Some(ext) = path.extension() {
            let ext_lower = ext.to_string_lossy().to_lowercase();
            if ext_lower == "lnk" || ext_lower == "exe" {
                let name = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                entries.push(ProgramEntry {
                    name,
                    path: path.to_string_lossy().to_string(),
                });
            }
        }
    }
}
