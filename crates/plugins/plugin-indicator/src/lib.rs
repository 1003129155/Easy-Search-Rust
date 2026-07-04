// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Plugin Indicator — shows available plugin keywords as hints.
//!
//! Behavior (same as Flow.Launcher's PluginIndicator):
//! - When input is empty: show all available keyword-triggered plugins
//! - When input partially matches a keyword or plugin name: show matching plugins
//! - Selecting a result fills the keyword into the search box
//!
//! This plugin does NOT have a keyword itself — it's a global match plugin
//! that only activates when no other keyword-plugin matches first.

use easysearch_core::{Action, Plugin, PluginResult, PluginRouterInfo};

/// Plugin Indicator — lists available plugins and their keywords.
pub struct PluginIndicatorPlugin {
    /// Cached list of keyword-triggered plugins (populated from Router).
    plugins: Vec<IndicatorEntry>,
}

#[derive(Debug, Clone)]
struct IndicatorEntry {
    /// Plugin name.
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
    /// Call this after all plugins are registered in the Router.
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

    /// Show all available keyword plugins (for empty query / home screen).
    #[allow(dead_code)]
    fn all_results(&self) -> Vec<PluginResult> {
        self.plugins
            .iter()
            .enumerate()
            .map(|(i, entry)| PluginResult {
                title: entry.keyword.trim().to_string(),
                subtitle: format!("激活 {} 插件", entry.name),
                icon: entry.icon.clone(),
                action: Action::DaemonSearch(entry.keyword.clone()),
                score: 100 - i as u32,
            })
            .collect()
    }

    /// Show plugins whose keyword or name matches the query.
    fn matching_results(&self, query: &str) -> Vec<PluginResult> {
        let q = query.to_lowercase();

        self.plugins
            .iter()
            .filter(|entry| {
                let kw_lower = entry.keyword.trim().to_lowercase();
                let name_lower = entry.name.to_lowercase();
                // Match if query is a prefix of the keyword, or contained in the name
                kw_lower.starts_with(&q)
                    || kw_lower.contains(&q)
                    || name_lower.contains(&q)
            })
            .enumerate()
            .map(|(i, entry)| PluginResult {
                title: entry.keyword.trim().to_string(),
                subtitle: format!("激活 {} — {}", entry.name, entry.description),
                icon: entry.icon.clone(),
                action: Action::DaemonSearch(entry.keyword.clone()),
                score: 200 - i as u32,
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
        None // global match — participates when no keyword plugin claims the query
    }

    fn matches(&self, query: &str) -> bool {
        let q = query.trim();
        // Activate when:
        // 1. Query is empty (home screen)
        // 2. Query partially matches a keyword or plugin name
        // 3. Query does NOT already start with a full recognized keyword
        //    (Router handles that — this plugin won't be called in that case)
        if q.is_empty() {
            return false;
        }
        // Check if query partially matches any keyword or name
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

    fn description(&self) -> &str {
        "显示可用插件关键词提示"
    }

    fn icon(&self) -> &str {
        "plugin"
    }
}
