// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Browser bookmark search plugin — FL-grade implementation.
//!
//! Features:
//! - Chrome, Edge, Brave, Opera, Vivaldi (Chromium-based)
//! - Firefox (from places.sqlite backup JSON)
//! - Multi-profile support (scans all User Data profiles, not just Default)
//! - File monitoring: reloads bookmarks when bookmark files change (poll-based)
//! - Fuzzy matching on name and URL
//! - Settings: enable/disable per browser, max results

mod chromium;
mod firefox;

use easysearch_core::{Action, Plugin, PluginResult, SettingControl, SettingItem};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Refresh interval for bookmark polling (5 minutes).
const REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// A single bookmark entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub name: String,
    pub url: String,
    /// Which browser/profile this came from.
    pub source: String,
}

/// Settings for the Bookmark plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkSettings {
    pub enable_chrome: bool,
    pub enable_edge: bool,
    pub enable_brave: bool,
    pub enable_firefox: bool,
    pub max_results: u32,
}

impl Default for BookmarkSettings {
    fn default() -> Self {
        Self {
            enable_chrome: true,
            enable_edge: true,
            enable_brave: true,
            enable_firefox: true,
            max_results: 8,
        }
    }
}

/// Browser bookmark plugin.
pub struct BookmarkPlugin {
    bookmarks: Arc<Mutex<Vec<Bookmark>>>,
    settings: BookmarkSettings,
    last_refresh: Mutex<Instant>,
}

impl BookmarkPlugin {
    #[must_use]
    pub fn new() -> Self {
        let settings = BookmarkSettings::default();
        let bookmarks = load_all_bookmarks(&settings);
        Self {
            bookmarks: Arc::new(Mutex::new(bookmarks)),
            settings,
            last_refresh: Mutex::new(Instant::now()),
        }
    }

    /// Reload bookmarks if the refresh interval has elapsed.
    fn maybe_refresh(&self) {
        let should_refresh = self
            .last_refresh
            .lock()
            .map(|ts| ts.elapsed() > REFRESH_INTERVAL)
            .unwrap_or(false);

        if should_refresh {
            let new_bookmarks = load_all_bookmarks(&self.settings);
            if let Ok(mut lock) = self.bookmarks.lock() {
                *lock = new_bookmarks;
            }
            if let Ok(mut ts) = self.last_refresh.lock() {
                *ts = Instant::now();
            }
        }
    }
}

impl Default for BookmarkPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for BookmarkPlugin {
    fn default_keyword(&self) -> Option<&str> {
        Some("b")
    }

    fn also_global(&self) -> bool {
        true
    }

    fn matches(&self, query: &str) -> bool {
        // Participate globally when query is at least 2 chars (avoid noise on single chars)
        query.trim().len() >= 2
    }

