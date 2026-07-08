// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Chromium-based browser bookmark loading.

use crate::Bookmark;
use rusqlite::{Connection, OpenFlags, params};
use std::collections::HashMap;
use std::path::PathBuf;

pub fn load_chromium_bookmarks(
    browser_subpath: &str,
    browser_name: &str,
    load_favicons: bool,
    favicon_cache_dir: &std::path::Path,
) -> Vec<Bookmark> {
    let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") else {
        return Vec::new();
    };

    let user_data_dir = PathBuf::from(local_app_data)
        .join(browser_subpath)
        .join("User Data");
    load_chromium_bookmarks_from_dir(
        &user_data_dir,
        browser_name,
        load_favicons,
        favicon_cache_dir,
    )
}

pub fn load_chromium_bookmarks_from_dir(
    user_data_dir: &std::path::Path,
    browser_name: &str,
    load_favicons: bool,
    favicon_cache_dir: &std::path::Path,
) -> Vec<Bookmark> {
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
        if !bookmarks_file.is_file() {
            continue;
        }

        let dir_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let source = if dir_name == "Default" {
            browser_name.to_string()
        } else {
            format!("{} ({})", browser_name, dir_name)
        };

        if let Ok(content) = std::fs::read_to_string(&bookmarks_file) {
            let start = all.len();
            parse_chromium_bookmarks(&content, &source, &mut all);

            if load_favicons {
                let favicon_db = path.join("Favicons");
                if favicon_db.is_file() {
                    load_profile_favicons(&favicon_db, &mut all[start..], favicon_cache_dir);
                }
            }
        }
    }

    all
}

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
                    favicon_path: None,
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

fn load_profile_favicons(
    favicon_db_path: &std::path::Path,
    bookmarks: &mut [Bookmark],
    favicon_cache_dir: &std::path::Path,
) {
    if bookmarks.is_empty() {
        return;
    }

    let _ = std::fs::create_dir_all(favicon_cache_dir);

    let temp_db_path = favicon_cache_dir.join(format!(
        "tempfavicons_{}_{}.db",
        std::process::id(),
        unique_stamp()
    ));
    if std::fs::copy(favicon_db_path, &temp_db_path).is_err() {
        return;
    }

    let _ = load_profile_favicons_from_copy(&temp_db_path, bookmarks, favicon_cache_dir);
    let _ = std::fs::remove_file(&temp_db_path);
}

fn load_profile_favicons_from_copy(
    temp_db_path: &std::path::Path,
    bookmarks: &mut [Bookmark],
    favicon_cache_dir: &std::path::Path,
) -> rusqlite::Result<()> {
    let connection = Connection::open_with_flags(temp_db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut statement = connection.prepare(
        "
        SELECT f.id, b.image_data
        FROM favicons f
        JOIN favicon_bitmaps b ON f.id = b.icon_id
        JOIN icon_mapping m ON f.id = m.icon_id
        WHERE m.page_url LIKE ?1
        ORDER BY b.width DESC
        LIMIT 1
        ",
    )?;
    let mut cached_paths: HashMap<String, String> = HashMap::new();

    for bookmark in bookmarks {
        let Some(domain) = extract_domain(&bookmark.url) else {
            continue;
        };

        if let Some(existing) = cached_paths.get(domain) {
            bookmark.favicon_path = Some(existing.clone());
            continue;
        }

        let mut rows = statement.query(params![format!("%{domain}%")])?;
        let Some(row) = rows.next()? else {
            continue;
        };

        let icon_id: i64 = row.get(0)?;
        let image_data: Vec<u8> = row.get(1)?;
        if image_data.is_empty() {
            continue;
        }

        let extension = detect_image_extension(&image_data).unwrap_or("png");
        let output_path = favicon_cache_dir.join(format!(
            "chromium_{}_{}.{}",
            sanitize_for_filename(domain),
            icon_id,
            extension
        ));
        if !output_path.is_file() && std::fs::write(&output_path, &image_data).is_err() {
            continue;
        }

        let output = output_path.to_string_lossy().to_string();
        cached_paths.insert(domain.to_string(), output.clone());
        bookmark.favicon_path = Some(output);
    }

    Ok(())
}

fn extract_domain(url: &str) -> Option<&str> {
    let scheme = url.find("://")?;
    let rest = &url[scheme + 3..];
    let host = rest.split(['/', '?', '#']).next()?.trim();
    if host.is_empty() {
        return None;
    }
    Some(
        host.rsplit('@')
            .next()
            .unwrap_or(host)
            .split(':')
            .next()
            .unwrap_or(host),
    )
}

fn sanitize_for_filename(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn detect_image_extension(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        Some("gif")
    } else if data.starts_with(b"BM") {
        Some("bmp")
    } else if data.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        Some("ico")
    } else if data.len() > 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some("webp")
    } else if data.starts_with(b"<svg") || data.starts_with(b"<?xml") {
        Some("svg")
    } else {
        None
    }
}

fn unique_stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}
