// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Program launcher plugin.
//!
//! Features:
//! - Win32 programs: scans Start Menu shortcuts and executables
//! - UWP/MSIX apps: enumerates installed apps
//! - JSON disk cache for fast startup
//! - Fuzzy matching with periodic refresh

mod cache;
mod fuzzy;
mod scanner;
mod settings;
mod uwp;

use easysearch_core::{
    Action, ContextAction, ContextData, Plugin, PluginResult, SettingControl, SettingItem,
};
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
            Some(cached) => (cached.entries, true),
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
        let entries = build_program_index(&self.settings);
        ProgramCache::save(&entries);

        if let Ok(mut lock) = self.programs.lock() {
            *lock = entries;
        }

        if let Ok(mut ts) = self.last_rebuild.lock() {
            *ts = Instant::now();
        }
    }

    /// Check if it's time for a periodic rebuild (every 30 minutes).
    fn maybe_refresh(&self) {
        let should_rebuild = self.last_rebuild.lock().is_ok_and(|mut ts| {
            if ts.elapsed() <= Duration::from_secs(30 * 60) {
                return false;
            }
            *ts = Instant::now();
            true
        });

        if should_rebuild {
            let programs = Arc::clone(&self.programs);
            let settings = self.settings.clone();
            std::thread::Builder::new()
                .name("program-index-refresh".into())
                .spawn(move || {
                    let entries = build_program_index(&settings);
                    ProgramCache::save(&entries);
                    if let Ok(mut current) = programs.lock() {
                        *current = entries;
                    }
                })
                .ok();
        }
    }
}

fn build_program_index(settings: &ProgramSettings) -> Vec<ProgramEntry> {
    let mut entries = scan_start_menu();
    entries.extend(scan_uwp_apps());

    if settings.hide_uninstallers {
        entries.retain(|entry| !is_uninstaller(&entry.name));
    }

    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    entries.dedup_by(|a, b| a.name.to_lowercase() == b.name.to_lowercase());
    entries
}

impl Default for ProgramPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ProgramPlugin {
    fn default_keyword(&self) -> Option<&str> {
        None
    }

    fn matches(&self, _query: &str) -> bool {
        true
    }

    fn priority(&self) -> i32 {
        5
    }

    fn query(&self, query: &str) -> Vec<PluginResult> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }

        self.maybe_refresh();

        let programs = self.programs.lock().unwrap_or_else(|e| e.into_inner());

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
                let context_actions = build_program_context_actions(&p.name, &p.path, p.source);
                let parent_path = std::path::Path::new(&p.path)
                    .parent()
                    .map(|pp| pp.to_string_lossy().to_string())
                    .unwrap_or_default();
                PluginResult {
                    title: p.name.clone(),
                    subtitle: p.path.clone(),
                    icon,
                    action: Action::Open(p.path.clone()),
                    score: score.saturating_sub(i as u32),
                    highlight: Vec::new(),
                    context_actions,
                    context_data: Some(ContextData {
                        is_directory: false,
                        file_path: p.path.clone(),
                        parent_path,
                    }),
                }
            })
            .collect()
    }

    fn name(&self) -> &str {
        "Program"
    }

    fn display_name(&self, locale: &str) -> String {
        match locale_prefix(locale) {
            "zh" => "程序",
            "ja" => "プログラム",
            _ => "Program",
        }
        .to_string()
    }

    fn description(&self) -> &str {
        "Launch installed Win32 and UWP applications with fuzzy matching"
    }

    fn description_for_locale(&self, locale: &str) -> String {
        match locale_prefix(locale) {
            "zh" => "启动已安装的 Win32 和 UWP 应用，并支持模糊匹配".to_string(),
            "ja" => "インストール済みの Win32 / UWP アプリをあいまい検索で起動します".to_string(),
            _ => self.description().to_string(),
        }
    }

    fn icon(&self) -> &str {
        "program"
    }

    fn settings_schema(&self) -> Option<Vec<SettingItem>> {
        self.settings_schema_for_locale("en")
    }

    fn settings_schema_for_locale(&self, locale: &str) -> Option<Vec<SettingItem>> {
        let texts = match locale_prefix(locale) {
            "zh" => [
                ("最大结果数", "搜索结果最多显示多少个程序"),
                ("隐藏卸载程序", "过滤掉名称中包含 Uninstall 的快捷方式"),
            ],
            "ja" => [
                ("最大結果数", "検索結果に表示するプログラム数の上限です"),
                (
                    "アンインストーラーを隠す",
                    "名前に Uninstall を含むショートカットを除外します",
                ),
            ],
            _ => [
                (
                    "Maximum results",
                    "How many programs to show at most in search results",
                ),
                (
                    "Hide uninstallers",
                    "Filter out shortcuts whose names contain Uninstall",
                ),
            ],
        };

        Some(vec![
            SettingItem {
                key: "max_results".to_string(),
                label: texts[0].0.to_string(),
                description: texts[0].1.to_string(),
                control: SettingControl::Number {
                    min: 1,
                    max: 30,
                    default: 8,
                },
            },
            SettingItem {
                key: "hide_uninstallers".to_string(),
                label: texts[1].0.to_string(),
                description: texts[1].1.to_string(),
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

/// Build context actions for a program result.
fn build_program_context_actions(
    title: &str,
    path: &str,
    source: ProgramSource,
) -> Vec<ContextAction> {
    use easysearch_core::context_labels as cl;
    use quick_launch_store::global_store;

    let is_saved = global_store()
        .lock()
        .map(|store| store.contains(path))
        .unwrap_or(false);

    let mut actions = Vec::new();

    // "Run as administrator" — only for Win32 programs (not UWP)
    if source == ProgramSource::StartMenu {
        actions.push(ContextAction {
            label: cl::run_as_admin(),
            action: Action::OpenAsAdmin(path.to_string()),
            shortcut_hint: String::new(),
        });
    }

    // "Open file location"
    if source == ProgramSource::StartMenu {
        actions.push(ContextAction {
            label: cl::open_file_location(),
            action: Action::OpenContainingFolder(path.to_string()),
            shortcut_hint: "Ctrl+Enter".to_string(),
        });
    }

    // "Add to / Remove from Quick Launch"
    actions.push(ContextAction {
        label: cl::toggle_quick_launch(is_saved),
        action: Action::ToggleQuickLaunch {
            path: path.to_string(),
            title: title.to_string(),
        },
        shortcut_hint: String::new(),
    });

    // "Copy path"
    actions.push(ContextAction {
        label: cl::copy_path(),
        action: Action::Copy(path.to_string()),
        shortcut_hint: String::new(),
    });

    // "Copy name"
    actions.push(ContextAction {
        label: cl::copy_name(),
        action: Action::Copy(title.to_string()),
        shortcut_hint: String::new(),
    });

    // "Windows context menu" — only for Win32 programs with a real file path
    if source == ProgramSource::StartMenu {
        actions.push(ContextAction {
            label: cl::windows_context_menu(),
            action: Action::ShowFileContextMenu {
                path: path.to_string(),
                is_dir: false,
            },
            shortcut_hint: "Alt+Enter".to_string(),
        });
    }

    actions
}

/// Check if a program name looks like an uninstaller.
fn is_uninstaller(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("uninstall")
        || lower.contains("卸载")
        || lower.contains("remove")
        || lower.starts_with("unins")
}

fn locale_prefix(locale: &str) -> &str {
    locale.split('-').next().unwrap_or(locale)
}
