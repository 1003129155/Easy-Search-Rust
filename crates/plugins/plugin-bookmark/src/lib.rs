// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Browser bookmark search plugin.
//! Reads bookmarks from Chrome/Edge/Firefox.

use easysearch_core::{Action, Plugin, PluginResult};

pub struct BookmarkPlugin {
    bookmarks: Vec<Bookmark>,
}

#[derive(Debug, Clone)]
struct Bookmark {
    name: String,
    url: String,
}

impl BookmarkPlugin {
    #[must_use]
    pub fn new() -> Self {
        let bookmarks = load_bookmarks();
        Self { bookmarks }
    }
}

impl Default for BookmarkPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for BookmarkPlugin {
    fn default_keyword(&self) -> Option<&str> {
        Some("b ")
    }

    fn query(&self, query: &str) -> Vec<PluginResult> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return self
                .bookmarks
                .iter()
                .take(8)
                .map(|b| bookmark_to_result(b))
                .collect();
        }

        self.bookmarks
            .iter()
            .filter(|b| {
                b.name.to_lowercase().contains(&q) || b.url.to_lowercase().contains(&q)
            })
            .take(8)
            .enumerate()
            .map(|(i, b)| {
                let mut r = bookmark_to_result(b);
                r.score = 800 - i as u32;
                r
            })
            .collect()
    }

    fn name(&self) -> &str {
        "Bookmark"
    }
}

fn bookmark_to_result(b: &Bookmark) -> PluginResult {
    PluginResult {
        title: b.name.clone(),
        subtitle: b.url.clone(),
        icon: String::from("bookmark"),
        action: Action::Open(b.url.clone()),
        score: 700,
    }
}

fn load_bookmarks() -> Vec<Bookmark> {
    let mut all = Vec::new();

    // Chrome / Edge bookmarks
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let local = std::path::PathBuf::from(local_app_data);
        let chrome_path = local.join("Google/Chrome/User Data/Default/Bookmarks");
        let edge_path = local.join("Microsoft/Edge/User Data/Default/Bookmarks");

        for path in [chrome_path, edge_path] {
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    parse_chromium_bookmarks(&content, &mut all);
                }
            }
        }
    }

    // Firefox bookmarks (from profile's bookmarkbackups JSON)
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let profiles_dir = std::path::PathBuf::from(appdata).join("Mozilla/Firefox/Profiles");
        if profiles_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&profiles_dir) {
                for entry in entries.flatten() {
                    let backup_dir = entry.path().join("bookmarkbackups");
                    if backup_dir.exists() {
                        if let Some(latest) = find_latest_json_backup(&backup_dir) {
                            if let Ok(content) = std::fs::read_to_string(&latest) {
                                parse_firefox_bookmarks_json(&content, &mut all);
                            }
                        }
                    }
                }
            }
        }
    }

    all
}

fn parse_chromium_bookmarks(json_str: &str, out: &mut Vec<Bookmark>) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) else {
        return;
    };

    let Some(roots) = value.get("roots") else {
        return;
    };

    for (_key, folder) in roots.as_object().into_iter().flatten() {
        extract_bookmarks_recursive(folder, out);
    }
}

fn extract_bookmarks_recursive(node: &serde_json::Value, out: &mut Vec<Bookmark>) {
    let Some(node_type) = node.get("type").and_then(|v| v.as_str()) else {
        return;
    };

    match node_type {
        "url" => {
            let name = node.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let url = node.get("url").and_then(|v| v.as_str()).unwrap_or("");
            if !url.is_empty() {
                out.push(Bookmark {
                    name: name.to_string(),
                    url: url.to_string(),
                });
            }
        }
        "folder" => {
            if let Some(children) = node.get("children").and_then(|v| v.as_array()) {
                for child in children {
                    extract_bookmarks_recursive(child, out);
                }
            }
        }
        _ => {}
    }
}

fn find_latest_json_backup(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut json_files: Vec<std::path::PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
        .collect();

    json_files.sort();
    json_files.pop()
}

fn parse_firefox_bookmarks_json(json_str: &str, out: &mut Vec<Bookmark>) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) else {
        return;
    };
    extract_firefox_bookmarks_recursive(&value, out);
}

fn extract_firefox_bookmarks_recursive(node: &serde_json::Value, out: &mut Vec<Bookmark>) {
    let node_type = node.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match node_type {
        "text/x-moz-place" => {
            let title = node.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let uri = node.get("uri").and_then(|v| v.as_str()).unwrap_or("");
            if !uri.is_empty() && (uri.starts_with("http://") || uri.starts_with("https://")) {
                out.push(Bookmark {
                    name: if title.is_empty() { uri.to_string() } else { title.to_string() },
                    url: uri.to_string(),
                });
            }
        }
        "text/x-moz-place-container" => {
            if let Some(children) = node.get("children").and_then(|v| v.as_array()) {
                for child in children {
                    extract_firefox_bookmarks_recursive(child, out);
                }
            }
        }
        _ => {
            if let Some(children) = node.get("children").and_then(|v| v.as_array()) {
                for child in children {
                    extract_firefox_bookmarks_recursive(child, out);
                }
            }
        }
    }
}
