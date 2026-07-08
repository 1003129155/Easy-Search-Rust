// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Plugin composition and conversion into search-window display items.

#[cfg(windows)]
use std::sync::Arc;

#[cfg(windows)]
use crate::shared::icon_assets;
#[cfg(windows)]
use easysearch_core::Router;
#[cfg(windows)]
use quick_launch_store::global_store;

#[cfg(windows)]
use super::renderer::DisplayItem;

/// Convert a PluginResult batch into DisplayItems with shortcut assignment
/// and history frequency boost applied.
#[cfg(windows)]
pub(crate) fn plugin_results_to_display(
    plugin_results: Vec<easysearch_core::PluginResult>,
    history: &super::history::History,
) -> Vec<DisplayItem> {
    plugin_results
        .into_iter()
        .enumerate()
        .map(|(i, r)| {
            let is_directory = r
                .context_data
                .as_ref()
                .map(|data| data.is_directory)
                .unwrap_or(false);
            let icon_path = resolve_display_icon_ref(&r.icon, &r.action, is_directory);
            let action_key = action_to_history_key_static(&r.action);
            let boosted_score = r.score + history.boost_score(&action_key);
            DisplayItem {
                title: r.title,
                subtitle: r.subtitle,
                icon: r.icon,
                shortcut: if i < 9 {
                    format!("Alt+{}", i + 1)
                } else {
                    String::new()
                },
                action: r.action,
                context_actions: r.context_actions,
                context_data: r.context_data.clone(),
                icon_path,
                is_directory,
                highlight: r.highlight,
                score: boosted_score,
            }
        })
        .collect()
}

#[cfg(windows)]
pub(crate) fn resolve_display_icon_ref(
    icon: &str,
    action: &easysearch_core::Action,
    is_directory: bool,
) -> Option<String> {
    if icon_assets::is_named_icon(icon) || icon_assets::is_filesystem_path(icon) {
        return Some(icon.to_string());
    }

    match action {
        easysearch_core::Action::Open(path)
        | easysearch_core::Action::OpenAsAdmin(path)
        | easysearch_core::Action::OpenContainingFolder(path)
        | easysearch_core::Action::OpenParentFolder(path)
            if icon_assets::is_filesystem_path(path) =>
        {
            Some(path.clone())
        }
        easysearch_core::Action::ShowFileContextMenu { path, .. }
            if icon_assets::is_filesystem_path(path) =>
        {
            Some(path.clone())
        }
        _ if is_directory => Some(String::from("folder")),
        _ => None,
    }
}

/// Build the plugin router with all built-in plugins registered.
/// If an engine is provided, FileSearchPlugin is also registered.
#[cfg(windows)]
pub(crate) fn build_plugin_router(engine: Option<Arc<easysearch_engine::SearchEngine>>) -> Router {
    let mut router = Router::new();
    router.register(Box::new(plugin_bookmark::BookmarkPlugin::new()));
    router.register(Box::new(plugin_program::ProgramPlugin::new()));
    router.register(Box::new(plugin_sys_cmd::SysCmdPlugin::new()));
    router.register(Box::new(plugin_win_settings::WinSettingsPlugin::new()));
    router.register(Box::new(plugin_quick_launch::QuickLaunchPlugin::new()));

    // FileSearchPlugin: the file search engine as a normal background plugin.
    if let Some(eng) = engine {
        router.register(Box::new(plugin_file_search::FileSearchPlugin::new(eng)));
    }

    // Plugin Indicator shows keyword hints and should run after the main plugins.
    let locale = crate::SHARED_SETTINGS
        .get()
        .and_then(|settings| settings.read().ok().map(|s| s.language.clone()))
        .filter(|locale| !locale.is_empty())
        .unwrap_or_else(crate::i18n::engine::I18nEngine::detect_system_locale);
    let mut indicator = plugin_indicator::PluginIndicatorPlugin::new();
    indicator.refresh(&router.plugin_infos_for_locale(&locale));
    router.register(Box::new(indicator));

    router
}

/// Build the combined home screen when the search box is empty:
/// top-1 recent item, plugin keyword hints, then remaining recent items.
#[cfg(windows)]
pub(crate) fn build_home_screen(
    history: &super::history::History,
    router: &Router,
    locale: &str,
) -> Vec<DisplayItem> {
    const MAX_HISTORY: usize = 10;

    let mut items = Vec::new();
    let recent = history.top_recent(MAX_HISTORY);

    if let Some(first) = recent.first() {
        items.push(recent_to_display(first));
    }

    items.extend(build_home_hints(router, locale));

    for r in recent.iter().skip(1) {
        items.push(recent_to_display(r));
    }

    items
}

