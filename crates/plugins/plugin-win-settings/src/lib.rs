// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Windows Settings plugin.
//!
//! Loads settings from embedded JSON (ported from Flow.Launcher data).

use easysearch_core::{Action, Plugin, PluginResult, SettingControl, SettingItem, context_labels};
use serde::Deserialize;
use std::collections::HashMap;

/// Embedded JSON data.
const SETTINGS_JSON: &str = include_str!("data.json");

/// Embedded translation data.
const TRANSLATIONS_ZH: &str = include_str!("translations_zh.json");
const TRANSLATIONS_JA: &str = include_str!("translations_ja.json");

/// A single Windows Setting entry from the JSON data.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SettingsEntry {
    /// Setting name (e.g. "AccessWorkOrSchool").
    name: String,
    /// Category area (e.g. "Accounts", "System", "Network").
    #[serde(default)]
    area: String,
    /// Type: "AppSettingsApp" or "ControlPanelApp".
    #[serde(default, rename = "Type")]
    #[allow(dead_code)]
    entry_type: String,
    /// Alternative names for search matching.
    #[serde(default)]
    alt_names: Vec<String>,
    /// ms-settings: URI or control panel command.
    #[serde(default)]
    command: String,
    /// Minimum required Windows build number (if specified).
    #[serde(default)]
    min_build: Option<u32>,
    /// Maximum supported Windows build number (if specified).
    #[serde(default)]
    max_build: Option<u32>,
}

/// Windows Settings plugin.
pub struct WinSettingsPlugin {
    entries: Vec<SettingsEntry>,
    /// Maximum number of results to show.
    max_results: usize,
    /// Translation maps: key → translated display name.
    translations_zh: HashMap<String, String>,
    translations_ja: HashMap<String, String>,
}

impl WinSettingsPlugin {
    #[must_use]
    pub fn new() -> Self {
        let entries = load_entries();
        let translations_zh: HashMap<String, String> =
            serde_json::from_str(TRANSLATIONS_ZH).unwrap_or_default();
        let translations_ja: HashMap<String, String> =
            serde_json::from_str(TRANSLATIONS_JA).unwrap_or_default();
        Self {
            entries,
            max_results: 10,
            translations_zh,
            translations_ja,
        }
    }

    /// Convert a PascalCase name to a human-readable display name.
    fn display_name(name: &str) -> String {
        let mut result = String::with_capacity(name.len() + 8);
        for (i, ch) in name.chars().enumerate() {
            if i > 0 && ch.is_uppercase() {
                result.push(' ');
            }
            result.push(ch);
        }
        result
    }

    /// Get the translated name for an entry based on current locale.
    fn translated_name(&self, entry: &SettingsEntry) -> String {
        let locale = context_labels::get_locale_prefix();
        match locale.as_str() {
            "zh" => self
                .translations_zh
                .get(&entry.name)
                .cloned()
                .unwrap_or_else(|| Self::display_name(&entry.name)),
            "ja" => self
                .translations_ja
                .get(&entry.name)
                .cloned()
                .unwrap_or_else(|| Self::display_name(&entry.name)),
            _ => Self::display_name(&entry.name),
        }
    }

    /// Get the translated area for an entry based on current locale.
    fn translated_area(&self, entry: &SettingsEntry) -> String {
        if entry.area.is_empty() {
            return String::new();
        }
        let locale = context_labels::get_locale_prefix();
        let area_key = format!("Area{}", entry.area);
        match locale.as_str() {
            "zh" => self
                .translations_zh
                .get(&area_key)
                .cloned()
                .unwrap_or_else(|| entry.area.clone()),
            "ja" => self
                .translations_ja
                .get(&area_key)
                .cloned()
                .unwrap_or_else(|| entry.area.clone()),
            _ => entry.area.clone(),
        }
    }
}

impl Default for WinSettingsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for WinSettingsPlugin {
    fn default_keyword(&self) -> Option<&str> {
        Some("s")
    }

