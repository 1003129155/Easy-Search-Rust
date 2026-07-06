// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Plugin management page view model — plugin list with enable/disable toggles
//! and expandable/collapsible per-plugin settings panels.

use easysearch_core::SettingItem;

/// A plugin entry in the plugin list.
#[derive(Debug, Clone)]
pub struct PluginEntry {
    /// Plugin identifier (matches Plugin::name()).
    pub id: String,
    /// Plugin display name.
    pub name: String,
    /// Plugin description.
    pub description: String,
    /// Plugin icon identifier.
    pub icon: String,
    /// Keyword (if any).
    pub keyword: Option<String>,
    /// Whether the plugin is enabled.
    pub enabled: bool,
    /// Whether the settings panel is expanded.
    pub expanded: bool,
    /// Settings schema for this plugin (None = no settings).
    pub settings_schema: Option<Vec<SettingItem>>,
    /// Current setting values (key -> JSON value string).
    pub setting_values: Vec<(String, String)>,
}

/// State for the plugin management page.
#[derive(Debug, Clone)]
pub struct PluginViewModel {
    /// List of available plugins.
    pub plugins: Vec<PluginEntry>,
}

/// Messages for the plugin management page.
#[derive(Debug, Clone)]
pub enum PluginMessage {
    /// User toggled a plugin's enabled state.
    TogglePlugin { index: usize, enabled: bool },
    /// User clicked to expand/collapse a plugin's settings panel.
    ToggleExpanded { index: usize },
    /// User changed a setting value for a plugin.
    SettingChanged {
        plugin_index: usize,
        key: String,
        value: String,
    },
}

impl PluginViewModel {
    /// Create a new PluginViewModel — will be populated from actual Router plugins.
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Populate from actual registered plugins.
    /// Called during initialization to build the list from live plugin data.
    pub fn populate_from_plugins(
        &mut self,
        plugins: Vec<PluginInfo>,
    ) {
        self.plugins = plugins
            .into_iter()
            .map(|info| PluginEntry {
                id: info.id,
                name: info.name,
                description: info.description,
                icon: info.icon,
                keyword: info.keyword,
                enabled: true,
                expanded: false,
                settings_schema: info.settings_schema,
                setting_values: info.setting_values,
            })
            .collect();
    }

    /// Handle an incoming message.
    pub fn update(&mut self, msg: PluginMessage) {
        match msg {
            PluginMessage::TogglePlugin { index, enabled } => {
                if let Some(plugin) = self.plugins.get_mut(index) {
                    plugin.enabled = enabled;
                }
            }
            PluginMessage::ToggleExpanded { index } => {
                if let Some(plugin) = self.plugins.get_mut(index) {
                    plugin.expanded = !plugin.expanded;
                }
            }
            PluginMessage::SettingChanged {
                plugin_index,
                key,
                value,
            } => {
                if let Some(plugin) = self.plugins.get_mut(plugin_index) {
                    // Update local cache
                    if let Some(entry) = plugin.setting_values.iter_mut().find(|(k, _)| k == &key) {
                        entry.1 = value;
                    } else {
                        plugin.setting_values.push((key, value));
                    }
                }
            }
        }
    }
}

/// Info extracted from a Plugin trait object for the settings UI.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub keyword: Option<String>,
    pub settings_schema: Option<Vec<SettingItem>>,
    pub setting_values: Vec<(String, String)>,
}
