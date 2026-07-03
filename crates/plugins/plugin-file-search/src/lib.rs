// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! File search plugin: bridges the search engine into the plugin system.
//!
//! This is a thin adapter — it takes user input, calls `SearchEngine::search()`,
//! and converts `EsSearchResult` into `PluginResult`.

use std::sync::Arc;

use easysearch_core::{Action, EsSearchResult, Plugin, PluginResult};
use easysearch_engine::SearchEngine;

/// File search plugin that wraps the search engine.
pub struct FileSearchPlugin {
    engine: Arc<SearchEngine>,
}

impl FileSearchPlugin {
    /// Create a new file search plugin backed by the given engine instance.
    #[must_use]
    pub fn new(engine: Arc<SearchEngine>) -> Self {
        Self { engine }
    }
}

impl Plugin for FileSearchPlugin {
    fn default_keyword(&self) -> Option<&str> {
        None // always participates in search (no keyword prefix)
    }

    fn matches(&self, query: &str) -> bool {
        // Participate when engine is ready and query is non-empty
        !query.trim().is_empty() && self.engine.is_ready()
    }

    fn query(&self, query: &str) -> Vec<PluginResult> {
        let q = query.trim();
        if q.is_empty() {
            return Vec::new();
        }

        let results = self.engine.search(q, 8);
        results.into_iter().map(|r| es_result_to_plugin(r)).collect()
    }

    fn name(&self) -> &str {
        "FileSearch"
    }
}

fn es_result_to_plugin(r: EsSearchResult) -> PluginResult {
    let full_path = r.path.clone();
    let icon = if r.is_directory {
        String::from("folder")
    } else {
        full_path.clone() // GUI will extract icon from file extension/path
    };

    PluginResult {
        title: r.name,
        subtitle: r.path.clone(),
        icon,
        action: Action::Open(full_path),
        score: r.score,
    }
}
