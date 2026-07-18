// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Window visibility management — show, hide, toggle, and foreground helpers.
//!
//! Extracted from `window.rs` to isolate Win32 visibility coordination from
//! message dispatch. All functions access state through `app_state::with_app_*`.

#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
#[cfg(windows)]
use windows::Win32::UI::Input::Ime::{ImmGetContext, ImmReleaseContext};
#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::*;

#[cfg(windows)]
use super::app_state::{self, ViewMode};
#[cfg(windows)]
use super::layout;
#[cfg(windows)]
use super::messages::*;
#[cfg(windows)]
use super::plugin_bridge::build_home_screen;

/// Toggle window visibility — if visible, hide; if hidden, show.
#[cfg(windows)]
pub(super) fn toggle_visibility() {
    easysearch_core::log_debug!("toggle_visibility called");

    // Read current visibility state without holding borrow during Win32 calls
    let (is_visible, hwnd) =
        app_state::with_app_ref(|app| (app.visible, app.hwnd)).unwrap_or((false, HWND::default()));

    if is_visible {
        easysearch_core::log_debug!("  -> hiding window");
        hide_window();
    } else {
        easysearch_core::log_debug!("  -> showing window");
        // Show: Win32 calls in show_window can trigger re-entrant messages,
        // so we must NOT hold the borrow_mut during those calls.
        show_window_safe(hwnd);
    }
}

/// Show the window — safe version that avoids RefCell re-entrancy.
/// Win32 calls (SetForegroundWindow, SetFocus) can trigger re-entrant messages
/// (IME initialization, WM_PAINT), so we must NOT hold the RefCell borrow.
#[cfg(windows)]
pub(super) fn show_window_safe(hwnd: HWND) {
    easysearch_core::log_debug!("show_window_safe: start");

    // Populate the home-screen plugin hints if the box is empty, and get the
    // resulting item count (used to size the window below).
    let item_count = app_state::with_app_mut(|app| {
        if app.input.text().trim().is_empty() {
            app.result_items =
                build_home_screen(&app.history, &app.plugin_router, app.i18n.current_locale());
            app.plugin_items.clear();
            app.result_selected_index = 0;
            super::window::sync_active_items(app);
            app.anim_frame = ANIM_TOTAL_FRAMES;
        }
        // Set visible=true BEFORE Win32 calls so that any WM_ACTIVATE(WA_INACTIVE)
        // triggered during show sequence will correctly see visible==true and hide.
        app.visible = true;
        app.items.len()
    })
    .unwrap_or(0);

    unsafe {
        // Multi-monitor support: show on the active monitor
        unsafe extern "system" {
            fn GetCursorPos(lp_point: *mut windows::Win32::Foundation::POINT) -> i32;
        }

        let mut cursor_pos = windows::Win32::Foundation::POINT { x: 0, y: 0 };
        GetCursorPos(&mut cursor_pos);
        easysearch_core::log_debug!(
            "show_window_safe: cursor at ({}, {})",
            cursor_pos.x,
            cursor_pos.y
        );

        let monitor = windows::Win32::Graphics::Gdi::MonitorFromPoint(
            cursor_pos,
            windows::Win32::Graphics::Gdi::MONITOR_DEFAULTTOPRIMARY,
        );

        let mut mi = windows::Win32::Graphics::Gdi::MONITORINFO {
            cbSize: std::mem::size_of::<windows::Win32::Graphics::Gdi::MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = windows::Win32::Graphics::Gdi::GetMonitorInfoW(monitor, &mut mi);

        let work = mi.rcWork;
        let mon_width = work.right - work.left;
        let mon_height = work.bottom - work.top;

        let width = layout::window_width_scaled(hwnd);
        let height = layout::window_height_scaled(item_count, hwnd);
        let x = work.left + (mon_width - width) / 2;
        let y = work.top + mon_height / 4;

        easysearch_core::log_debug!(
            "show_window_safe: SetWindowPos x={x} y={y} w={width} h={height}"
        );
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE,
        );

        // Pre-render content BEFORE showing the window so the first visible
        // frame already has a fully-painted client area (avoids the "border
        // appears before content" flash).
        easysearch_core::log_debug!("show_window_safe: pre-render before ShowWindow");
        super::render_bridge::do_render();

        easysearch_core::log_debug!("show_window_safe: show and force foreground focus");
        show_window_and_activate(hwnd);

        // Ensure IME context is properly associated with the focused window.
        // Some IMEs lose context when a WS_POPUP window regains focus.
        {
            let himc = ImmGetContext(hwnd);
            if !himc.is_invalid() {
                let _ = ImmReleaseContext(hwnd, himc);
            }
        }
    }

    easysearch_core::log_debug!("show_window_safe: done, visible=true");
}

