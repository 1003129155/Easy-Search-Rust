// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! File search plugin: bridges the search engine into the plugin system.
//!
//! This is a thin adapter — it takes user input, calls `SearchEngine::search()`,
//! and converts `EsSearchResult` into `PluginResult`.

use std::path::Path;
use std::sync::Arc;

use easysearch_core::{
    Action, ContextAction, ContextData, EsSearchResult, Plugin, PluginResult,
};
use easysearch_engine::{SearchEngine, SearchFilter, SearchQuery};
use quick_launch_store::global_store;

const MAX_FILE_RESULTS: usize = 50;

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

    fn needs_background(&self) -> bool {
        true // MFT index search is I/O-heavy; must run off the UI thread
    }

    fn query(&self, query: &str) -> Vec<PluginResult> {
        let q = query.trim();
        if q.is_empty() {
            return Vec::new();
        }

        // Path-prefix search mode: "\C:\path\" lists all items under that directory.
        if q.starts_with('\\') {
            let prefix = q.trim_start_matches('\\');
            let search_query = SearchQuery::new("", MAX_FILE_RESULTS)
                .with_filter(SearchFilter {
                    path_prefix: Some(prefix.to_string()),
                    ..Default::default()
                });
            let results = self.engine.search_query(&search_query);
            return results.into_iter().map(|r| es_result_to_plugin(r)).collect();
        }

        let results = self.engine.search(q, MAX_FILE_RESULTS);
        results.into_iter().map(|r| es_result_to_plugin(r)).collect()
    }

    fn name(&self) -> &str {
        "FileSearch"
    }
}

fn es_result_to_plugin(r: EsSearchResult) -> PluginResult {
    let full_path = r.path.clone();
    let parent_path = Path::new(&full_path)
        .parent()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default();
    let icon = if r.is_directory {
        String::from("folder")
    } else {
        full_path.clone() // GUI will extract icon from file extension/path
    };
    let context_actions = build_context_actions(&r.name, &full_path, r.is_directory, &parent_path);

    PluginResult {
        title: r.name,
        subtitle: r.path.clone(),
        icon,
        action: Action::Open(full_path),
        score: r.score,
        highlight: r.highlight,
        context_actions,
        context_data: Some(ContextData {
            is_directory: r.is_directory,
            file_path: r.path,
            parent_path,
        }),
    }
}

fn build_context_actions(
    title: &str,
    path: &str,
    is_directory: bool,
    parent_path: &str,
) -> Vec<ContextAction> {
    use easysearch_core::context_labels as cl;

    let is_saved = global_store()
        .lock()
        .map(|store| store.contains(path))
        .unwrap_or(false);

    let mut actions = vec![
        ContextAction {
            label: cl::open_item(is_directory),
            action: Action::Open(path.to_string()),
            shortcut_hint: "Enter".to_string(),
        },
        ContextAction {
            label: cl::open_containing_folder(is_directory),
            action: if is_directory {
                Action::OpenParentFolder(path.to_string())
            } else {
                Action::OpenContainingFolder(path.to_string())
            },
            shortcut_hint: "Ctrl+Enter".to_string(),
        },
    ];

    if !is_directory && !parent_path.is_empty() {
        actions.push(ContextAction {
            label: cl::open_parent_folder(),
            action: Action::OpenParentFolder(path.to_string()),
            shortcut_hint: String::new(),
        });
    }

    actions.push(ContextAction {
        label: cl::copy_path(),
        action: Action::Copy(path.to_string()),
        shortcut_hint: String::new(),
    });
    actions.push(ContextAction {
        label: cl::copy_name(),
        action: Action::Copy(title.to_string()),
        shortcut_hint: String::new(),
    });
    actions.push(ContextAction {
        label: cl::toggle_quick_launch(is_saved),
        action: Action::ToggleQuickLaunch {
            path: path.to_string(),
            title: title.to_string(),
        },
        shortcut_hint: String::new(),
    });
    actions.push(ContextAction {
        label: cl::search_in_folder(),
        action: Action::EnterPathSearch(if is_directory {
            path.to_string()
        } else {
            parent_path.to_string()
        }),
        shortcut_hint: String::new(),
    });
    actions.push(ContextAction {
        label: cl::windows_context_menu(),
        action: Action::ShowFileContextMenu {
            path: path.to_string(),
            is_dir: is_directory,
        },
        shortcut_hint: "Alt+Enter".to_string(),
    });

    actions
}
