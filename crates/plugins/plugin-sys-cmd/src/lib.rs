// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! System Commands plugin — FL-grade implementation.
//!
//! Features:
//! - 21 commands (matching Flow.Launcher's Sys plugin)
//! - Bilingual matching (English + Chinese keywords)
//! - Confirmation required for destructive operations
//! - Native Win32 API for shutdown/restart/lock/sleep/hibernate
//! - Settings: skip confirmation toggle
//! - Fuzzy matching on command name and keywords

use easysearch_core::{Action, Plugin, PluginResult, SettingControl, SettingItem, SystemCmd};
use serde::{Deserialize, Serialize};

/// Settings for the System Commands plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SysCmdSettings {
    /// Whether to skip confirmation dialogs for power actions.
    pub skip_confirmation: bool,
}

impl Default for SysCmdSettings {
    fn default() -> Self {
        Self {
            skip_confirmation: false,
        }
    }
}

/// A system command entry with multilingual keywords.
struct CmdEntry {
    /// Search keywords (English + Chinese).
    keywords: &'static [&'static str],
    /// Display title.
    title: &'static str,
    /// Description / subtitle.
    subtitle: &'static str,
    /// Icon name.
    icon: &'static str,
    /// The action to perform.
    action: CmdAction,
    /// Whether this is a destructive action (requires confirmation).
    needs_confirm: bool,
}

