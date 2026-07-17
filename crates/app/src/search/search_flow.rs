// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Search orchestration: input handling, debounce, deferred merging, and resize.

#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::*;

#[cfg(windows)]
use super::app_state::{AppState, DeferredQuery, ViewMode};
#[cfg(windows)]
use super::layout;
#[cfg(windows)]
use super::messages::*;
#[cfg(windows)]
use super::plugin_bridge::{build_home_screen, plugin_results_to_display};
#[cfg(windows)]
use super::window::sync_active_items;

/// Called whenever the search input text changes.
/// Resets search state, cancels pending queries, and either shows the home
/// screen or kicks off a debounced search.
#[cfg(windows)]
pub(super) fn on_input_changed(app: &mut AppState) {
    let query = app.input.text().to_string();
    app.view_mode = ViewMode::Results;
    app.selected_index = 0;
    app.result_selected_index = 0;
    app.context_items.clear();
    app.context_selected_index = 0;
    app.context_source_index = None;
    app.current_search_seq += 1;
    app.input_focused = true;
    app.cursor_moved_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    // Clear async preview
    app.preview = None;
    app.preview_seq += 1;

    // Cancel any pending deferred query
    app.deferred_query = None;
    unsafe {
        let _ = KillTimer(Some(app.hwnd), DEFERRED_POLL_TIMER_ID);
        let _ = KillTimer(Some(app.hwnd), SEARCH_DEBOUNCE_TIMER_ID);
        let _ = KillTimer(Some(app.hwnd), BUSY_ANIM_TIMER_ID);
    }

    if query.trim().is_empty() {
        app.plugin_router.reset_search_sessions();
        // Home screen: top-1 recent → plugin hints → remaining recent (max 10).
        app.plugin_items.clear();
        app.result_items =
            build_home_screen(&app.history, &app.plugin_router, app.i18n.current_locale());
        app.anim_frame = ANIM_TOTAL_FRAMES;
        app.search_active = false;
    } else {
        app.search_active = true;
        app.anim_frame = 0;
        unsafe {
            let _ = SetTimer(
                Some(app.hwnd),
                SEARCH_DEBOUNCE_TIMER_ID,
                SEARCH_DEBOUNCE_MS,
                None,
            );
        }
    }

    sync_active_items(app);

    // Resize window based on results
    resize_for_results(app);
}

/// Execute the actual plugin query after the debounce timer fires.
/// Runs immediate plugins synchronously and starts background (deferred) plugins.
#[cfg(windows)]
pub(super) fn run_debounced_search(app: &mut AppState) {
    let query = app.input.text().to_string();
    if query.trim().is_empty() {
        app.search_active = false;
        return;
    }

    let (immediate_results, keyword_matched) = app.plugin_router.query_immediate(&query);
    app.plugin_items = plugin_results_to_display(immediate_results, &app.history);
    app.result_items = app.plugin_items.clone();
    app.anim_frame = 0;
    sync_active_items(app);
    resize_for_results(app);

    let current_seq = app.current_search_seq;
    if let Some((deferred_rx, cancel)) = app.plugin_router.query_background(&query, keyword_matched)
    {
        app.deferred_query = Some(DeferredQuery {
            rx: deferred_rx,
            seq_id: current_seq,
            cancel,
        });
        unsafe {
            let _ = SetTimer(
                Some(app.hwnd),
                DEFERRED_POLL_TIMER_ID,
                DEFERRED_POLL_MS,
                None,
            );
            let _ = SetTimer(Some(app.hwnd), BUSY_ANIM_TIMER_ID, ANIM_FRAME_MS, None);
        }
    } else {
        app.search_active = false;
    }
}

/// Resize window to fit the current number of results.
#[cfg(windows)]
pub(super) fn resize_for_results(app: &mut AppState) {
    let has_preview = app.preview.is_some() && app.view_mode == ViewMode::ContextActions;
    let height = layout::window_height_with_preview_scaled(app.items.len(), has_preview, app.hwnd);
    let width = layout::window_width_scaled(app.hwnd);

    if app.last_window_size == (width, height) {
        return;
    }
    app.last_window_size = (width, height);

    unsafe {
        let _ = SetWindowPos(
            app.hwnd,
            None,
            0,
            0,
            width,
            height,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
    // Manually resize render target since WM_SIZE handler may be skipped due to re-entrancy
    app.renderer.resize(width as u32, height as u32);
}

/// Poll the deferred (background) query receiver and merge results if ready.
///
/// Returns `true` if the timer should be stopped (results received, channel
/// disconnected, or no deferred query pending). Returns `false` if still waiting.
#[cfg(windows)]
pub(super) fn poll_deferred_results(app: &mut AppState) -> bool {
    use super::plugin_bridge::action_to_history_key_static;
    use super::renderer::DisplayItem;

    match &mut app.deferred_query {
        Some(dq) => match dq.rx.try_recv() {
            Ok(plugin_results) => {
                // Only accept results matching current seq_id
                if dq.seq_id == app.current_search_seq {
                    let query = app.input.text().to_string();
                    let new_items = plugin_results_to_display(plugin_results, &app.history);

                    // Apply history boost to immediate plugin items too
                    let mut immediate_items: Vec<DisplayItem> = app
                        .plugin_items
                        .iter()
                        .map(|item| {
                            let mut item = item.clone();
                            let key = action_to_history_key_static(&item.action);
                            item.score = item.score.saturating_add(app.history.boost_score(&key));
                            item
                        })
                        .collect();

                    // Merge all results
                    immediate_items.extend(new_items);

                    // Deduplicate by (title, subtitle): sort+dedup
                    // First sort by (title, subtitle) to group duplicates,
                    // within same group sort by score descending so highest is kept.
                    immediate_items.sort_by(|a, b| {
                        a.title
                            .cmp(&b.title)
                            .then(a.subtitle.cmp(&b.subtitle))
                            .then(b.score.cmp(&a.score))
                    });
                    // Adjacent dedup keeps the first (highest score) of each group
                    immediate_items.dedup_by(|b, a| a.title == b.title && a.subtitle == b.subtitle);
                    // Final sort by score descending for display order
                    immediate_items.sort_by(|a, b| b.score.cmp(&a.score));

                    // Move pinned items to the top
                    let mut pinned = Vec::new();
                    let mut unpinned = Vec::new();
                    for item in immediate_items {
                        let key = action_to_history_key_static(&item.action);
                        if app.history.is_pinned(&query, &key) {
                            pinned.push(item);
                        } else {
                            unpinned.push(item);
                        }
                    }
                    pinned.sort_by_key(|item| {
                        let key = action_to_history_key_static(&item.action);
                        app.history
                            .pinned_position(&query, &key)
                            .unwrap_or(usize::MAX)
                    });
                    pinned.extend(unpinned);
                    let mut all_items = pinned;

                    // Reassign shortcut labels based on final position
                    for (i, item) in all_items.iter_mut().enumerate() {
                        item.shortcut = if i < 9 {
                            format!("Alt+{}", i + 1)
                        } else {
                            String::new()
                        };
                    }

                    app.result_items = all_items;
                    if app.view_mode == ViewMode::Results {
                        sync_active_items(app);
                    }
                    app.search_active = false;
                    resize_for_results(app);
                }
                true // done, stop timer
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                false // still waiting
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                true // sender dropped, stop
            }
        },
        None => true, // no deferred query, stop timer
    }
}
