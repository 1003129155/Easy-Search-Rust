// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Chromium-based browser bookmark loading.
//! Supports multi-profile (scans all profiles, not just Default).

use crate::Bookmark;
use std::path::PathBuf;

/// Load bookmarks from a Chromium-based browser.
/// `browser_subpath` is relative to LOCALAPPDATA, e.g. "Google/Chrome/User Data".
/// `browser_name` is the display name for the source field.
pub fn load_chromium_bookmarks(browser_subpath: &str, browser_name: &str) -> Vec<Bookmark> {
    let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") else {
        return Vec::new();
    };

    let user_data_dir = PathBuf::from(local_app_data).join(browser_subpath).join("User Data");
    load_chromium_bookmarks_from_dir(&user_data_dir, browser_name)
}

/// Load bookmarks from a specific User Data directory.
/// Scans all subdirectories for a `Bookmarks` file (like FlowLauncher).
pub fn load_chromium_bookmarks_from_dir(user_data_dir: &std::path::Path, browser_name: &str) -> Vec<Bookmark> {
    if !user_data_dir.exists() {
        return Vec::new();
    }

    let mut all = Vec::new();

    let Ok(entries) = std::fs::read_dir(user_data_dir) else {
        return Vec::new();
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let bookmarks_file = path.join("Bookmarks");
        if bookmarks_file.exists() {
            let dir_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let source = if dir_name == "Default" {
                browser_name.to_string()
            } else {
                format!("{} ({})", browser_name, dir_name)
            };

            if let Ok(content) = std::fs::read_to_string(&bookmarks_file) {
                parse_chromium_bookmarks(&content, &source, &mut all);
            }
        }
    }

    all
}

/// Parse a Chromium Bookmarks JSON file.
fn parse_chromium_bookmarks(json_str: &str, source: &str, out: &mut Vec<Bookmark>) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json_str) else {
        return;
    };

    let Some(roots) = value.get("roots") else {
        return;
    };

    for (_key, folder) in roots.as_object().into_iter().flatten() {
        extract_recursive(folder, source, out);
    }
}

fn extract_recursive(node: &serde_json::Value, source: &str, out: &mut Vec<Bookmark>) {
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
                    source: source.to_string(),
                });
            }
        }
        "folder" => {
            if let Some(children) = node.get("children").and_then(|v| v.as_array()) {
                for child in children {
                    extract_recursive(child, source, out);
                }
            }
        }
        _ => {}
    }
}
