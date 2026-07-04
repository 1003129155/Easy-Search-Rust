// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Windows Settings plugin — data-driven, 524 settings entries.
//!
//! Loads settings from embedded JSON (from Flow.Launcher's data).
//! Features:
//! - 524 settings entries covering all Windows Settings areas
//! - Alternative names (AltNames) for discovery
//! - Area-based categorization
//! - Fuzzy matching on Name, AltNames, Area, and Command
//! - OS build filtering (hides unsupported settings)

use easysearch_core::{Action, Plugin, PluginResult, SettingControl, SettingItem};
use serde::Deserialize;

/// Embedded JSON data (524 entries from Flow.Launcher).
const SETTINGS_JSON: &str = include_str!("data.json");

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
}

impl WinSettingsPlugin {
    #[must_use]
    pub fn new() -> Self {
        let entries = load_entries();
        Self {
            entries,
            max_results: 10,
        }
    }

    /// Convert a PascalCase name to a human-readable display name.
    /// "AccessWorkOrSchool" → "Access Work Or School"
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
            // Show first N entries when no query
            return self
                .entries
                .iter()
                .take(self.max_results)
                .enumerate()
                .map(|(i, e)| entry_to_result(e, 750 - i as u32))
                .collect();
        }

        // Fuzzy match on name, alt_names, area, command
        let mut scored: Vec<(&SettingsEntry, u32)> = self
            .entries
            .iter()
            .filter_map(|e| {
                let name_lower = e.name.to_lowercase();
                let display = Self::display_name(&e.name).to_lowercase();
                let area_lower = e.area.to_lowercase();

                // Score calculation
                let mut score: u32 = 0;

                // Exact prefix match on display name → highest
                if display.starts_with(&q) {
                    score = 900;
                } else if name_lower.contains(&q) {
                    score = 800;
                } else if display.contains(&q) {
                    score = 750;
                } else if area_lower.contains(&q) {
                    score = 600;
                } else if e.command.to_lowercase().contains(&q) {
                    score = 500;
                } else {
                    // Check alt names
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
            .map(|(e, score)| entry_to_result(e, score))
            .collect()
    }

    fn name(&self) -> &str {
        "WindowsSettings"
    }

    fn description(&self) -> &str {
        "快速打开 Windows 设置页面（524 项）"
    }

    fn icon(&self) -> &str {
        "settings"
    }

    fn settings_schema(&self) -> Option<Vec<SettingItem>> {
        Some(vec![SettingItem {
            key: "max_results".to_string(),
            label: "最大显示数量".to_string(),
            description: "搜索结果最多显示多少条".to_string(),
            control: SettingControl::Number {
                min: 5,
                max: 20,
                default: 10,
            },
        }])
    }

    fn on_setting_changed(&mut self, key: &str, value: &str) {
        if key == "max_results" {
            if let Ok(v) = value.parse::<usize>() {
                self.max_results = v.clamp(5, 20);
            }
        }
    }

    fn setting_values(&self) -> Vec<(String, String)> {
        vec![("max_results".to_string(), self.max_results.to_string())]
    }
}

fn entry_to_result(e: &SettingsEntry, score: u32) -> PluginResult {
    let display_name = WinSettingsPlugin::display_name(&e.name);
    let subtitle = if e.area.is_empty() {
        e.command.clone()
    } else {
        format!("{} — {}", e.area, e.command)
    };

    PluginResult {
        title: display_name,
        subtitle,
        icon: String::from("settings"),
        action: Action::Open(e.command.clone()),
        score,
    }
}

/// Load and filter settings entries from embedded JSON.
fn load_entries() -> Vec<SettingsEntry> {
    let entries: Vec<SettingsEntry> = serde_json::from_str(SETTINGS_JSON).unwrap_or_default();

    // Filter by current OS build (optional — skip entries that don't apply)
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
            // Must have a command
            !e.command.is_empty()
        })
        .collect()
}

/// Get the current Windows build number.
fn get_windows_build() -> u32 {
    #[cfg(windows)]
    {
        // Read from registry: HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\CurrentBuildNumber
        use std::os::windows::process::CommandExt;
        use std::process::Command;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;

        let mut command = Command::new("cmd");
        command.args(["/c", "ver"]);
        command.creation_flags(CREATE_NO_WINDOW);

        let output = command.output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .unwrap_or_default();

        // Parse "Microsoft Windows [Version 10.0.22631.4890]" → 22631
        if let Some(start) = output.find("10.0.") {
            let rest = &output[start + 5..];
            if let Some(end) = rest.find(|c: char| !c.is_ascii_digit()) {
                if let Ok(build) = rest[..end].parse::<u32>() {
                    return build;
                }
            }
        }
        99999 // default: assume latest
    }
    #[cfg(not(windows))]
    {
        99999
    }
}
