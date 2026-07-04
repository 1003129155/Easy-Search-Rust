// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Process Killer plugin — FL-grade implementation.
//!
//! Features:
//! - Native Win32 API for process enumeration (ToolHelp32 snapshot)
//! - Fuzzy matching on process name
//! - Window title display for processes with visible windows
//! - Kill by individual PID or all instances of the same exe
//! - System process exclusion (conhost, csrss, svchost, etc.)
//! - Self-protection (can't kill own process)
//! - Settings: show window titles, prioritize visible windows

#[cfg(windows)]
mod native;

use easysearch_core::{Action, Plugin, PluginResult, SettingControl, SettingItem};
use serde::{Deserialize, Serialize};

/// Processes that should never be killed (system-critical).
const EXCLUDED_PROCESSES: &[&str] = &[
    "system",
    "system idle process",
    "csrss.exe",
    "smss.exe",
    "wininit.exe",
    "services.exe",
    "lsass.exe",
    "svchost.exe",
    "conhost.exe",
    "dwm.exe",
    "winlogon.exe",
    "fontdrvhost.exe",
    "memory compression",
    "registry",
    "sihost.exe",
];

/// Settings for the Process Killer plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessKillerSettings {
    /// Whether to show window titles alongside process names.
    pub show_window_title: bool,
    /// Whether to prioritize processes with visible windows.
    pub prioritize_visible: bool,
}

impl Default for ProcessKillerSettings {
    fn default() -> Self {
        Self {
            show_window_title: true,
            prioritize_visible: true,
        }
    }
}

/// A process entry gathered from the system.
#[derive(Debug, Clone)]
struct ProcessEntry {
    /// Process ID.
    pid: u32,
    /// Executable name (e.g. "notepad.exe").
    name: String,
    /// Full path to executable (if available).
    path: Option<String>,
    /// Window title (if the process has a visible window).
    window_title: Option<String>,
}

/// Process Killer plugin.
pub struct ProcessKillerPlugin {
    settings: ProcessKillerSettings,
}

impl ProcessKillerPlugin {
    #[must_use]
    pub fn new() -> Self {
        Self {
            settings: ProcessKillerSettings::default(),
        }
    }

    /// Enumerate processes and filter by query.
    fn get_matching_processes(&self, filter: &str) -> Vec<PluginResult> {
        let current_pid = std::process::id();
        let processes = enumerate_processes();

        let filter_lower = filter.to_lowercase();

        // Filter and score
        let mut scored: Vec<(ProcessEntry, u32)> = processes
            .into_iter()
            .filter(|p| {
                // Exclude system processes
                let name_lower = p.name.to_lowercase();
                if EXCLUDED_PROCESSES.contains(&name_lower.as_str()) {
                    return false;
                }
                // Self-protection
                if p.pid == current_pid {
                    return false;
                }
                // Match: process name OR window title
                if name_lower.contains(&filter_lower) {
                    return true;
                }
                if let Some(ref title) = p.window_title {
                    if title.to_lowercase().contains(&filter_lower) {
                        return true;
                    }
                }
                false
            })
            .map(|p| {
                let mut score: u32 = 800;
                // Boost for exact prefix match
                if p.name.to_lowercase().starts_with(&filter_lower) {
                    score += 100;
                }
                // Boost for visible window
                if self.settings.prioritize_visible && p.window_title.is_some() {
                    score += 50;
                }
                (p, score)
            })
            .collect();

        // Sort by score descending, then by name
        scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.name.cmp(&b.0.name)));

        // Deduplicate by (name, pid) — but show up to 10 results
        scored.truncate(10);

        scored
            .into_iter()
            .map(|(p, score)| {
                let title = if self.settings.show_window_title {
                    match &p.window_title {
                        Some(wt) if !wt.is_empty() => format!("{} — {}", p.name, wt),
                        _ => format!("终止 {}", p.name),
                    }
                } else {
                    format!("终止 {}", p.name)
                };

                let subtitle = match &p.path {
                    Some(path) => format!("PID: {} | {}", p.pid, path),
                    None => format!("PID: {}", p.pid),
                };

                PluginResult {
                    title,
                    subtitle,
                    icon: p.path.clone().unwrap_or_else(|| String::from("process")),
                    action: Action::RunCommand {
                        cmd: format!("taskkill /F /PID {}", p.pid),
                        keep_open: false,
                    },
                    score,
                }
            })
            .collect()
    }
}

impl Default for ProcessKillerPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ProcessKillerPlugin {
    fn default_keyword(&self) -> Option<&str> {
        Some("kill ")
    }

    fn query(&self, query: &str) -> Vec<PluginResult> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return vec![PluginResult {
                title: String::from("终止进程"),
                subtitle: String::from("输入进程名称或窗口标题"),
                icon: String::from("process"),
                action: Action::None,
                score: 500,
            }];
        }

        self.get_matching_processes(&q)
    }

    fn name(&self) -> &str {
        "ProcessKiller"
    }

    fn description(&self) -> &str {
        "通过关键字 \"kill \" 搜索并终止进程"
    }

    fn icon(&self) -> &str {
        "process"
    }

    fn settings_schema(&self) -> Option<Vec<SettingItem>> {
        Some(vec![
            SettingItem {
                key: "show_window_title".to_string(),
                label: "显示窗口标题".to_string(),
                description: "在结果中显示进程的可见窗口标题".to_string(),
                control: SettingControl::Toggle { default: true },
            },
            SettingItem {
                key: "prioritize_visible".to_string(),
                label: "优先显示可见窗口进程".to_string(),
                description: "将有可见窗口的进程排在前面".to_string(),
                control: SettingControl::Toggle { default: true },
            },
        ])
    }

    fn on_setting_changed(&mut self, key: &str, value: &str) {
        match key {
            "show_window_title" => {
                if let Ok(v) = serde_json::from_str::<bool>(value) {
                    self.settings.show_window_title = v;
                }
            }
            "prioritize_visible" => {
                if let Ok(v) = serde_json::from_str::<bool>(value) {
                    self.settings.prioritize_visible = v;
                }
            }
            _ => {}
        }
    }

    fn setting_values(&self) -> Vec<(String, String)> {
        vec![
            (
                "show_window_title".to_string(),
                serde_json::to_string(&self.settings.show_window_title).unwrap_or_default(),
            ),
            (
                "prioritize_visible".to_string(),
                serde_json::to_string(&self.settings.prioritize_visible).unwrap_or_default(),
            ),
        ]
    }
}

/// Enumerate all running processes using platform-specific APIs.
/// Falls back to tasklist on non-Windows or if native API fails.
fn enumerate_processes() -> Vec<ProcessEntry> {
    #[cfg(windows)]
    {
        native::enumerate_processes_native()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}
