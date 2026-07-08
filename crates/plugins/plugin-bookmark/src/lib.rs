// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Browser bookmark search plugin.

mod chromium;
mod firefox;

use easysearch_core::{Action, Plugin, PluginResult, SettingControl, SettingItem};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub name: String,
    pub url: String,
    pub source: String,
    #[serde(default)]
    pub favicon_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkSettings {
    pub enable_chrome: bool,
    pub enable_edge: bool,
    pub enable_brave: bool,
    pub enable_firefox: bool,
    pub enable_favicons: bool,
    pub max_results: u32,
}

impl Default for BookmarkSettings {
    fn default() -> Self {
        Self {
            enable_chrome: true,
            enable_edge: true,
            enable_brave: true,
            enable_firefox: true,
            enable_favicons: true,
            max_results: 8,
        }
    }
}

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

    fn display_name(&self, locale: &str) -> String {
        match locale_prefix(locale) {
            "zh" => "涔︾",
            "ja" => "銉栥儍銈優銉笺偗",
            _ => "Bookmark",
        }
        .to_string()
    }

    fn description(&self) -> &str {
        "Search browser bookmarks across Chrome, Edge, Brave, and Firefox"
    }

    fn description_for_locale(&self, locale: &str) -> String {
        match locale_prefix(locale) {
            "zh" => "鎼滅储 Chrome銆丒dge銆丅rave 鍜?Firefox 鐨勬祻瑙堝櫒涔︾".to_string(),
            "ja" => {
                "Chrome銆丒dge銆丅rave銆丗irefox 銇儢銉┿偊銈躲兗銉栥儍銈優銉笺偗銈掓绱仐銇俱仚"
                    .to_string()
            }
            _ => self.description().to_string(),
        }
    }

    fn icon(&self) -> &str {
        "bookmark"
    }

    fn settings_schema(&self) -> Option<Vec<SettingItem>> {
        Some(settings_items())
    }

    fn settings_schema_for_locale(&self, _locale: &str) -> Option<Vec<SettingItem>> {
        Some(settings_items())
    }

    fn on_setting_changed(&mut self, key: &str, value: &str) {
        match key {
            "enable_chrome" => assign_bool(&mut self.settings.enable_chrome, value),
            "enable_edge" => assign_bool(&mut self.settings.enable_edge, value),
            "enable_brave" => assign_bool(&mut self.settings.enable_brave, value),
            "enable_firefox" => assign_bool(&mut self.settings.enable_firefox, value),
            "enable_favicons" => assign_bool(&mut self.settings.enable_favicons, value),
            "max_results" => {
                if let Ok(v) = serde_json::from_str(value) {
                    self.settings.max_results = v;
                }
            }
            _ => {}
        }

        let new_bookmarks = load_all_bookmarks(&self.settings);
        if let Ok(mut lock) = self.bookmarks.lock() {
            *lock = new_bookmarks;
        }
    }

    fn setting_values(&self) -> Vec<(String, String)> {
        vec![
            json_setting("enable_chrome", self.settings.enable_chrome),
            json_setting("enable_edge", self.settings.enable_edge),
            json_setting("enable_brave", self.settings.enable_brave),
            json_setting("enable_firefox", self.settings.enable_firefox),
            json_setting("enable_favicons", self.settings.enable_favicons),
            (
                "max_results".to_string(),
                serde_json::to_string(&self.settings.max_results).unwrap_or_default(),
            ),
        ]
    }
}

fn bookmark_to_result(b: &Bookmark, score: u32) -> PluginResult {
    PluginResult {
        title: b.name.clone(),
        subtitle: format!("{} - {}", b.source, b.url),
        icon: b
            .favicon_path
            .clone()
            .unwrap_or_else(|| String::from("bookmark")),
        action: Action::Open(b.url.clone()),
        score,
        highlight: Vec::new(),
        context_actions: Vec::new(),
        context_data: None,
    }
}

fn load_all_bookmarks(settings: &BookmarkSettings) -> Vec<Bookmark> {
    let mut all = Vec::new();
    let favicon_cache_dir = favicon_cache_dir();

    if settings.enable_chrome {
        all.extend(chromium::load_chromium_bookmarks(
            "Google/Chrome",
            "Chrome",
            settings.enable_favicons,
            &favicon_cache_dir,
        ));
        all.extend(chromium::load_chromium_bookmarks(
            "Google/Chrome SxS",
            "Chrome Canary",
            settings.enable_favicons,
            &favicon_cache_dir,
        ));
        all.extend(chromium::load_chromium_bookmarks(
            "Chromium",
            "Chromium",
            settings.enable_favicons,
            &favicon_cache_dir,
        ));
    }
    if settings.enable_edge {
        all.extend(chromium::load_chromium_bookmarks(
            "Microsoft/Edge",
            "Edge",
            settings.enable_favicons,
            &favicon_cache_dir,
        ));
        all.extend(chromium::load_chromium_bookmarks(
            "Microsoft/Edge Dev",
            "Edge Dev",
            settings.enable_favicons,
            &favicon_cache_dir,
        ));
        all.extend(chromium::load_chromium_bookmarks(
            "Microsoft/Edge SxS",
            "Edge Canary",
            settings.enable_favicons,
            &favicon_cache_dir,
        ));
    }
    if settings.enable_brave {
        all.extend(chromium::load_chromium_bookmarks(
            "BraveSoftware/Brave-Browser",
            "Brave",
            settings.enable_favicons,
            &favicon_cache_dir,
        ));
    }
    if settings.enable_firefox {
        all.extend(firefox::load_firefox_bookmarks());
    }

    all
}

fn toggle_setting(key: &str, label: &str, description: &str, default: bool) -> SettingItem {
    SettingItem {
        key: key.to_string(),
        label: label.to_string(),
        description: description.to_string(),
        control: SettingControl::Toggle { default },
    }
}

fn assign_bool(slot: &mut bool, value: &str) {
    if let Ok(v) = serde_json::from_str(value) {
        *slot = v;
    }
}

fn json_setting(key: &str, value: bool) -> (String, String) {
    (
        key.to_string(),
        serde_json::to_string(&value).unwrap_or_default(),
    )
}

fn favicon_cache_dir() -> PathBuf {
    easysearch_core::paths::plugin_cache_dir("bookmark").join("favicons")
}

fn settings_items() -> Vec<SettingItem> {
    vec![
        toggle_setting(
            "enable_chrome",
            "Enable Chrome",
            "Load Google Chrome bookmarks",
            true,
        ),
        toggle_setting(
            "enable_edge",
            "Enable Edge",
            "Load Microsoft Edge bookmarks",
            true,
        ),
        toggle_setting(
            "enable_brave",
            "Enable Brave",
            "Load Brave browser bookmarks",
            true,
        ),
        toggle_setting(
            "enable_firefox",
            "Enable Firefox",
            "Load Firefox bookmarks",
            true,
        ),
        toggle_setting(
            "enable_favicons",
            "Enable favicons",
            "Try loading per-site icons from the browser favicon database",
            true,
        ),
        SettingItem {
            key: "max_results".to_string(),
            label: "Maximum results".to_string(),
            description: "How many bookmarks to show at most in search results".to_string(),
            control: SettingControl::Number {
                min: 1,
                max: 20,
                default: 8,
            },
        },
    ]
}

fn locale_prefix(locale: &str) -> &str {
    locale.split('-').next().unwrap_or(locale)
}
