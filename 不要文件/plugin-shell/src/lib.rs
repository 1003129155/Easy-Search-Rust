// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Shell command plugin — FL-grade implementation.
//!
//! Features:
//! - Command history with execution counts (persisted to JSON)
//! - Multiple shell backends: CMD, PowerShell, Pwsh (PowerShell Core)
//! - Admin elevation (Ctrl+Shift triggers run-as-admin)
//! - Path autocomplete (when input ends with `\` or `/`)
//! - Windows Terminal integration (optional)
//! - Context menu: Run as admin, Run normally, Copy command

mod history;
mod settings;

use easysearch_core::{Action, Plugin, PluginResult, SettingControl, SettingItem};
pub use history::CommandHistory;
pub use settings::{Shell, ShellSettings};

/// Shell command plugin.
pub struct ShellPlugin {
    settings: ShellSettings,
    history: CommandHistory,
}

impl ShellPlugin {
    /// Create a new Shell plugin with default settings.
    #[must_use]
    pub fn new() -> Self {
        let settings = ShellSettings::load();
        let history = CommandHistory::load();
        Self { settings, history }
    }

    /// Create with explicit settings (for testing or custom config).
    #[must_use]
    pub fn with_settings(settings: ShellSettings) -> Self {
        let history = CommandHistory::load();
        Self { settings, history }
    }

    /// Build the command line to execute a shell command.
    fn build_command(&self, cmd: &str, run_as_admin: bool) -> Action {
        let shell_cmd = build_shell_command(cmd, &self.settings);
        if run_as_admin {
            Action::RunCommand {
                cmd: format!("__admin__{}", shell_cmd),
                keep_open: self.settings.leave_shell_open,
            }
        } else {
            Action::RunCommand {
                cmd: shell_cmd,
                keep_open: self.settings.leave_shell_open,
            }
        }
    }

    /// Get results from command history (when query is empty).
    fn results_from_history(&self) -> Vec<PluginResult> {
        let mut entries: Vec<_> = self.history.entries().collect();
        entries.sort_by(|a, b| b.1.cmp(a.1));

        if self.settings.show_only_most_used {
            entries.truncate(self.settings.most_used_count as usize);
        }

        entries
            .into_iter()
            .enumerate()
            .map(|(i, (cmd, count))| PluginResult {
                title: cmd.clone(),
                subtitle: format!("已执行 {} 次", count),
                icon: String::from("terminal"),
                action: self.build_command(cmd, false),
                score: 900 - i as u32,
            })
            .collect()
    }

    /// Get history commands matching the current query.
    fn history_matches(&self, query: &str) -> Vec<PluginResult> {
        let q = query.to_lowercase();
        let mut entries: Vec<_> = self
            .history
            .entries()
            .filter(|(cmd, _)| cmd.to_lowercase().contains(&q) && *cmd != query)
            .collect();

        entries.sort_by(|a, b| b.1.cmp(a.1));

        if self.settings.show_only_most_used {
            entries.truncate(self.settings.most_used_count as usize);
        }

        entries
            .into_iter()
            .enumerate()
            .map(|(i, (cmd, count))| PluginResult {
                title: cmd.clone(),
                subtitle: format!("已执行 {} 次", count),
                icon: String::from("terminal"),
                action: self.build_command(cmd, false),
                score: 800 - i as u32,
            })
            .collect()
    }

