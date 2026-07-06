// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Shared Quick Launch persistence for EasySearch.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickLaunchItem {
    pub title: String,
    pub path: String,
    pub is_directory: bool,
}

#[derive(Debug, Clone)]
pub struct QuickLaunchStore {
    items: Vec<QuickLaunchItem>,
    file_path: PathBuf,
}

impl QuickLaunchStore {
    #[must_use]
    pub fn load() -> Self {
        Self::load_from(default_store_path())
    }

    #[must_use]
    pub fn load_from(file_path: PathBuf) -> Self {
        let items = fs::read_to_string(&file_path)
            .ok()
            .and_then(|text| serde_json::from_str::<Vec<QuickLaunchItem>>(&text).ok())
            .unwrap_or_default();

        Self { items, file_path }
    }

    pub fn save(&self) -> std::io::Result<()> {
        save_items(&self.file_path, &self.items)
    }

    #[must_use]
    pub fn contains(&self, path: &str) -> bool {
        self.items.iter().any(|item| item.path.eq_ignore_ascii_case(path))
    }

    pub fn add(&mut self, item: QuickLaunchItem) {
        if self.contains(&item.path) {
            self.items.retain(|existing| !existing.path.eq_ignore_ascii_case(&item.path));
        }
        self.items.push(item);
    }

    pub fn remove(&mut self, path: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|item| !item.path.eq_ignore_ascii_case(path));
        self.items.len() != before
    }

    pub fn toggle(&mut self, path: &str, title: &str, is_directory: bool) -> bool {
        if self.remove(path) {
            false
        } else {
            self.add(QuickLaunchItem {
                title: title.to_string(),
                path: path.to_string(),
                is_directory,
            });
            true
        }
    }

    #[must_use]
    pub fn search(&self, query: &str) -> Vec<&QuickLaunchItem> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return self.items.iter().collect();
        }

        self.items
            .iter()
            .filter(|item| {
                item.title.to_lowercase().contains(&q) || item.path.to_lowercase().contains(&q)
            })
            .collect()
    }

    #[must_use]
    pub fn all(&self) -> &[QuickLaunchItem] {
        &self.items
    }
}

static STORE: OnceLock<Mutex<QuickLaunchStore>> = OnceLock::new();

#[must_use]
pub fn global_store() -> &'static Mutex<QuickLaunchStore> {
    STORE.get_or_init(|| Mutex::new(QuickLaunchStore::load()))
}

fn default_store_path() -> PathBuf {
    easysearch_core::paths::quick_launch_file()
}

fn save_items(path: &Path, items: &[QuickLaunchItem]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_vec_pretty(items)
        .map_err(|err| std::io::Error::other(format!("serialize quick launch: {err}")))?;
    let temp_path = path.with_extension("json.tmp");
    fs::write(&temp_path, json)?;

    if path.exists() {
        let _ = fs::remove_file(path);
    }
    fs::rename(&temp_path, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{QuickLaunchItem, QuickLaunchStore};

    fn unique_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "easysearch-{name}-{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ))
    }

    #[test]
    fn load_missing_file_returns_empty_store() {
        let path = unique_path("missing");
        let store = QuickLaunchStore::load_from(path);
        assert!(store.all().is_empty());
    }

    #[test]
    fn toggle_adds_and_removes_items() {
        let path = unique_path("toggle");
        let mut store = QuickLaunchStore::load_from(path.clone());

        assert!(store.toggle("C:\\demo.txt", "demo", false));
        assert!(store.contains("C:\\demo.txt"));
        assert_eq!(store.all().len(), 1);

        assert!(!store.toggle("C:\\demo.txt", "demo", false));
        assert!(!store.contains("C:\\demo.txt"));
        assert!(store.all().is_empty());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn save_and_reload_round_trips_items() {
        let path = unique_path("save");
        let mut store = QuickLaunchStore::load_from(path.clone());
        store.add(QuickLaunchItem {
            title: "Docs".to_string(),
            path: "C:\\Docs".to_string(),
            is_directory: true,
        });
        store.save().unwrap();

        let loaded = QuickLaunchStore::load_from(path.clone());
        assert_eq!(loaded.all().len(), 1);
        assert_eq!(loaded.all()[0].title, "Docs");

        let _ = std::fs::remove_file(path);
    }
}
