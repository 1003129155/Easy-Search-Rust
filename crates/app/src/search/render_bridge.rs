// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Rendering orchestration for the search window.
//!
//! The renderer itself stays in `renderer.rs`; this module translates current
//! `AppState` into renderer parameters, starts icon-loading animation timers,
//! and forwards async icon load completions back to the window thread.

#[cfg(windows)]
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, SetTimer};

#[cfg(windows)]
use super::app_state;
#[cfg(windows)]
use super::app_state::ViewMode;
#[cfg(windows)]
use super::layout;
#[cfg(windows)]
use super::messages::{
    IconReadyPayload, ANIM_FRAME_MS, ANIM_TOTAL_FRAMES, BUSY_ANIM_TIMER_ID, WM_ICON_READY,
};

/// Trigger a re-render.
#[cfg(windows)]
pub(super) fn do_render() {
    app_state::with_app_mut(|app| {
        // Determine placeholder text based on index state
        let placeholder = if let Some(ref err) = app.index_error {
            err.as_str()
        } else if app.index_ready {
            app.i18n.get("placeholder_ready")
        } else if !app.index_status.is_empty() {
            app.index_status.as_str()
        } else {
            app.i18n.get("placeholder_indexing")
        };
        let anim_progress = app.anim_frame as f32 / ANIM_TOTAL_FRAMES as f32;

        // Show preview only in context actions mode
        let preview_param = if app.view_mode == ViewMode::ContextActions {
            if let Some(ref preview) = app.preview {
                if !app.items.is_empty() {
                    let results_height = layout::window_height(app.items.len());
                    Some((preview, results_height))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        app.renderer.render(
            app.input.text(),
            app.input.cursor(),
            app.input.selection_range(),
            app.input.has_selection(),
            &app.items,
            app.selected_index,
            placeholder,
            &mut app.icon_cache,
            anim_progress,
            app.search_active,
            preview_param,
        );

        let icon_requests = app.icon_cache.take_load_requests();
        if !icon_requests.is_empty() {
            unsafe {
                let _ = SetTimer(Some(app.hwnd), BUSY_ANIM_TIMER_ID, ANIM_FRAME_MS, None);
            }
            spawn_icon_loads(app.hwnd, icon_requests);
        }
    });
}

#[cfg(windows)]
fn spawn_icon_loads(hwnd: HWND, requests: Vec<super::icon::IconLoadRequest>) {
    let hwnd_raw = hwnd.0 as usize;
    for request in requests {
        std::thread::Builder::new()
            .name("icon-loader".to_string())
            .spawn(move || {
                let pixels = super::icon::load_icon_pixels(&request);
                let payload = Box::new(IconReadyPayload { request, pixels });
                unsafe {
                    let h = HWND(hwnd_raw as *mut _);
                    let _ = PostMessageW(
                        Some(h),
                        WM_ICON_READY,
                        WPARAM(0),
                        LPARAM(Box::into_raw(payload) as isize),
                    );
                }
            })
            .ok();
    }
}