/// Show the popup and activate it while the foreground input queue is
/// attached. `ShowWindow(SW_SHOW)` is deliberately performed inside the
/// attachment window so its synchronous activation messages use the same
/// input queue as the foreground application.
#[cfg(windows)]
unsafe fn show_window_and_activate(hwnd: HWND) {
    unsafe {
        let foreground = GetForegroundWindow();
        let our_tid = GetCurrentThreadId();
        let foreground_tid = if foreground.0 == std::ptr::null_mut() || foreground == hwnd {
            0
        } else {
            GetWindowThreadProcessId(foreground, None)
        };
        let should_attach = foreground_tid != 0 && foreground_tid != our_tid;

        if should_attach {
            let _ = AttachThreadInput(our_tid, foreground_tid, true);
        }

        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(Some(hwnd));

        if should_attach {
            let _ = AttachThreadInput(our_tid, foreground_tid, false);
        }

        easysearch_core::log_debug!(
            "show_window_and_activate: foreground={}",
            GetForegroundWindow() == hwnd
        );
    }
}

/// Force a window to the foreground, working around Windows' restrictions.
///
/// `SetForegroundWindow` can silently fail if the calling thread doesn't hold
/// the "foreground lock." Temporarily attaching our input queue to the current
/// foreground thread lets us activate and focus the popup before detaching.
#[cfg(windows)]
unsafe fn force_foreground(hwnd: HWND) {
    unsafe {
        let foreground = GetForegroundWindow();
        let our_tid = GetCurrentThreadId();
        let foreground_tid = if foreground.0 == std::ptr::null_mut() || foreground == hwnd {
            0
        } else {
            GetWindowThreadProcessId(foreground, None)
        };
        let should_attach = foreground_tid != 0 && foreground_tid != our_tid;

        if should_attach {
            let _ = AttachThreadInput(our_tid, foreground_tid, true);
        }

        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(Some(hwnd));

        if should_attach {
            let _ = AttachThreadInput(our_tid, foreground_tid, false);
        }

        easysearch_core::log_debug!(
            "force_foreground: foreground={}",
            GetForegroundWindow() == hwnd
        );
    }
}

/// Queue a deactivation check after the current Win32 message stack unwinds.
///
/// Activation APIs send messages synchronously, often while `AppState` is
/// borrowed. Deferring the check prevents a transient borrow failure from
/// permanently dropping the only notification that the window lost focus.
#[cfg(windows)]
pub(super) fn queue_deactivation_check(hwnd: HWND) {
    unsafe {
        let _ = PostMessageW(
            Some(hwnd),
            WM_DEACTIVATE_CHECK,
            Default::default(),
            Default::default(),
        );
    }
}

/// Hide the window when it remains visible but no longer owns the foreground.
/// A stale queued check is harmless if the window was reactivated in between.
#[cfg(windows)]
pub(super) fn hide_if_deactivated(hwnd: HWND) {
    let is_visible = unsafe { IsWindowVisible(hwnd) }.as_bool();
    if is_visible && unsafe { GetForegroundWindow() } == hwnd {
        return;
    }

    if is_visible {
        easysearch_core::log_debug!("deactivation check: foreground moved away, hiding window");
        hide_window();
    }

    if unsafe { IsWindowVisible(hwnd) }.as_bool() {
        easysearch_core::log_debug!("deactivation check: state busy, force hiding via Win32");
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
            let _ = KillTimer(Some(hwnd), DEFERRED_POLL_TIMER_ID);
        }
    }

    // Repair logical state even if Win32 was force-hidden above. If another
    // synchronous callback still owns the state borrow, retry on the next turn.
    let repaired = app_state::with_app_mut(|app| {
        if app.visible {
            clear_hidden_state(app);
        }
    })
    .is_some();
    if !repaired {
        queue_deactivation_check(hwnd);
    }
}

