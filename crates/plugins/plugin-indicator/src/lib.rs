// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Plugin Indicator shows available plugin keywords as hints.

use easysearch_core::{Action, Plugin, PluginResult, PluginRouterInfo};

/// Plugin Indicator lists available plugins and their keywords.
pub struct PluginIndicatorPlugin {
    /// Cached list of keyword-triggered plugins (populated from Router).
    plugins: Vec<IndicatorEntry>,
}

#[derive(Debug, Clone)]
struct IndicatorEntry {
    /// Plugin display name.
    name: String,
    /// Keyword (e.g. "> ", "kill ", "b ").
    keyword: String,
    /// Plugin description.
    description: String,
    /// Plugin icon.
    icon: String,
}

impl PluginIndicatorPlugin {
    /// Create empty (will be populated later via `refresh`).
    #[must_use]
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    /// Refresh the plugin list from Router metadata.
    pub fn refresh(&mut self, infos: &[PluginRouterInfo]) {
        self.plugins = infos
            .iter()
            .filter(|info| info.keyword.is_some() && info.enabled)
            .map(|info| IndicatorEntry {
                name: info.name.clone(),
                keyword: info.keyword.clone().unwrap_or_default(),
                description: info.description.clone(),
                icon: info.icon.clone(),
            })
            .collect();
    }

    /// Show plugins whose keyword or name matches the query.
    fn matching_results(&self, query: &str) -> Vec<PluginResult> {
        let q = query.to_lowercase();

        self.plugins
            .iter()
            .filter(|entry| {
                let kw_lower = entry.keyword.trim().to_lowercase();
                let name_lower = entry.name.to_lowercase();
                kw_lower.starts_with(&q) || kw_lower.contains(&q) || name_lower.contains(&q)
            })
            .enumerate()
            .map(|(i, entry)| PluginResult {
                title: entry.keyword.trim().to_string(),
                subtitle: if entry.description.is_empty() {
                    format!("Activate {}", entry.name)
                } else {
                    format!("Activate {} - {}", entry.name, entry.description)
                },
                icon: entry.icon.clone(),
                action: Action::DaemonSearch(entry.keyword.clone()),
                score: 200 - i as u32,
                highlight: Vec::new(),
                context_actions: Vec::new(),
                context_data: None,
            })
            .collect()
    }
}

impl Default for PluginIndicatorPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for PluginIndicatorPlugin {
    fn default_keyword(&self) -> Option<&str> {
        None
    }

    fn matches(&self, query: &str) -> bool {
        let q = query.trim();
        if q.is_empty() {
            return false;
        }

        let q_lower = q.to_lowercase();
        self.plugins.iter().any(|entry| {
            let kw = entry.keyword.trim().to_lowercase();
            let name = entry.name.to_lowercase();
            kw.starts_with(&q_lower) || kw.contains(&q_lower) || name.contains(&q_lower)
        })
    }

    fn query(&self, query: &str) -> Vec<PluginResult> {
        let q = query.trim();
        if q.is_empty() {
            Vec::new()
        } else {
            self.matching_results(q)
        }
    }

    fn name(&self) -> &str {
        "PluginIndicator"
    }

    fn display_name(&self, locale: &str) -> String {
        match locale.split('-').next().unwrap_or(locale) {
            "zh" => "插件提示",
            "ja" => "プラグイン候補",
            _ => "Plugin Indicator",
        }
        .to_string()
    }

    fn description(&self) -> &str {
        "Show available plugin keywords and shortcuts"
    }

    fn description_for_locale(&self, locale: &str) -> String {
        match locale.split('-').next().unwrap_or(locale) {
            "zh" => "显示可用插件的关键字和快捷提示".to_string(),
            "ja" => "利用できるプラグインのキーワード候補を表示します".to_string(),
            _ => self.description().to_string(),
        }
    }

    fn icon(&self) -> &str {
        "plugin"
    }
}