    fn query(&self, query: &str) -> Vec<PluginResult> {
        let q = query.trim().to_lowercase();

        if q.is_empty() {
            return self
                .entries
                .iter()
                .take(self.max_results)
                .enumerate()
                .map(|(i, e)| self.entry_to_result(e, 750 - i as u32))
                .collect();
        }

        let mut scored: Vec<(&SettingsEntry, u32)> = self
            .entries
            .iter()
            .filter_map(|e| {
                let name_lower = e.name.to_lowercase();
                let display = Self::display_name(&e.name).to_lowercase();
                let translated = self.translated_name(e).to_lowercase();
                let area_lower = e.area.to_lowercase();
                let translated_area = self.translated_area(e).to_lowercase();

                let mut score: u32 = 0;

                if translated.starts_with(&q) || display.starts_with(&q) {
                    score = 900;
                } else if name_lower.contains(&q) || translated.contains(&q) {
                    score = 800;
                } else if display.contains(&q) {
                    score = 750;
                } else if area_lower.contains(&q) || translated_area.contains(&q) {
                    score = 600;
                } else if e.command.to_lowercase().contains(&q) {
                    score = 500;
                } else {
                    for alt in &e.alt_names {
                        if alt.to_lowercase().contains(&q) {
                            score = 700;
                            break;
                        }
                    }
                }

                if score > 0 {
                    Some((e, score))
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| b.1.cmp(&a.1));
        scored.truncate(self.max_results);

        scored
            .into_iter()
            .map(|(e, score)| self.entry_to_result(e, score))
            .collect()
    }

    fn name(&self) -> &str {
        "WindowsSettings"
    }

    fn display_name(&self, locale: &str) -> String {
        match locale_prefix(locale) {
            "zh" => "Windows 设置",
            "ja" => "Windows 設定",
            _ => "Windows Settings",
        }
        .to_string()
    }

    fn description(&self) -> &str {
        "Open Windows settings pages and control panel entries"
    }

    fn description_for_locale(&self, locale: &str) -> String {
        match locale_prefix(locale) {
            "zh" => "快速打开 Windows 设置页和控制面板项目".to_string(),
            "ja" => "Windows の設定ページやコントロールパネル項目をすばやく開きます".to_string(),
            _ => self.description().to_string(),
        }
    }

    fn icon(&self) -> &str {
        "settings"
    }

    fn settings_schema(&self) -> Option<Vec<SettingItem>> {
        self.settings_schema_for_locale("en")
    }

    fn settings_schema_for_locale(&self, locale: &str) -> Option<Vec<SettingItem>> {
        let (label, description) = match locale_prefix(locale) {
            "zh" => ("最大显示数量", "搜索结果最多显示多少条"),
            "ja" => ("最大表示件数", "検索結果に表示する件数の上限です"),
            _ => ("Maximum results", "How many Windows settings items to show at most"),
        };

        Some(vec![SettingItem {
            key: "max_results".to_string(),
            label: label.to_string(),
            description: description.to_string(),
            control: SettingControl::Number {
                min: 5,
                max: 20,
                default: 10,
            },
        }])
    }

    fn on_setting_changed(&mut self, key: &str, value: &str) {
        if key == "max_results" {
            let raw = value.trim_matches('"');
            if let Ok(v) = raw.parse::<usize>() {
                self.max_results = v.clamp(5, 20);
            }
        }
    }

    fn setting_values(&self) -> Vec<(String, String)> {
        vec![("max_results".to_string(), self.max_results.to_string())]
    }
}

impl WinSettingsPlugin {
    fn entry_to_result(&self, e: &SettingsEntry, score: u32) -> PluginResult {
        let display_name = self.translated_name(e);
        let area = self.translated_area(e);
        let subtitle = if area.is_empty() {
            e.command.clone()
        } else {
            format!("{} - {}", area, e.command)
        };

        PluginResult {
            title: display_name,
            subtitle,
            icon: String::from("settings"),
            action: Action::Open(e.command.clone()),
            score,
            highlight: Vec::new(),
            context_actions: Vec::new(),
            context_data: None,
        }
    }
}

/// Load and filter settings entries from embedded JSON.
fn load_entries() -> Vec<SettingsEntry> {
    let entries: Vec<SettingsEntry> = serde_json::from_str(SETTINGS_JSON).unwrap_or_default();
    let current_build = get_windows_build();

    entries
        .into_iter()
        .filter(|e| {
            if let Some(min) = e.min_build {
                if current_build < min {
                    return false;
                }
            }
            if let Some(max) = e.max_build {
                if current_build > max {
                    return false;
                }
            }
            !e.command.is_empty()
        })
        .collect()
}

/// Get the current Windows build number.
fn get_windows_build() -> u32 {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use std::process::Command;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        let mut command = Command::new("cmd");
        command.args(["/c", "ver"]);
        command.creation_flags(CREATE_NO_WINDOW);

        let output = command
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default();

        if let Some(start) = output.find("10.0.") {
            let rest = &output[start + 5..];
            if let Some(end) = rest.find(|c: char| !c.is_ascii_digit()) {
                if let Ok(build) = rest[..end].parse::<u32>() {
                    return build;
                }
            }
        }
        99999
    }
    #[cfg(not(windows))]
    {
        99999
    }
}

fn locale_prefix(locale: &str) -> &str {
    locale.split('-').next().unwrap_or(locale)
}
