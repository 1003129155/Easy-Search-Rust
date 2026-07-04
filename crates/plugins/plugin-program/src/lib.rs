// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Program launcher plugin — FL-grade implementation.
//!
//! Features:
//! - Win32 programs: Scans Start Menu (.lnk, .exe)
//! - UWP/MSIX apps: Enumerates via PowerShell `Get-AppxPackage`
//! - JSON disk cache: Instant load on startup, background rebuild
//! - Fuzzy matching: prefix > contains > initials scoring
//! - Periodic refresh: rebuilds index every 30 minutes
//! - Settings: max_results, hide_uninstallers

mod cache;
mod fuzzy;
mod scanner;
mod settings;
mod uwp;

use easysearch_core::{Action, Plugin, PluginResult, SettingControl, SettingItem};
pub use settings::ProgramSettings;

use cache::ProgramCache;
use fuzzy::fuzzy_score;
use scanner::scan_start_menu;
use uwp::scan_uwp_apps;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A discovered program entry (Win32 or UWP).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProgramEntry {
    /// Display name.
    pub name: String,
    /// Executable / shortcut path (or UWP app ID).
    pub path: String,
    /// Source type for icon hints.
    pub source: ProgramSource,
}

/// Where this program entry came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ProgramSource {
    StartMenu,
    Uwp,
}

/// Program launcher plugin.
pub struct ProgramPlugin {
    /// Shared program list (can be updated from background refresh).
    programs: Arc<Mutex<Vec<ProgramEntry>>>,
    /// Settings.
    settings: ProgramSettings,
    /// Last time the index was fully rebuilt.
    last_rebuild: Mutex<Instant>,
}

impl ProgramPlugin {
    /// Create and initialize the program plugin.
    /// Loads from cache first, then does a full scan if cache is stale.
    #[must_use]
    pub fn new() -> Self {
        let settings = ProgramSettings::load();
        let (programs, needs_rebuild) = match ProgramCache::load() {
            Some(cached) if !cached.is_stale() => (cached.entries, false),
            Some(cached) => (cached.entries, true), // use stale cache but trigger rebuild
            None => (Vec::new(), true),
        };

        let programs = Arc::new(Mutex::new(programs));
        let last_rebuild = Mutex::new(Instant::now());

        let plugin = Self {
            programs,
            settings,
            last_rebuild,
        };

        if needs_rebuild {
            plugin.rebuild_index();
        }

        plugin
    }

    /// Full index rebuild: scan Win32 + UWP, apply filters, save cache.
    fn rebuild_index(&self) {
        let mut entries = scan_start_menu();
        entries.extend(scan_uwp_apps());

        // Filter uninstallers if configured
        if self.settings.hide_uninstallers {
            entries.retain(|e| !is_uninstaller(&e.name));
        }

        // Deduplicate by lowercase name
        entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        entries.dedup_by(|a, b| a.name.to_lowercase() == b.name.to_lowercase());

        // Save to cache
        ProgramCache::save(&entries);

        // Update in-memory list
        if let Ok(mut lock) = self.programs.lock() {
            *lock = entries;
        }

        // Update rebuild timestamp
        if let Ok(mut ts) = self.last_rebuild.lock() {
            *ts = Instant::now();
        }
    }

    /// Check if it's time for a periodic rebuild (every 30 minutes).
    fn maybe_refresh(&self) {
        let should_rebuild = self
            .last_rebuild
            .lock()
            .map(|ts| ts.elapsed() > Duration::from_secs(30 * 60))
            .unwrap_or(false);

        if should_rebuild {
            self.rebuild_index();
        }
    }
}

impl Default for ProgramPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ProgramPlugin {
    fn default_keyword(&self) -> Option<&str> {
        None // global match
    }

    fn matches(&self, _query: &str) -> bool {
        true
    }

    fn query(&self, query: &str) -> Vec<PluginResult> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }

        // Periodic refresh check
        self.maybe_refresh();

        let programs = self.programs.lock().unwrap_or_else(|e| e.into_inner());

        // Score and sort
        let mut scored: Vec<(u32, &ProgramEntry)> = programs
            .iter()
            .filter_map(|p| {
                let score = fuzzy_score(&q, &p.name.to_lowercase());
                if score > 0 { Some((score, p)) } else { None }
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.truncate(self.settings.max_results as usize);

        scored
            .into_iter()
            .enumerate()
            .map(|(i, (score, p))| {
                let icon = match p.source {
                    ProgramSource::StartMenu => p.path.clone(),
                    ProgramSource::Uwp => "uwp-app".to_string(),
                };
                PluginResult {
                    title: p.name.clone(),
                    subtitle: p.path.clone(),
                    icon,
                    action: Action::Open(p.path.clone()),
                    score: score.saturating_sub(i as u32),
                }
            })
            .collect()
    }

    fn name(&self) -> &str {
        "Program"
    }

    fn description(&self) -> &str {
        "启动已安装的程序（Win32 + UWP），支持模糊匹配"
    }

    fn icon(&self) -> &str {
        "program"
    }

    fn settings_schema(&self) -> Option<Vec<SettingItem>> {
        Some(vec![
            SettingItem {
                key: "max_results".to_string(),
                label: "最大结果数".to_string(),
                description: "搜索结果最多显示多少个程序".to_string(),
                control: SettingControl::Number {
                    min: 1,
                    max: 30,
                    default: 8,
                },
            },
            SettingItem {
                key: "hide_uninstallers".to_string(),
                label: "隐藏卸载程序".to_string(),
                description: "过滤掉名称包含 Uninstall 的快捷方式".to_string(),
                control: SettingControl::Toggle { default: true },
            },
        ])
    }

    fn on_setting_changed(&mut self, key: &str, value: &str) {
        match key {
            "max_results" => {
                if let Ok(v) = serde_json::from_str::<u32>(value) {
                    self.settings.max_results = v;
                }
            }
            "hide_uninstallers" => {
                if let Ok(v) = serde_json::from_str::<bool>(value) {
                    self.settings.hide_uninstallers = v;
                    // Rebuild to apply filter change
                    self.rebuild_index();
                }
            }
            _ => {}
        }
        self.settings.save();
    }

    fn setting_values(&self) -> Vec<(String, String)> {
        vec![
            (
                "max_results".to_string(),
                serde_json::to_string(&self.settings.max_results).unwrap_or_default(),
            ),
            (
                "hide_uninstallers".to_string(),
                serde_json::to_string(&self.settings.hide_uninstallers).unwrap_or_default(),
            ),
        ]
    }
}

/// Check if a program name looks like an uninstaller.
fn is_uninstaller(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("uninstall")
        || lower.contains("卸载")
        || lower.contains("remove")
        || lower.starts_with("unins")
}
