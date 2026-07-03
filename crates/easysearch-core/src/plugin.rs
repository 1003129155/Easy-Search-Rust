// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Plugin system core types.
//!
//! Defines the [`Plugin`] trait and common types shared by all plugins.

use serde::{Deserialize, Serialize};

/// A single result item produced by a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResult {
    /// Display title (primary text).
    pub title: String,
    /// Subtitle / secondary text.
    pub subtitle: String,
    /// Icon identifier (path or built-in name).
    pub icon: String,
    /// Action to perform when the user selects this result.
    pub action: Action,
    /// Relevance score (higher = better match).
    pub score: u32,
}

/// What happens when a result is executed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    /// Open a URL or file path via the system shell.
    Open(String),
    /// Copy text to the clipboard.
    Copy(String),
    /// Run a shell command.
    RunCommand { cmd: String, keep_open: bool },
    /// Send a search query to the daemon (file search).
    DaemonSearch(String),
    /// Execute a system command (shutdown, lock, etc.).
    SystemCommand(SystemCmd),
    /// No action (informational result).
    None,
}

/// System-level commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemCmd {
    Shutdown,
    Restart,
    Lock,
    Sleep,
    Hibernate,
    Logout,
    EmptyRecycleBin,
}

/// The plugin trait. Each built-in plugin implements this.
pub trait Plugin: Send + Sync {
    /// Default keyword prefix that triggers this plugin (e.g. "> ", "kill ").
    /// Return `None` for plugins that match based on content (calculator, URL).
    /// This is the compile-time default; runtime keyword is managed by Router.
    fn default_keyword(&self) -> Option<&str>;

    /// Whether this plugin wants to handle the given raw query.
    /// Called only for plugins with no keyword (global match).
    fn matches(&self, query: &str) -> bool {
        let _ = query;
        false
    }

    /// Produce results for the given query.
    /// For keyword-triggered plugins, `query` has the keyword stripped.
    fn query(&self, query: &str) -> Vec<PluginResult>;

    /// Human-readable name of this plugin.
    fn name(&self) -> &str;

    /// Plugin description shown in the settings panel.
    fn description(&self) -> &str {
        ""
    }

    /// Plugin icon identifier.
    fn icon(&self) -> &str {
        "plugin"
    }

    /// Settings schema — defines what options this plugin exposes in the settings UI.
    /// Return `None` if the plugin has no configurable settings.
    fn settings_schema(&self) -> Option<Vec<SettingItem>> {
        None
    }

    /// Called when a setting value changes from the UI.
    /// `key` is the `SettingItem.key`, `value` is the new JSON-serialized value.
    fn on_setting_changed(&mut self, _key: &str, _value: &str) {
        // default: no-op
    }

    /// Get current setting values as key-value pairs (for populating the UI).
    /// Values are JSON-serialized strings.
    fn setting_values(&self) -> Vec<(String, String)> {
        Vec::new()
    }
}

// ─── Settings Schema Types ───────────────────────────────────────────────────

/// A single setting item that the plugin exposes to the settings UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingItem {
    /// Unique key for this setting (e.g. "shell_type", "leave_open").
    pub key: String,
    /// Display label in the UI.
    pub label: String,
    /// Optional description / help text shown below the control.
    pub description: String,
    /// The type of UI control to render.
    pub control: SettingControl,
}

/// What kind of UI control to use for a setting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SettingControl {
    /// On/off toggle switch.
    Toggle { default: bool },
    /// Dropdown with a list of options. `options` is a list of (value, display_label).
    Dropdown {
        options: Vec<(String, String)>,
        default: String,
    },
    /// Text input field.
    TextInput {
        placeholder: String,
        default: String,
    },
    /// Numeric spinner (integer).
    Number { min: i64, max: i64, default: i64 },
}

/// Router dispatches queries to the appropriate plugin(s).
pub struct Router {
    plugins: Vec<PluginSlot>,
}

/// A registered plugin with its runtime-configurable keyword.
struct PluginSlot {
    plugin: Box<dyn Plugin>,
    /// Runtime keyword override. `None` means use `plugin.default_keyword()`.
    keyword_override: Option<Option<String>>,
    /// Whether this plugin is enabled.
    enabled: bool,
}

impl PluginSlot {
    /// Get the effective keyword for routing.
    fn effective_keyword(&self) -> Option<&str> {
        match &self.keyword_override {
            Some(ovr) => ovr.as_deref(),
            None => self.plugin.default_keyword(),
        }
    }
}

impl Router {
    /// Create a new empty router.
    #[must_use]
    pub fn new() -> Self {
        Self { plugins: Vec::new() }
    }

    /// Register a plugin with the router (uses default keyword).
    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(PluginSlot {
            plugin,
            keyword_override: None,
            enabled: true,
        });
    }

    /// Set a custom keyword for a plugin by name.
    /// Pass `Some("kw ")` to set a keyword, or `None` to make it a global plugin.
    pub fn set_keyword(&mut self, plugin_name: &str, keyword: Option<String>) {
        if let Some(slot) = self.plugins.iter_mut().find(|s| s.plugin.name() == plugin_name) {
            slot.keyword_override = Some(keyword);
        }
    }

    /// Enable or disable a plugin by name.
    pub fn set_enabled(&mut self, plugin_name: &str, enabled: bool) {
        if let Some(slot) = self.plugins.iter_mut().find(|s| s.plugin.name() == plugin_name) {
            slot.enabled = enabled;
        }
    }

    /// Get a mutable reference to a plugin by name (for settings changes).
    pub fn plugin_mut(&mut self, name: &str) -> Option<&mut (dyn Plugin)> {
        for slot in &mut self.plugins {
            if slot.plugin.name() == name {
                return Some(slot.plugin.as_mut());
            }
        }
        None
    }

    /// Get plugin metadata for the settings UI.
    pub fn plugin_infos(&self) -> Vec<PluginRouterInfo> {
        self.plugins
            .iter()
            .map(|slot| PluginRouterInfo {
                name: slot.plugin.name().to_string(),
                description: slot.plugin.description().to_string(),
                icon: slot.plugin.icon().to_string(),
                keyword: slot.effective_keyword().map(|s| s.to_string()),
                default_keyword: slot.plugin.default_keyword().map(|s| s.to_string()),
                enabled: slot.enabled,
            })
            .collect()
    }

    /// Route a query to matching plugins and collect results.
    pub fn query(&self, raw_query: &str) -> Vec<PluginResult> {
        let mut results = Vec::new();
        let query_trimmed = raw_query.trim();

        if query_trimmed.is_empty() {
            return results;
        }

        for slot in &self.plugins {
            if !slot.enabled {
                continue;
            }

            if let Some(kw) = slot.effective_keyword() {
                if let Some(stripped) = query_trimmed.strip_prefix(kw) {
                    let stripped = stripped.trim_start();
                    results.extend(slot.plugin.query(stripped));
                }
            } else if slot.plugin.matches(query_trimmed) {
                results.extend(slot.plugin.query(query_trimmed));
            }
        }

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    }
}

/// Info about a registered plugin (for the settings UI).
#[derive(Debug, Clone)]
pub struct PluginRouterInfo {
    pub name: String,
    pub description: String,
    pub icon: String,
    /// Current effective keyword (None = global match).
    pub keyword: Option<String>,
    /// The plugin's built-in default keyword.
    pub default_keyword: Option<String>,
    /// Whether the plugin is enabled.
    pub enabled: bool,
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}