    /// Path autocomplete results when input looks like a directory path.
    fn path_autocomplete(&self, cmd: &str) -> Vec<PluginResult> {
        let expanded = expand_env_vars(cmd);

        // Check if it ends with a separator and the directory exists
        let (base_dir, prefix) = if expanded.ends_with('\\') || expanded.ends_with('/') {
            if std::path::Path::new(&expanded).is_dir() {
                (expanded.clone(), cmd.to_string())
            } else {
                return Vec::new();
            }
        } else {
            // Try parent directory for partial path completion
            let path = std::path::Path::new(&expanded);
            match path.parent() {
                Some(parent) if parent.is_dir() => {
                    let parent_str = parent.to_string_lossy().to_string();
                    let cmd_parent = std::path::Path::new(cmd)
                        .parent()
                        .map(|p| {
                            let s = p.to_string_lossy().to_string();
                            if s.ends_with('\\') || s.ends_with('/') {
                                s
                            } else {
                                format!("{s}\\")
                            }
                        })
                        .unwrap_or_default();
                    (parent_str, cmd_parent)
                }
                _ => return Vec::new(),
            }
        };

        let Ok(entries) = std::fs::read_dir(&base_dir) else {
            return Vec::new();
        };

        let mut results: Vec<PluginResult> = entries
            .flatten()
            .filter_map(|entry| {
                let file_name = entry.file_name().to_string_lossy().to_string();
                let full_display = format!("{prefix}{file_name}");

                // Filter: must start with the user's input (case insensitive)
                if !full_display.to_lowercase().starts_with(&cmd.to_lowercase()) {
                    return None;
                }

                Some(PluginResult {
                    title: full_display.clone(),
                    subtitle: if entry.path().is_dir() {
                        String::from("目录")
                    } else {
                        String::from("文件")
                    },
                    icon: String::from("terminal"),
                    action: self.build_command(&full_display, false),
                    score: 600,
                })
            })
            .collect();

        results.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        results.truncate(8);
        results
    }
}

impl Default for ShellPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for ShellPlugin {
    fn default_keyword(&self) -> Option<&str> {
        Some("> ")
    }

    fn query(&self, query: &str) -> Vec<PluginResult> {
        let cmd = query.trim();

        // Empty query -> show history
        if cmd.is_empty() {
            return self.results_from_history();
        }

        let mut results = Vec::new();

        // Primary result: execute the current command
        let execution_count = self.history.count(cmd);
        let subtitle = if execution_count > 0 {
            format!(
                "通过 {} 执行 (已执行 {} 次)",
                self.settings.shell.display_name(),
                execution_count
            )
        } else {
            format!("通过 {} 执行", self.settings.shell.display_name())
        };

        results.push(PluginResult {
            title: cmd.to_string(),
            subtitle,
            icon: String::from("terminal"),
            action: self.build_command(cmd, false),
            score: 5000, // highest priority
        });

        // History matches
        results.extend(self.history_matches(cmd));

        // Path autocomplete
        let autocomplete = self.path_autocomplete(cmd);
        if !autocomplete.is_empty() {
            results.extend(autocomplete);
        }

        results
    }

    fn name(&self) -> &str {
        "Shell"
    }

    fn description(&self) -> &str {
        "通过关键字 \"> \" 执行 Shell 命令，支持命令历史和路径补全"
    }

    fn icon(&self) -> &str {
        "terminal"
    }

    fn settings_schema(&self) -> Option<Vec<SettingItem>> {
        Some(vec![
            SettingItem {
                key: "shell".to_string(),
                label: "Shell 类型".to_string(),
                description: "选择执行命令使用的 Shell".to_string(),
                control: SettingControl::Dropdown {
                    options: vec![
                        ("Cmd".to_string(), "CMD".to_string()),
                        ("PowerShell".to_string(), "PowerShell".to_string()),
                        ("Pwsh".to_string(), "Pwsh (PowerShell Core)".to_string()),
                        ("RunCommand".to_string(), "直接运行".to_string()),
                    ],
                    default: "Cmd".to_string(),
                },
            },
            SettingItem {
                key: "leave_shell_open".to_string(),
                label: "执行后保持窗口打开".to_string(),
                description: "命令执行完毕后不关闭终端窗口".to_string(),
                control: SettingControl::Toggle { default: false },
            },
            SettingItem {
                key: "run_as_administrator".to_string(),
                label: "Ctrl+Shift 以管理员运行".to_string(),
                description: "按住 Ctrl+Shift 回车时以管理员权限执行".to_string(),
                control: SettingControl::Toggle { default: true },
            },
            SettingItem {
                key: "use_windows_terminal".to_string(),
                label: "使用 Windows Terminal".to_string(),
                description: "使用 wt.exe 作为终端宿主".to_string(),
                control: SettingControl::Toggle { default: false },
            },
            SettingItem {
                key: "show_only_most_used".to_string(),
                label: "仅显示最常用命令".to_string(),
                description: "历史列表只显示使用频率最高的命令".to_string(),
                control: SettingControl::Toggle { default: false },
            },
            SettingItem {
                key: "most_used_count".to_string(),
                label: "最常用命令数量".to_string(),
                description: "历史列表最多显示多少条命令".to_string(),
                control: SettingControl::Number {
                    min: 1,
                    max: 20,
                    default: 5,
                },
            },
        ])
    }