#[cfg(windows)]
fn clear_hidden_state(app: &mut super::app_state::AppState) -> HWND {
    let hwnd = app.hwnd;
    app.visible = false;
    app.input.clear();
    app.items.clear();
    app.result_items.clear();
    app.context_items.clear();
    app.plugin_items.clear();
    app.deferred_query = None;
    app.plugin_router.reset_search_sessions();
    app.preview = None;
    app.preview_seq += 1;
    app.selected_index = 0;
    app.result_selected_index = 0;
    app.context_selected_index = 0;
    app.context_source_index = None;
    app.view_mode = ViewMode::Results;
    app.pending_ime_char_suppression = 0;
    app.input_focused = true;
    hwnd
}

/// Hide the window and clear input.
#[cfg(windows)]
pub(super) fn hide_window() {
    // Step 1: Update state (brief borrow, no Win32 calls)
    let hwnd = app_state::with_app_mut(|app| {
        if !app.visible {
            return None;
        }
        Some(clear_hidden_state(app))
    })
    .flatten();

    // Step 2: Call Win32 APIs AFTER releasing the borrow
    if let Some(hwnd) = hwnd {
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
            let _ = KillTimer(Some(hwnd), DEFERRED_POLL_TIMER_ID);
        }
    }
}

/// Show the window with fade-in animation.
/// Flow.Launcher uses CircleEase animation, 160-560ms.
/// We use AnimateWindow(AW_BLEND) for similar effect.
#[cfg(windows)]
#[allow(dead_code)]
pub(super) fn show_window(app: &mut super::app_state::AppState) {
    unsafe {
        // Multi-monitor support: show on the active monitor
        unsafe extern "system" {
            fn GetCursorPos(lp_point: *mut windows::Win32::Foundation::POINT) -> i32;
        }

        let mut cursor_pos = windows::Win32::Foundation::POINT { x: 0, y: 0 };
        GetCursorPos(&mut cursor_pos);
        easysearch_core::log_debug!(
            "show_window: cursor at ({}, {})",
            cursor_pos.x,
            cursor_pos.y
        );

        let monitor = windows::Win32::Graphics::Gdi::MonitorFromPoint(
            cursor_pos,
            windows::Win32::Graphics::Gdi::MONITOR_DEFAULTTOPRIMARY,
        );

        let mut mi = windows::Win32::Graphics::Gdi::MONITORINFO {
            cbSize: std::mem::size_of::<windows::Win32::Graphics::Gdi::MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = windows::Win32::Graphics::Gdi::GetMonitorInfoW(monitor, &mut mi);

        let work = mi.rcWork;
        let mon_width = work.right - work.left;
        let mon_height = work.bottom - work.top;

        let width = layout::window_width_scaled(app.hwnd);
        let height = layout::window_height_scaled(app.items.len(), app.hwnd);
        let x = work.left + (mon_width - width) / 2;
        let y = work.top + mon_height / 4;

        easysearch_core::log_debug!("show_window: SetWindowPos x={x} y={y} w={width} h={height}");
        let _ = SetWindowPos(
            app.hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE,
        );

        easysearch_core::log_debug!("show_window: ShowWindow(SW_SHOW)");
        let _ = ShowWindow(app.hwnd, SW_SHOW);

        easysearch_core::log_debug!("show_window: force foreground focus");
        force_foreground(app.hwnd);
    }
    app.visible = true;
    easysearch_core::log_debug!("show_window: done, visible=true");
}

/// Hide with fade-out animation (old version — only call when already holding borrow
/// and you know ShowWindow won't be needed, or refactor the caller).
#[cfg(windows)]
#[allow(dead_code)]
pub(super) fn hide_window_inner(app: &mut super::app_state::AppState) {
    if !app.visible {
        return;
    }
    // NOTE: ShowWindow here can cause re-entrancy! This function should only be called
    // from contexts where the borrow has been released, or when we accept the risk.
    unsafe {
        let _ = ShowWindow(app.hwnd, SW_HIDE);
        let _ = KillTimer(Some(app.hwnd), DEFERRED_POLL_TIMER_ID);
    }
    app.visible = false;
    app.input.clear();
    app.items.clear();
    app.result_items.clear();
    app.context_items.clear();
    app.plugin_items.clear();
    app.deferred_query = None;
    app.plugin_router.reset_search_sessions();
    app.preview = None;
    app.preview_seq += 1;
    app.selected_index = 0;
    app.result_selected_index = 0;
    app.context_selected_index = 0;
    app.context_source_index = None;
    app.pending_ime_char_suppression = 0;
    app.view_mode = ViewMode::Results;
    app.input_focused = true;
}
