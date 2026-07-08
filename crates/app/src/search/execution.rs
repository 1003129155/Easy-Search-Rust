// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Action execution orchestration.
//!
//! This module owns the "what happens when the user activates an item" logic:
//! extracting the selected action under a short borrow, recording history,
//! hiding the window, and dispatching the action (open file, toggle quick
//! launch, enter path search, show native context menu, etc.).
//!
//! It deliberately avoids holding the `AppState` borrow across Win32 calls
//! (`ShellExecute`, `ShowWindow`) because those can re-enter the window
//! procedure and trigger `RefCell` borrow panics.

#[cfg(windows)]
use super::app_state::{self, ViewMode};
#[cfg(windows)]
use super::plugin_bridge::action_to_history_key_static;
#[cfg(windows)]
use super::window::{open_context_actions, show_native_context_menu_safe, sync_active_items};
#[cfg(windows)]
use quick_launch_store::global_store;

/// Execute the currently selected item — safe version that avoids RefCell re-entrancy.
/// Extracts the action with a brief borrow, then releases it before calling Win32 APIs.
#[cfg(windows)]
pub(super) fn execute_selected_safe() {
    // Step 1: Extract the action and record history while briefly borrowing
    let item = app_state::with_app_mut(|app| {
        if app.items.is_empty() {
            return None;
        }
        let idx = app.selected_index.min(app.items.len() - 1);
        let item = app.items[idx].clone();

        // Record usage with full metadata for the home-screen recent panel.
        let history_key = action_to_history_key_static(&item.action);
        let icon = item.icon_path.as_deref().unwrap_or(&item.icon);
        app.history.record_full(
            &history_key,
            &item.title,
            &item.subtitle,
            icon,
            item.is_directory,
        );
        app.history.save();

        Some(item)
    })
    .flatten();

    let Some(item) = item else {
        return;
    };

    // Step 2: Hide window (outside borrow — ShowWindow can trigger re-entrant messages)

    // Step 3: Execute the action (outside borrow — ShellExecute etc.)
    execute_action_safe(item.action, item.title, item.context_data);
}

/// Ctrl+Enter handler: for folders → open in Explorer; for files → open containing folder.
#[cfg(windows)]
pub(super) fn open_folder_or_containing_safe() {
    let info = app_state::with_app_ref(|app| {
        if app.items.is_empty() {
            return None;
        }
        let idx = app.selected_index.min(app.items.len() - 1);
        let is_dir = app.items[idx]
            .context_data
            .as_ref()
            .map(|data| data.is_directory)
            .unwrap_or(false);
        let path = app.items[idx]
            .context_data
            .as_ref()
            .map(|data| data.file_path.clone())
            .or_else(|| match &app.items[idx].action {
                easysearch_core::Action::Open(p)
                | easysearch_core::Action::OpenAsAdmin(p)
                | easysearch_core::Action::EnterPathSearch(p) => Some(p.clone()),
                _ => None,
            });
        path.map(|p| (p, is_dir))
    })
    .flatten();

    let Some((path, is_dir)) = info else {
        return;
    };

    super::visibility::hide_window();

    if is_dir {
        // Folder: open it directly in Explorer
        super::action::execute(&easysearch_core::Action::Open(path));
    } else {
        // File: open its containing folder with the file selected
        super::fs_actions::open_containing_folder(&path);
    }
}

/// Dispatch an already-extracted action. Must be called *outside* an `AppState`
/// borrow because it re-borrows internally and performs Win32 side effects.
#[cfg(windows)]
pub(super) fn execute_action_safe(
    action: easysearch_core::Action,
    _title: String,
    context_data: Option<easysearch_core::ContextData>,
) {
    match action {
        easysearch_core::Action::DaemonSearch(query) => {
            app_state::with_app_mut(|app| {
                app.input.set_text(&query);
                app.input.move_end(false);
                super::search_flow::on_input_changed(app);
            });
        }
        easysearch_core::Action::EnterPathSearch(path) => {
            // Normalize: ensure trailing backslash so path_prefix filter
            // matches only children of this directory (not sibling prefixes).
            let query = if path.ends_with('\\') {
                format!("\\{path}")
            } else {
                format!("\\{path}\\")
            };
            app_state::with_app_mut(|app| {
                app.input.set_text(&query);
                app.input.move_end(false);
                super::search_flow::on_input_changed(app);
            });
        }
        easysearch_core::Action::ToggleQuickLaunch {
            path,
            title: item_title,
        } => {
            let is_dir = context_data
                .as_ref()
                .map(|data| data.is_directory)
                .unwrap_or_else(|| super::fs_actions::is_directory(&path));
            {
                let mut store = global_store().lock().unwrap_or_else(|err| err.into_inner());
                let _ = store.toggle(&path, &item_title, is_dir);
                let _ = store.save();
            }

            app_state::with_app_mut(|app| {
                let was_context = app.view_mode == ViewMode::ContextActions;
                let source_index = app.context_source_index;
                super::search_flow::on_input_changed(app);
                if was_context {
                    if let Some(index) = source_index {
                        app.result_selected_index =
                            index.min(app.result_items.len().saturating_sub(1));
                        sync_active_items(app);
                        let _ = open_context_actions(app);
                    }
                }
            });
        }
        easysearch_core::Action::ShowFileContextMenu { .. } => {
            show_native_context_menu_safe();
        }
        other => {
            let should_hide = !matches!(
                other,
                easysearch_core::Action::None | easysearch_core::Action::ShowFileContextMenu { .. }
            );
            if should_hide {
                super::visibility::hide_window();
            }
            super::action::execute(&other);
        }
    }
}