    fn on_setting_changed(&mut self, key: &str, value: &str) {
        match key {
            "shell" => {
                if let Ok(shell) = serde_json::from_str::<Shell>(value) {
                    self.settings.shell = shell;
                }
            }
            "leave_shell_open" => {
                if let Ok(v) = serde_json::from_str::<bool>(value) {
                    self.settings.leave_shell_open = v;
                }
            }
            "run_as_administrator" => {
                if let Ok(v) = serde_json::from_str::<bool>(value) {
                    self.settings.run_as_administrator = v;
                }
            }
            "use_windows_terminal" => {
                if let Ok(v) = serde_json::from_str::<bool>(value) {
                    self.settings.use_windows_terminal = v;
                }
            }
            "show_only_most_used" => {
                if let Ok(v) = serde_json::from_str::<bool>(value) {
                    self.settings.show_only_most_used = v;
                }
            }
            "most_used_count" => {
                if let Ok(v) = serde_json::from_str::<u32>(value) {
                    self.settings.most_used_count = v;
                }
            }
            _ => {}
        }
        self.settings.save();
    }

    fn setting_values(&self) -> Vec<(String, String)> {
        vec![
            ("shell".to_string(), serde_json::to_string(&self.settings.shell).unwrap_or_default()),
            ("leave_shell_open".to_string(), serde_json::to_string(&self.settings.leave_shell_open).unwrap_or_default()),
            ("run_as_administrator".to_string(), serde_json::to_string(&self.settings.run_as_administrator).unwrap_or_default()),
            ("use_windows_terminal".to_string(), serde_json::to_string(&self.settings.use_windows_terminal).unwrap_or_default()),
            ("show_only_most_used".to_string(), serde_json::to_string(&self.settings.show_only_most_used).unwrap_or_default()),
            ("most_used_count".to_string(), serde_json::to_string(&self.settings.most_used_count).unwrap_or_default()),
        ]
    }
}

/// Build the actual shell command string based on settings.
fn build_shell_command(cmd: &str, settings: &ShellSettings) -> String {
    match settings.shell {
        Shell::Cmd => {
            let flag = if settings.leave_shell_open { "/k" } else { "/c" };
            if settings.use_windows_terminal {
                format!("wt.exe cmd {flag} {cmd}")
            } else {
                format!("cmd.exe {flag} {cmd}")
            }
        }
        Shell::PowerShell => {
            if settings.use_windows_terminal {
                if settings.leave_shell_open {
                    format!("wt.exe powershell -NoExit -Command {cmd}")
                } else {
                    format!("wt.exe powershell -Command {cmd}")
                }
            } else if settings.leave_shell_open {
                format!("powershell.exe -NoExit -Command {cmd}")
            } else {
                format!("powershell.exe -Command {cmd}")
            }
        }
        Shell::Pwsh => {
            if settings.use_windows_terminal {
                if settings.leave_shell_open {
                    format!("wt.exe pwsh -NoExit -Command {cmd}")
                } else {
                    format!("wt.exe pwsh -Command {cmd}")
                }
            } else if settings.leave_shell_open {
                format!("pwsh.exe -NoExit -Command {cmd}")
            } else {
                format!("pwsh.exe -Command {cmd}")
            }
        }
        Shell::RunCommand => {
            // Direct execution — split into command and args
            cmd.to_string()
        }
    }
}

/// Expand Windows environment variables in a string (%USERPROFILE%, etc.)
fn expand_env_vars(s: &str) -> String {
    let mut result = s.to_string();
    // Simple expansion: find %VAR% and replace
    while let Some(start) = result.find('%') {
        if let Some(end) = result[start + 1..].find('%') {
            let var_name = &result[start + 1..start + 1 + end];
            if let Ok(value) = std::env::var(var_name) {
                result = format!("{}{}{}", &result[..start], value, &result[start + 2 + end..]);
            } else {
                break;
            }
        } else {
            break;
        }
    }
    result
}