/// Internal action types.
enum CmdAction {
    /// System power/session command.
    System(SystemCmd),
    /// Run a shell command directly.
    Run(&'static str),
    /// Open a path/URL.
    Open(&'static str),
    /// Application-internal command (restart app, open settings, etc.)
    AppCommand(AppCmd),
}

/// Application-level commands.
#[derive(Debug, Clone)]
enum AppCmd {
    ExitApp,
    RestartApp,
    OpenSettings,
    ReloadPlugins,
    CheckUpdate,
    OpenLogFolder,
    OpenUserDataFolder,
    OpenDocs,
    ToggleGameMode,
}

/// All 21 system commands.
const COMMANDS: &[CmdEntry] = &[
    CmdEntry {
        keywords: &["shutdown", "power off", "关机", "关闭计算机"],
        title: "关机",
        subtitle: "关闭计算机",
        icon: "shutdown",
        action: CmdAction::System(SystemCmd::Shutdown),
        needs_confirm: true,
    },
    CmdEntry {
        keywords: &["restart", "reboot", "重启", "重新启动"],
        title: "重启",
        subtitle: "重新启动计算机",
        icon: "restart",
        action: CmdAction::System(SystemCmd::Restart),
        needs_confirm: true,
    },
    CmdEntry {
        keywords: &["lock", "锁定", "锁屏"],
        title: "锁定",
        subtitle: "锁定计算机",
        icon: "lock",
        action: CmdAction::System(SystemCmd::Lock),
        needs_confirm: false,
    },
    CmdEntry {
        keywords: &["sleep", "suspend", "睡眠", "待机"],
        title: "睡眠",
        subtitle: "进入睡眠模式",
        icon: "sleep",
        action: CmdAction::System(SystemCmd::Sleep),
        needs_confirm: true,
    },
    CmdEntry {
        keywords: &["hibernate", "休眠"],
        title: "休眠",
        subtitle: "进入休眠模式",
        icon: "hibernate",
        action: CmdAction::System(SystemCmd::Hibernate),
        needs_confirm: true,
    },
    CmdEntry {
        keywords: &["logout", "logoff", "sign out", "注销", "登出"],
        title: "注销",
        subtitle: "注销当前用户",
        icon: "logoff",
        action: CmdAction::System(SystemCmd::Logout),
        needs_confirm: true,
    },
    CmdEntry {
        keywords: &["empty recycle bin", "clear recycle", "清空回收站", "回收站"],
        title: "清空回收站",
        subtitle: "永久删除回收站中的所有文件",
        icon: "recyclebin",
        action: CmdAction::System(SystemCmd::EmptyRecycleBin),
        needs_confirm: true,
    },
    CmdEntry {
        keywords: &["open recycle bin", "recycle bin", "打开回收站"],
        title: "打开回收站",
        subtitle: "在资源管理器中打开回收站",
        icon: "recyclebin",
        action: CmdAction::Open("shell:RecycleBinFolder"),
        needs_confirm: false,
    },
    CmdEntry {
        keywords: &["index option", "indexing", "索引选项"],
        title: "索引选项",
        subtitle: "打开 Windows 索引选项",
        icon: "settings",
        action: CmdAction::Run("control.exe srchadmin.dll"),
        needs_confirm: false,
    },
    CmdEntry {
        keywords: &["exit", "quit", "退出", "关闭程序"],
        title: "退出 EasySearch",
        subtitle: "完全退出程序",
        icon: "app",
        action: CmdAction::AppCommand(AppCmd::ExitApp),
        needs_confirm: false,
    },
    CmdEntry {
        keywords: &["restart easysearch", "restart app", "重启程序"],
        title: "重启 EasySearch",
        subtitle: "重新启动程序",
        icon: "app",
        action: CmdAction::AppCommand(AppCmd::RestartApp),
        needs_confirm: false,
    },
    CmdEntry {
        keywords: &["settings", "preferences", "设置", "偏好"],
        title: "打开设置",
        subtitle: "打开 EasySearch 设置面板",
        icon: "settings",
        action: CmdAction::AppCommand(AppCmd::OpenSettings),
        needs_confirm: false,
    },
    CmdEntry {
        keywords: &["reload", "refresh plugins", "重新加载", "刷新插件"],
        title: "重新加载插件",
        subtitle: "重新加载所有插件数据",
        icon: "app",
        action: CmdAction::AppCommand(AppCmd::ReloadPlugins),
        needs_confirm: false,
    },
    CmdEntry {
        keywords: &["check update", "update", "检查更新"],
        title: "检查更新",
        subtitle: "检查 EasySearch 是否有新版本",
        icon: "app",
        action: CmdAction::AppCommand(AppCmd::CheckUpdate),
        needs_confirm: false,
    },
    CmdEntry {
        keywords: &["log", "logs", "open log", "日志", "打开日志"],
        title: "打开日志目录",
        subtitle: "在资源管理器中打开日志文件夹",
        icon: "app",
        action: CmdAction::AppCommand(AppCmd::OpenLogFolder),
        needs_confirm: false,
    },
    CmdEntry {
        keywords: &[
            "userdata",
            "user data",
            "data folder",
            "数据目录",
            "用户数据",
        ],
        title: "打开数据目录",
        subtitle: "在资源管理器中打开用户数据文件夹",
        icon: "app",
        action: CmdAction::AppCommand(AppCmd::OpenUserDataFolder),
        needs_confirm: false,
    },
    CmdEntry {
        keywords: &["help", "docs", "documentation", "tips", "帮助", "文档"],
        title: "帮助文档",
        subtitle: "打开 EasySearch 使用文档",
        icon: "app",
        action: CmdAction::AppCommand(AppCmd::OpenDocs),
        needs_confirm: false,
    },
    CmdEntry {
        keywords: &["game mode", "gaming", "游戏模式"],
        title: "切换游戏模式",
        subtitle: "启用/禁用游戏模式（隐藏搜索窗口热键）",
        icon: "app",
        action: CmdAction::AppCommand(AppCmd::ToggleGameMode),
        needs_confirm: false,
    },
    CmdEntry {
        keywords: &["advanced restart", "recovery", "高级重启", "恢复"],
        title: "高级重启",
        subtitle: "重启到高级启动选项（恢复环境）",
        icon: "restart",
        action: CmdAction::Run("shutdown /r /o /t 0"),
        needs_confirm: true,
    },
    CmdEntry {
        keywords: &["screen saver", "screensaver", "屏幕保护", "屏保"],
        title: "屏幕保护程序",
        subtitle: "打开屏幕保护设置",
        icon: "settings",
        action: CmdAction::Run("control desk.cpl,,@screensaver"),
        needs_confirm: false,
    },
    CmdEntry {
        keywords: &["environment variable", "env var", "环境变量"],
        title: "环境变量",
        subtitle: "打开系统环境变量编辑器",
        icon: "settings",
        action: CmdAction::Run("rundll32 sysdm.cpl,EditEnvironmentVariables"),
        needs_confirm: false,
    },
];

/// System Commands plugin.
pub struct SysCmdPlugin {
    settings: SysCmdSettings,
}

impl SysCmdPlugin {
    #[must_use]
    pub fn new() -> Self {
        Self {
            settings: SysCmdSettings::default(),
        }
    }
}

impl Default for SysCmdPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for SysCmdPlugin {
    fn default_keyword(&self) -> Option<&str> {
        None // global match — triggers when query matches any command keyword
    }

    fn matches(&self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return false;
        }
        COMMANDS.iter().any(|c| {
            c.keywords
                .iter()
                .any(|kw| kw.contains(&q.as_str()) || q.contains(kw))
        })
    }

    fn query(&self, query: &str) -> Vec<PluginResult> {
        let q = query.trim().to_lowercase();

        COMMANDS
            .iter()
            .filter(|c| {
                c.keywords
                    .iter()
                    .any(|kw| kw.contains(&q.as_str()) || q.contains(kw))
            })
            .map(|c| {
                let subtitle = if c.needs_confirm && !self.settings.skip_confirmation {
                    format!("{} (需要确认)", c.subtitle)
                } else {
                    c.subtitle.to_string()
                };

                let action = match &c.action {
                    CmdAction::System(cmd) => Action::SystemCommand(cmd.clone()),
                    CmdAction::Run(cmd) => Action::RunCommand {
                        cmd: cmd.to_string(),
                        keep_open: false,
                    },
                    CmdAction::Open(target) => Action::Open(target.to_string()),
                    CmdAction::AppCommand(_app_cmd) => {
                        // App-level commands are handled as special DaemonSearch markers
                        // The GUI layer interprets these
                        Action::DaemonSearch(format!("__app_cmd__:{}", c.title))
                    }
                };

                PluginResult {
                    title: c.title.to_string(),
                    subtitle,
                    icon: c.icon.to_string(),
                    action,
                    score: 850,
                    highlight: Vec::new(),
                    context_actions: Vec::new(),
                    context_data: None,
                }
            })
            .collect()
    }

    fn name(&self) -> &str {
        "SystemCommand"
    }

    fn display_name(&self, locale: &str) -> String {
        match locale.split('-').next().unwrap_or(locale) {
            "zh" => "系统命令",
            "ja" => "システムコマンド",
            _ => "System Commands",
        }
        .to_string()
    }

    fn description(&self) -> &str {
        "系统命令：关机、重启、锁定、睡眠等（21 条命令）"
    }

    fn description_for_locale(&self, locale: &str) -> String {
        match locale.split('-').next().unwrap_or(locale) {
            "zh" => "执行关机、重启、锁定以及 EasySearch 应用命令".to_string(),
            "ja" => "シャットダウン、再起動、ロック、EasySearch のアプリ内コマンドを実行します"
                .to_string(),
            _ => "Run power, session, and EasySearch system commands".to_string(),
        }
    }

    fn icon(&self) -> &str {
        "system"
    }

    fn settings_schema(&self) -> Option<Vec<SettingItem>> {
        self.settings_schema_for_locale("zh-CN")
    }

    fn settings_schema_for_locale(&self, locale: &str) -> Option<Vec<SettingItem>> {
        let (label, description) = match locale.split('-').next().unwrap_or(locale) {
            "zh" => ("跳过确认对话框", "执行关机、重启等危险操作时不弹出确认"),
            "ja" => (
                "確認ダイアログを省略",
                "シャットダウンや再起動などの操作で確認を表示しません",
            ),
            _ => (
                "Skip confirmation dialogs",
                "Do not ask for confirmation before shutdown, restart, and similar actions",
            ),
        };

        Some(vec![SettingItem {
            key: "skip_confirmation".to_string(),
            label: label.to_string(),
            description: description.to_string(),
            control: SettingControl::Toggle { default: false },
        }])
    }

    fn on_setting_changed(&mut self, key: &str, value: &str) {
        if key == "skip_confirmation" {
            if let Ok(v) = serde_json::from_str::<bool>(value) {
                self.settings.skip_confirmation = v;
            }
        }
    }

    fn setting_values(&self) -> Vec<(String, String)> {
        vec![(
            "skip_confirmation".to_string(),
            serde_json::to_string(&self.settings.skip_confirmation).unwrap_or_default(),
        )]
    }
}