/// Convert an action to a history key (non-mut version for search thread).
#[cfg(windows)]
pub(crate) fn action_to_history_key_static(action: &easysearch_core::Action) -> String {
    match action {
        easysearch_core::Action::Open(path) => format!("open:{path}"),
        easysearch_core::Action::OpenAsAdmin(path) => format!("open-admin:{path}"),
        easysearch_core::Action::OpenContainingFolder(path) => format!("open-folder:{path}"),
        easysearch_core::Action::OpenParentFolder(path) => format!("open-parent:{path}"),
        easysearch_core::Action::EnterPathSearch(path) => format!("path-search:{path}"),
        easysearch_core::Action::Copy(text) => format!("copy:{}", &text[..text.len().min(50)]),
        easysearch_core::Action::RunCommand { cmd, .. } => format!("run:{cmd}"),
        easysearch_core::Action::SystemCommand(cmd) => format!("sys:{cmd:?}"),
        easysearch_core::Action::DaemonSearch(q) => format!("search:{q}"),
        easysearch_core::Action::ToggleQuickLaunch { path, .. } => format!("quick-launch:{path}"),
        easysearch_core::Action::ShowFileContextMenu { path, .. } => {
            format!("windows-context:{path}")
        }
        easysearch_core::Action::None => String::new(),
    }
}

/// Build the "home screen" plugin hint list shown when the search box is empty.
#[cfg(windows)]
fn build_home_hints(router: &Router, locale: &str) -> Vec<DisplayItem> {
    router
        .plugin_infos_for_locale(locale)
        .into_iter()
        .filter(|info| {
            info.enabled
                && info
                    .keyword
                    .as_deref()
                    .map_or(false, |k| !k.trim().is_empty())
        })
        .map(|info| {
            let keyword = info.keyword.clone().unwrap_or_default();
            let desc = if info.description.is_empty() {
                info.name.clone()
            } else {
                format!("{} - {}", info.name, info.description)
            };
            DisplayItem {
                title: keyword.trim().to_string(),
                subtitle: desc,
                icon: info.icon.clone(),
                shortcut: String::new(),
                action: easysearch_core::Action::None,
                context_actions: Vec::new(),
                context_data: None,
                icon_path: resolve_display_icon_ref(
                    &info.icon,
                    &easysearch_core::Action::None,
                    false,
                ),
                is_directory: false,
                highlight: Vec::new(),
                score: 100,
            }
        })
        .collect()
}

/// Convert a [`RecentItem`] into a [`DisplayItem`] for the home screen.
#[cfg(windows)]
fn recent_to_display(r: &super::history::RecentItem) -> DisplayItem {
    let action = history_key_to_action(&r.action_key);
    let icon_path = resolve_display_icon_ref(&r.icon, &action, r.is_directory);
    let (context_actions, context_data) =
        build_recent_context(&r.action_key, &r.title, r.is_directory);

    DisplayItem {
        title: r.title.clone(),
        subtitle: r.subtitle.clone(),
        icon: r.icon.clone(),
        shortcut: String::new(),
        action,
        context_actions,
        context_data,
        icon_path,
        is_directory: r.is_directory,
        highlight: Vec::new(),
        score: 0,
    }
}

/// Build context actions and context data for a recent history item.
#[cfg(windows)]
fn build_recent_context(
    action_key: &str,
    title: &str,
    is_directory: bool,
) -> (
    Vec<easysearch_core::ContextAction>,
    Option<easysearch_core::ContextData>,
) {
    use easysearch_core::context_labels as cl;
    use easysearch_core::{Action, ContextAction, ContextData};

    let file_path = if let Some(path) = action_key.strip_prefix("open:") {
        Some(path.to_string())
    } else if let Some(path) = action_key.strip_prefix("open-admin:") {
        Some(path.to_string())
    } else if let Some(path) = action_key.strip_prefix("open-folder:") {
        Some(path.to_string())
    } else if let Some(path) = action_key.strip_prefix("open-parent:") {
        Some(path.to_string())
    } else {
        None
    };

    let Some(path) = file_path else {
        return (Vec::new(), None);
    };

    let parent_path = std::path::Path::new(&path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let is_saved = global_store()
        .lock()
        .map(|store| store.contains(&path))
        .unwrap_or(false);

    let mut actions = vec![
        ContextAction {
            label: cl::open_item(is_directory),
            action: Action::Open(path.clone()),
            shortcut_hint: "Enter".to_string(),
        },
        ContextAction {
            label: cl::open_containing_folder(is_directory),
            action: if is_directory {
                Action::OpenParentFolder(path.clone())
            } else {
                Action::OpenContainingFolder(path.clone())
            },
            shortcut_hint: "Ctrl+Enter".to_string(),
        },
    ];

    if !is_directory && !parent_path.is_empty() {
        actions.push(ContextAction {
            label: cl::open_parent_folder(),
            action: Action::OpenParentFolder(path.clone()),
            shortcut_hint: String::new(),
        });
    }

    actions.push(ContextAction {
        label: cl::copy_path(),
        action: Action::Copy(path.clone()),
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
            path: path.clone(),
            title: title.to_string(),
        },
        shortcut_hint: String::new(),
    });
    actions.push(ContextAction {
        label: cl::search_in_folder(),
        action: Action::EnterPathSearch(if is_directory {
            path.clone()
        } else {
            parent_path.clone()
        }),
        shortcut_hint: String::new(),
    });
    actions.push(ContextAction {
        label: cl::windows_context_menu(),
        action: Action::ShowFileContextMenu {
            path: path.clone(),
            is_dir: is_directory,
        },
        shortcut_hint: "Alt+Enter".to_string(),
    });

    let context_data = ContextData {
        is_directory,
        file_path: path,
        parent_path,
    };

    (actions, Some(context_data))
}

