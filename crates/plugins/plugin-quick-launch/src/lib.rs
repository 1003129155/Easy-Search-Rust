// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Quick Launch plugin backed by the shared Quick Launch store.

use std::path::Path;

use easysearch_core::{Action, ContextAction, ContextData, Plugin, PluginResult};
use quick_launch_store::{QuickLaunchItem, global_store};

pub struct QuickLaunchPlugin {
    max_results: usize,
}

impl QuickLaunchPlugin {
    #[must_use]
    pub fn new() -> Self {
        Self { max_results: 8 }
    }
}

impl Default for QuickLaunchPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for QuickLaunchPlugin {
    fn default_keyword(&self) -> Option<&str> {
        Some("q")
    }

    fn query(&self, query: &str) -> Vec<PluginResult> {
        let store = global_store().lock().unwrap_or_else(|err| err.into_inner());
        let items: Vec<&QuickLaunchItem> = if query.trim().is_empty() {
            store.all().iter().take(self.max_results).collect()
        } else {
            store
                .search(query)
                .into_iter()
                .take(self.max_results)
                .collect()
        };

        items
            .into_iter()
            .map(|item| {
                let parent_path = Path::new(&item.path)
                    .parent()
                    .map(|path| path.to_string_lossy().to_string())
                    .unwrap_or_default();
                PluginResult {
                    title: item.title.clone(),
                    subtitle: item.path.clone(),
                    icon: if item.is_directory {
                        "folder".to_string()
                    } else {
                        item.path.clone()
                    },
                    action: Action::Open(item.path.clone()),
                    score: 800,
                    highlight: Vec::new(),
                    context_actions: build_context_actions(
                        &item.title,
                        &item.path,
                        item.is_directory,
                        &parent_path,
                    ),
                    context_data: Some(ContextData {
                        is_directory: item.is_directory,
                        file_path: item.path.clone(),
                        parent_path,
                    }),
                }
            })
            .collect()
    }

    fn name(&self) -> &str {
        "QuickLaunch"
    }

    fn display_name(&self, locale: &str) -> String {
        match locale_prefix(locale) {
            "zh" => "快速启动".to_string(),
            "ja" => "クイック起動".to_string(),
            _ => "Quick Launch".to_string(),
        }
    }

    fn description(&self) -> &str {
        "Browse saved Quick Launch files and folders"
    }

    fn description_for_locale(&self, locale: &str) -> String {
        match locale_prefix(locale) {
            "zh" => "浏览已保存到快速启动的文件和文件夹".to_string(),
            "ja" => "クイック起動に保存したファイルとフォルダーを参照します".to_string(),
            _ => "Browse saved Quick Launch files and folders".to_string(),
        }
    }

    fn icon(&self) -> &str {
        "star"
    }
}

fn locale_prefix(locale: &str) -> &str {
    locale.split('-').next().unwrap_or(locale)
}

fn build_context_actions(
    title: &str,
    path: &str,
    is_directory: bool,
    parent_path: &str,
) -> Vec<ContextAction> {
    use easysearch_core::context_labels as cl;

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
        label: cl::toggle_quick_launch(true),
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