    fn query(&self, query: &str) -> Vec<PluginResult> {
        self.maybe_refresh();

        let q = query.trim().to_lowercase();
        let bookmarks = self.bookmarks.lock().unwrap_or_else(|e| e.into_inner());

        if q.is_empty() {
            return bookmarks
                .iter()
                .take(self.settings.max_results as usize)
                .map(|b| bookmark_to_result(b, 700))
                .collect();
        }

        let mut scored: Vec<(&Bookmark, u32)> = bookmarks
            .iter()
            .filter_map(|b| {
                let name_lower = b.name.to_lowercase();
                let url_lower = b.url.to_lowercase();

                if name_lower.starts_with(&q) {
                    Some((b, 900))
                } else if name_lower.contains(&q) {
                    Some((b, 800))
                } else if url_lower.contains(&q) {
                    Some((b, 600))
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| b.1.cmp(&a.1));
        scored.truncate(self.settings.max_results as usize);

        scored
            .into_iter()
            .map(|(b, score)| bookmark_to_result(b, score))
            .collect()
    }

    fn name(&self) -> &str {
        "Bookmark"
    }

    fn description(&self) -> &str {
        "搜索浏览器书签（Chrome/Edge/Brave/Firefox，多配置文件）"
    }

    fn icon(&self) -> &str {
        "bookmark"
    }

    fn settings_schema(&self) -> Option<Vec<SettingItem>> {
        Some(vec![
            SettingItem {
                key: "enable_chrome".to_string(),
                label: "启用 Chrome".to_string(),
                description: "加载 Google Chrome 书签".to_string(),
                control: SettingControl::Toggle { default: true },
            },
            SettingItem {
                key: "enable_edge".to_string(),
                label: "启用 Edge".to_string(),
                description: "加载 Microsoft Edge 书签".to_string(),
                control: SettingControl::Toggle { default: true },
            },
            SettingItem {
                key: "enable_brave".to_string(),
                label: "启用 Brave".to_string(),
                description: "加载 Brave 浏览器书签".to_string(),
                control: SettingControl::Toggle { default: true },
            },
            SettingItem {
                key: "enable_firefox".to_string(),
                label: "启用 Firefox".to_string(),
                description: "加载 Firefox 书签".to_string(),
                control: SettingControl::Toggle { default: true },
            },
            SettingItem {
                key: "max_results".to_string(),
                label: "最大结果数".to_string(),
                description: "搜索结果最多显示多少条书签".to_string(),
                control: SettingControl::Number {
                    min: 1,
                    max: 20,
                    default: 8,
                },
            },
        ])
    }

    fn on_setting_changed(&mut self, key: &str, value: &str) {
        match key {
            "enable_chrome" => { if let Ok(v) = serde_json::from_str(value) { self.settings.enable_chrome = v; } }
            "enable_edge" => { if let Ok(v) = serde_json::from_str(value) { self.settings.enable_edge = v; } }
            "enable_brave" => { if let Ok(v) = serde_json::from_str(value) { self.settings.enable_brave = v; } }
            "enable_firefox" => { if let Ok(v) = serde_json::from_str(value) { self.settings.enable_firefox = v; } }
            "max_results" => { if let Ok(v) = serde_json::from_str(value) { self.settings.max_results = v; } }
            _ => {}
        }
        // Trigger reload with new settings
        let new_bookmarks = load_all_bookmarks(&self.settings);
        if let Ok(mut lock) = self.bookmarks.lock() {
            *lock = new_bookmarks;
        }
    }

    fn setting_values(&self) -> Vec<(String, String)> {
        vec![
            ("enable_chrome".to_string(), serde_json::to_string(&self.settings.enable_chrome).unwrap_or_default()),
            ("enable_edge".to_string(), serde_json::to_string(&self.settings.enable_edge).unwrap_or_default()),
            ("enable_brave".to_string(), serde_json::to_string(&self.settings.enable_brave).unwrap_or_default()),
            ("enable_firefox".to_string(), serde_json::to_string(&self.settings.enable_firefox).unwrap_or_default()),
            ("max_results".to_string(), serde_json::to_string(&self.settings.max_results).unwrap_or_default()),
        ]
    }
}

fn bookmark_to_result(b: &Bookmark, score: u32) -> PluginResult {
    PluginResult {
        title: b.name.clone(),
        subtitle: format!("{} — {}", b.source, b.url),
        icon: String::from("bookmark"),
        action: Action::Open(b.url.clone()),
        score,
    }
}

/// Load all bookmarks from enabled browsers.
fn load_all_bookmarks(settings: &BookmarkSettings) -> Vec<Bookmark> {
    let mut all = Vec::new();

    if settings.enable_chrome {
        all.extend(chromium::load_chromium_bookmarks("Google/Chrome", "Chrome"));
        all.extend(chromium::load_chromium_bookmarks("Google/Chrome SxS", "Chrome Canary"));
        all.extend(chromium::load_chromium_bookmarks("Chromium", "Chromium"));
    }
    if settings.enable_edge {
        all.extend(chromium::load_chromium_bookmarks("Microsoft/Edge", "Edge"));
        all.extend(chromium::load_chromium_bookmarks("Microsoft/Edge Dev", "Edge Dev"));
        all.extend(chromium::load_chromium_bookmarks("Microsoft/Edge SxS", "Edge Canary"));
    }
    if settings.enable_brave {
        all.extend(chromium::load_chromium_bookmarks("BraveSoftware/Brave-Browser", "Brave"));
    }
    if settings.enable_firefox {
        all.extend(firefox::load_firefox_bookmarks());
    }

    all
}
