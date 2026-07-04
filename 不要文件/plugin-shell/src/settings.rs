// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Shell plugin settings — persisted to JSON.

use serde::{Deserialize, Serialize};

/// Available shell backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shell {
    /// Windows cmd.exe
    Cmd,
    /// Windows PowerShell (powershell.exe)
    PowerShell,
    /// PowerShell Core (pwsh.exe)
    Pwsh,
    /// Direct execution (no shell wrapper)
    RunCommand,
}

impl Shell {
    /// Human-readable name for display.
    pub fn display_name(&self) -> &'static str {
        match self {
            Shell::Cmd => "CMD",
            Shell::PowerShell => "PowerShell",
            Shell::Pwsh => "Pwsh",
            Shell::RunCommand => "直接运行",
        }
    }
}

impl Default for Shell {
    fn default() -> Self {
        Shell::Cmd
    }
}

/// Plugin settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellSettings {
    /// Which shell to use.
    pub shell: Shell,

    /// Whether to leave the shell window open after command finishes.
    pub leave_shell_open: bool,

    /// Whether Ctrl+Shift should run as administrator.
    pub run_as_administrator: bool,

    /// Whether to use Windows Terminal (wt.exe) as the terminal host.
    pub use_windows_terminal: bool,

    /// Only show the N most-used commands in history.
    pub show_only_most_used: bool,

    /// How many most-used commands to show.
    pub most_used_count: u32,

    /// Whether to show "press any key" before closing (when not leaving open).
    pub close_shell_after_press: bool,
}

impl Default for ShellSettings {
    fn default() -> Self {
        Self {
            shell: Shell::Cmd,
            leave_shell_open: false,
            run_as_administrator: true,
            use_windows_terminal: false,
            show_only_most_used: false,
            most_used_count: 5,
            close_shell_after_press: false,
        }
    }
}

impl ShellSettings {
    /// Load settings from the config file, or return defaults.
    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(settings) = serde_json::from_str(&content) {
                    return settings;
                }
            }
        }
        Self::default()
    }

    /// Save settings to the config file.
    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, json);
        }
    }

    /// Config file path: %APPDATA%/EasySearch/plugins/shell/settings.json
    fn config_path() -> std::path::PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("EasySearch")
            .join("plugins")
            .join("shell")
            .join("settings.json")
    }
}