/// Inverse of [`action_to_history_key_static`]: parse a history key into an action.
#[cfg(windows)]
fn history_key_to_action(key: &str) -> easysearch_core::Action {
    if let Some(path) = key.strip_prefix("open:") {
        return easysearch_core::Action::Open(path.to_string());
    }
    if let Some(path) = key.strip_prefix("open-admin:") {
        return easysearch_core::Action::OpenAsAdmin(path.to_string());
    }
    if let Some(path) = key.strip_prefix("open-folder:") {
        return easysearch_core::Action::OpenContainingFolder(path.to_string());
    }
    if let Some(path) = key.strip_prefix("open-parent:") {
        return easysearch_core::Action::OpenParentFolder(path.to_string());
    }
    easysearch_core::Action::None
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use easysearch_core::Action;

    #[test]
    fn history_key_encodes_open_action() {
        let key = action_to_history_key_static(&Action::Open("C:\\a.txt".into()));
        assert_eq!(key, "open:C:\\a.txt");
    }

    #[test]
    fn history_key_encodes_all_path_variants() {
        assert_eq!(
            action_to_history_key_static(&Action::OpenAsAdmin("p".into())),
            "open-admin:p"
        );
        assert_eq!(
            action_to_history_key_static(&Action::OpenContainingFolder("p".into())),
            "open-folder:p"
        );
        assert_eq!(
            action_to_history_key_static(&Action::OpenParentFolder("p".into())),
            "open-parent:p"
        );
        assert_eq!(
            action_to_history_key_static(&Action::EnterPathSearch("p".into())),
            "path-search:p"
        );
        assert_eq!(
            action_to_history_key_static(&Action::ToggleQuickLaunch {
                path: "p".into(),
                title: "t".into()
            }),
            "quick-launch:p"
        );
    }

    #[test]
    fn history_key_none_is_empty() {
        assert_eq!(action_to_history_key_static(&Action::None), "");
    }

    #[test]
    fn history_key_copy_truncates_long_text() {
        let long = "x".repeat(100);
        let key = action_to_history_key_static(&Action::Copy(long));
        // "copy:" prefix + at most 50 chars of text
        assert_eq!(key, format!("copy:{}", "x".repeat(50)));
    }

    #[test]
    fn history_key_copy_short_text_intact() {
        let key = action_to_history_key_static(&Action::Copy("hi".into()));
        assert_eq!(key, "copy:hi");
    }

    #[test]
    fn key_to_action_roundtrips_open_family() {
        // These four action variants must survive an encode → decode round-trip
        // because the home-screen recent panel reconstructs actions from keys.
        let cases = [
            Action::Open("C:\\file.txt".into()),
            Action::OpenAsAdmin("C:\\file.txt".into()),
            Action::OpenContainingFolder("C:\\dir".into()),
            Action::OpenParentFolder("C:\\dir".into()),
        ];
        for action in cases {
            let key = action_to_history_key_static(&action);
            let decoded = history_key_to_action(&key);
            // Compare via re-encoding since Action has no PartialEq.
            assert_eq!(action_to_history_key_static(&decoded), key);
        }
    }

    #[test]
    fn key_to_action_unknown_prefix_is_none() {
        let decoded = history_key_to_action("search:hello");
        assert!(matches!(decoded, Action::None));
    }

    #[test]
    fn key_to_action_empty_is_none() {
        assert!(matches!(history_key_to_action(""), Action::None));
    }
}
