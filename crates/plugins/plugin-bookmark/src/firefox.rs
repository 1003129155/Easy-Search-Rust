// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Firefox bookmark loading from bookmark backup JSON files.
//! Scans all Firefox profiles.

use crate::Bookmark;
use std::path::PathBuf;

/// Load bookmarks from all Firefox profiles.
pub fn load_firefox_bookmarks() -> Vec<Bookmark> {
    let Some(appdata) = std::env::var_os("APPDATA") else {
        return Vec::new();
    };

    let profiles_dir = PathBuf::from(appdata).join("Mozilla/Firefox/Profiles");
    if !profiles_dir.exists() {
        return Vec::new();
    }

    let mut all = Vec::new();

    let Ok(entries) = std::fs::read_dir(&profiles_dir) else {
        return Vec::new();
    };

    for entry in entries.flatten() {
        let profile_path = entry.path();
        if !profile_path.is_dir() {
            continue;
        }

        let profile_name = profile_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let source = format!("Firefox ({})", profile_name);

        // Try bookmarkbackups directory
        let backup_dir = profile_path.join("bookmarkbackups");
        if backup_dir.exists() {
            if let Some(latest) = find_latest_json_backup(&backup_dir) {
                if let Ok(content) = std::fs::read_to_string(&latest) {
                    parse_firefox_bookmarks(&content, &source, &mut all);
                }
            }
        }
    }

    all
}

/// Find the latest .json backup file in a directory.
fn find_latest_json_backup(dir: &std::path::Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut json_files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
        .collect();

    json_files.sort();
    json_files.pop() // latest by filename (date-sorted naming)
}

/// Parse Firefox bookmark backup JSON.
fn parse_firefox_bookmarks(json_str: &str, source: &str, out: &mut Vec<Bookmark>) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) else {
        return;
    };
    extract_recursive(&value, source, out);
}

fn extract_recursive(node: &serde_json::Value, source: &str, out: &mut Vec<Bookmark>) {
    let node_type = node.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match node_type {
        "text/x-moz-place" => {
            let title = node.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let uri = node.get("uri").and_then(|v| v.as_str()).unwrap_or("");
            if !uri.is_empty() && (uri.starts_with("http://") || uri.starts_with("https://")) {
                out.push(Bookmark {
                    name: if title.is_empty() {
                        uri.to_string()
                    } else {
                        title.to_string()
                    },
                    url: uri.to_string(),
                    source: source.to_string(),
                    favicon_path: None,
                });
            }
        }
        "text/x-moz-place-container" | _ => {
            if let Some(children) = node.get("children").and_then(|v| v.as_array()) {
                for child in children {
                    extract_recursive(child, source, out);
                }
            }
        }
    }
}
