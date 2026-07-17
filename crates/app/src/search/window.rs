// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Win32 window creation, message loop, and event handling.
//! Includes IME support, debounce search, and integrated engine.

#[cfg(windows)]
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
#[cfg(windows)]
use windows::Win32::Graphics::Gdi::ValidateRect;
#[cfg(windows)]
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
#[cfg(windows)]
use windows::Win32::UI::Input::Ime::{
    CANDIDATEFORM, CFS_EXCLUDE, CFS_FORCE_POSITION, CFS_POINT, COMPOSITIONFORM, GCS_RESULTSTR,
    ImmGetCompositionStringW, ImmGetContext, ImmReleaseContext, ImmSetCandidateWindow,
    ImmSetCompositionWindow,
};
#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState;
#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::VK_CONTROL;
use windows::Win32::UI::WindowsAndMessaging::*;
#[cfg(windows)]
use windows::core::PCWSTR;

#[cfg(windows)]
use super::app_state::{self, AppState, ViewMode};
#[cfg(windows)]
use super::input::InputState;
#[cfg(windows)]
use super::layout;
#[cfg(windows)]
use super::messages::*;
#[cfg(windows)]
use super::plugin_bridge::{action_to_history_key_static, build_plugin_router};
#[cfg(windows)]
use super::renderer::Renderer;
#[cfg(windows)]
use crate::shared::hotkey;
#[cfg(windows)]
use crate::shared::tray;

/// Window class name.
#[cfg(windows)]
const CLASS_NAME: &str = "EasySearchWindow";

/// Run the GUI application.
#[cfg(windows)]
pub fn run() -> Result<(), String> {
    crate::log!("run() entered");

    unsafe {
        // Initialize COM (needed for WIC, Shell)
        let _ = windows::Win32::System::Com::CoInitializeEx(
            None,
            windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
        );
    }
    crate::log!("COM initialized");

    let hinstance =
        unsafe { GetModuleHandleW(None) }.map_err(|e| format!("GetModuleHandleW failed: {e}"))?;

    // Register window class
    let class_name = wide_null(CLASS_NAME);
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wnd_proc),
        hInstance: hinstance.into(),
        lpszClassName: PCWSTR(class_name.as_ptr()),
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or_default(),
        ..Default::default()
    };

    let atom = unsafe { RegisterClassExW(&wc) };
    if atom == 0 {
        return Err("RegisterClassExW failed".to_string());
    }

    // Create the popup window (initially hidden)
    // Use system DPI for initial sizing (we'll resize after hwnd is available)
    let sys_dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForSystem() };
    let dpi_factor = sys_dpi as f32 / 96.0;
    let width = layout::scale_with(layout::WINDOW_WIDTH, dpi_factor);
    let height = layout::scale_with(layout::SEARCH_BAR_HEIGHT, dpi_factor);

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            PCWSTR(class_name.as_ptr()),
            PCWSTR(wide_null("EasySearch").as_ptr()),
            WS_POPUP,
            // Center on screen
            (GetSystemMetrics(SM_CXSCREEN) - width) / 2,
            GetSystemMetrics(SM_CYSCREEN) / 4,
            width,
            height,
            None,
            None,
            Some(hinstance.into()),
            None,
        )
    }
    .map_err(|e| format!("CreateWindowExW failed: {e}"))?;
    crate::log!("Window handle created: {:?}", hwnd.0);

    // Now that we have hwnd, get the actual per-monitor DPI and resize if different
    let actual_width = layout::window_width_scaled(hwnd);
    let actual_height = layout::scale(layout::SEARCH_BAR_HEIGHT, hwnd);
    if actual_width != width || actual_height != height {
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                None,
                0,
                0,
                actual_width,
                actual_height,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

    // Initialize renderer
    let mut renderer = Renderer::new()?;
    crate::log!("Renderer created");
    renderer.create_render_target(hwnd, actual_width as u32, actual_height as u32)?;
    crate::log!("Render target created");

    // Apply DWM window styling (Win11 round corners + shadow)
    apply_dwm_style(hwnd);
    crate::log!("DWM style applied");
    // Register global hotkey
    if !hotkey::register(hwnd) {
        crate::log!("WARNING: failed to register hotkey (Alt+Space)");
    } else {
        crate::log!("Hotkey Alt+Space registered successfully");
    }

    // Add tray icon
    tray::add_tray_icon(hwnd);

    // Set window icon (for Alt-Tab and taskbar)
    {
        let app_icon = tray::load_app_icon();
        unsafe {
            SendMessageW(
                hwnd,
                WM_SETICON,
                Some(WPARAM(0)),
                Some(LPARAM(app_icon.0 as isize)),
            ); // ICON_SMALL
            SendMessageW(
                hwnd,
                WM_SETICON,
                Some(WPARAM(1)),
                Some(LPARAM(app_icon.0 as isize)),
            ); // ICON_BIG
        }
    }

    crate::log!("Window created, tray icon added. Entering message loop...");

    // ── Initialize search engine ──────────────────────────────────────────────
    let engine_for_search = super::engine_bridge::start_engine(hwnd);

    // Store app state — build router AFTER engine so FileSearchPlugin can be registered
    app_state::init(AppState {
        hwnd,
        renderer,
        input: InputState::new(),
        view_mode: ViewMode::Results,
        items: Vec::new(),
        selected_index: 0,
        result_items: Vec::new(),
        result_selected_index: 0,
        context_items: Vec::new(),
        context_selected_index: 0,
        context_source_index: None,
        visible: false,
        plugin_router: build_plugin_router(Some(engine_for_search.clone())),
        plugin_items: Vec::new(),
        deferred_query: None,
        current_search_seq: 0,
        index_ready: false,
        history: super::history::History::load(),
        i18n: {
            let settings_read = crate::SHARED_SETTINGS.get().and_then(|s| s.read().ok());
            match settings_read {
                Some(ref s) if !s.language.is_empty() => {
                    crate::i18n::engine::I18nEngine::with_locale(&s.language)
                }
                _ => crate::i18n::engine::I18nEngine::new(),
            }
        },
        icon_cache: super::icon::IconCache::new(),
        anim_frame: ANIM_TOTAL_FRAMES, // Start fully visible (no animation on first load)
        search_active: false,
        last_window_size: (actual_width, actual_height),
        engine: Some(engine_for_search.clone()),
        preview: None,
        preview_seq: 0,
        index_status: String::new(),
        index_error: None,
        pending_ime_char_suppression: 0,
        input_focused: true,
        cursor_moved_at: 0,
    });

    // Start settings poll timer
    unsafe {
        let _ = SetTimer(Some(hwnd), SETTINGS_POLL_TIMER_ID, SETTINGS_POLL_MS, None);
    }

    // Initial render
    do_render();

    // Message loop
    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    // Cleanup
    hotkey::unregister(hwnd);
    tray::remove_tray_icon(hwnd);

    Ok(())
}

#[cfg(windows)]
pub(super) fn sync_active_items(app: &mut AppState) {
    match app.view_mode {
        ViewMode::Results => {
            app.items = app.result_items.clone();
            app.selected_index = app
                .result_selected_index
                .min(app.items.len().saturating_sub(1));
        }
        ViewMode::ContextActions => {
            app.items = app.context_items.clone();
            app.selected_index = app
                .context_selected_index
                .min(app.items.len().saturating_sub(1));
        }
    }
}

#[cfg(windows)]
pub(super) fn open_context_actions(app: &mut AppState) -> bool {
    if app.view_mode != ViewMode::Results || app.result_items.is_empty() {
        return false;
    }

    let index = app.result_selected_index.min(app.result_items.len() - 1);
    let source = app.result_items[index].clone();
    let context_items = super::context::build_context_items(&source);
    if context_items.is_empty() {
        return false;
    }

    app.context_source_index = Some(index);
    app.context_items = context_items;
    app.context_selected_index = 0;
    app.view_mode = ViewMode::ContextActions;

    // Start async preview loading for the source item
    app.preview = None;
    app.preview_seq += 1;
    if let Some(ref path) = source.icon_path {
        let path_owned = path.clone();
        let hwnd_raw = app.hwnd.0 as usize;
        let seq = app.preview_seq;
        std::thread::Builder::new()
            .name("preview-load".to_string())
            .spawn(move || {
                if let Some(info) = super::preview::PreviewInfo::from_path(&path_owned) {
                    // Box it and pass pointer via LPARAM; seq via WPARAM
                    let boxed = Box::into_raw(Box::new(info));
                    unsafe {
                        let hwnd = HWND(hwnd_raw as *mut _);
                        let _ = PostMessageW(
                            Some(hwnd),
                            WM_PREVIEW_READY,
                            WPARAM(seq as usize),
                            LPARAM(boxed as isize),
                        );
                    }
                }
            })
            .ok();
    }

    sync_active_items(app);
    resize_for_results(app);
    true
}

#[cfg(windows)]
fn close_context_actions(app: &mut AppState) -> bool {
    if app.view_mode != ViewMode::ContextActions {
        return false;
    }

    app.view_mode = ViewMode::Results;
    app.context_items.clear();
    app.context_selected_index = 0;
    app.context_source_index = None;
    // Clear preview and bump seq to discard any in-flight result
    app.preview = None;
    app.preview_seq += 1;
    sync_active_items(app);
    resize_for_results(app);
    true
}

#[cfg(windows)]
fn set_active_selection(app: &mut AppState, index: usize) {
    app.selected_index = index.min(app.items.len().saturating_sub(1));
    match app.view_mode {
        ViewMode::Results => {
            app.result_selected_index = app.selected_index;
        }
        ViewMode::ContextActions => {
            app.context_selected_index = app.selected_index;
        }
    }
}

#[cfg(windows)]
fn item_index_from_client_point(app: &AppState, x: i32, y: i32) -> Option<usize> {
    if x < 0 || y < 0 || app.items.is_empty() {
        return None;
    }

    let list_top =
        (layout::SEARCH_BAR_HEIGHT + layout::SEPARATOR_HEIGHT + layout::RESULT_MARGIN_V) as i32;
    if y < list_top {
        return None;
    }

    let row = ((y - list_top) as f32 / layout::ITEM_HEIGHT).floor() as usize;
    let total_items = app.items.len();
    let max_visible = layout::MAX_VISIBLE_ITEMS;
    let scroll_offset = if app.selected_index >= max_visible {
        app.selected_index - max_visible + 1
    } else {
        0
    };
    let visible_end = (scroll_offset + max_visible).min(total_items);
    let visible_count = visible_end.saturating_sub(scroll_offset);

    if row >= visible_count {
        None
    } else {
        Some(scroll_offset + row)
    }
}

#[cfg(windows)]
fn selected_item_screen_point(app: &AppState) -> POINT {
    let mut rect = RECT::default();
    let _ = unsafe { GetWindowRect(app.hwnd, &mut rect) };

    let max_visible = layout::MAX_VISIBLE_ITEMS;
    let scroll_offset = if app.selected_index >= max_visible {
        app.selected_index - max_visible + 1
    } else {
        0
    };
    let visible_row = app.selected_index.saturating_sub(scroll_offset);
    let y = rect.top
        + (layout::SEARCH_BAR_HEIGHT + layout::SEPARATOR_HEIGHT + layout::RESULT_MARGIN_V) as i32
        + visible_row as i32 * layout::ITEM_HEIGHT as i32
        + (layout::ITEM_HEIGHT as i32 / 2);
    let x = rect.left + layout::WINDOW_WIDTH as i32 - 24;

    POINT { x, y }
}

#[cfg(windows)]
pub(super) fn show_native_context_menu_safe() {
    let request = app_state::with_app_ref(|app| {
        if app.items.is_empty() {
            return None;
        }

        let point = selected_item_screen_point(app);
        let current_item = &app.items[app.selected_index.min(app.items.len() - 1)];
        match &current_item.action {
            easysearch_core::Action::ShowFileContextMenu { path, is_dir } => {
                Some((app.hwnd, path.clone(), *is_dir, point))
            }
            _ => current_item
                .context_data
                .as_ref()
                .map(|data| (app.hwnd, data.file_path.clone(), data.is_directory, point))
                .or_else(|| {
                    app.context_source_index
                        .and_then(|index| app.result_items.get(index))
                        .and_then(|item| item.context_data.as_ref())
                        .map(|data| (app.hwnd, data.file_path.clone(), data.is_directory, point))
                }),
        }
    })
    .flatten();

    let Some((hwnd, path, _is_dir, point)) = request else {
        return;
    };

    let _ = super::shell_context_menu::show_for_path(hwnd, &path, Some(point));
}

/// Position the IME composition window *and* candidate list near the text caret.
///
/// Only setting the composition window (`CFS_POINT`) is not enough: modern
/// TSF-based IMEs (Microsoft Pinyin / Japanese IME, etc.) frequently ignore it
/// for the candidate list and fall back to the top-left corner of the monitor.
/// To fix this we additionally:
///   * force the composition window position with `CFS_FORCE_POSITION`, and
///   * set the candidate window via `ImmSetCandidateWindow` with `CFS_EXCLUDE`,
///     supplying the caret rectangle so the IME places the candidate list right
///     below the search bar instead of at (0,0).
///
/// `caret_x` / `caret_y` are in window client coordinates (top-left origin).
#[cfg(windows)]
fn position_ime_windows(hwnd: HWND, caret_x: i32, _caret_y: i32) {
    use windows::Win32::Foundation::{POINT, RECT};

    unsafe {
        let himc = ImmGetContext(hwnd);
        if himc.is_invalid() {
            return;
        }

        // Composition window: force the position so the IME cannot relocate it.
        // Position at caret_x, vertically at the text baseline within the search bar.
        let comp_y = (layout::SEARCH_BAR_HEIGHT as i32 - 22) / 2; // text vertical center
        let mut cf = COMPOSITIONFORM {
            dwStyle: CFS_POINT | CFS_FORCE_POSITION,
            ptCurrentPos: POINT {
                x: caret_x,
                y: comp_y,
            },
            ..Default::default()
        };
        let _ = ImmSetCompositionWindow(himc, &mut cf);

        // Candidate window: CFS_EXCLUDE tells the IME "don't overlap rcArea,
        // place the candidate list below it". We set rcArea to the full search
        // bar height so the candidate list appears just below the bar.
        let mut cand = CANDIDATEFORM {
            dwIndex: 0,
            dwStyle: CFS_EXCLUDE,
            ptCurrentPos: POINT {
                x: caret_x,
                y: comp_y,
            },
            rcArea: RECT {
                left: caret_x.saturating_sub(2),
                top: 0,
                right: caret_x + 2,
                bottom: layout::SEARCH_BAR_HEIGHT as i32, // exclude the entire search bar
            },
        };
        let _ = ImmSetCandidateWindow(himc, &mut cand);

        let _ = ImmReleaseContext(hwnd, himc);
    }
}

/// Window procedure.
#[cfg(windows)]
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_HOTKEY => {
            if wparam.0 as i32 == hotkey::HOTKEY_ID {
                toggle_visibility();
            }
            LRESULT(0)
        }

        WM_CHAR => {
            let ch = char::from_u32(wparam.0 as u32);
            if let Some(ch) = ch {
                // Ignore control characters (Ctrl+key combos generate 0x01-0x1A)
                // Only accept printable characters (>= space, not DEL)
                if ch >= ' ' && ch != '\x7f' {
                    // Also skip if Ctrl is held (Ctrl+V etc. handled in WM_KEYDOWN)
                    let ctrl_held = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
                    if !ctrl_held {
                        app_state::with_app_mut(|app| {
                            if app.pending_ime_char_suppression > 0 {
                                app.pending_ime_char_suppression -= 1;
                                return;
                            }
                            app.input.insert_char(ch);
                            on_input_changed(app);
                        });
                        do_render();
                    }
                }
            }
            LRESULT(0)
        }

        // ── IME Support ──────────────────────────────────────────────────────
        WM_IME_STARTCOMPOSITION => {
            // Position the IME composition + candidate window near the text caret
            let cursor_x = app_state::with_app_ref(|app| {
                app.renderer
                    .measure_text_width(app.input.text(), app.input.cursor())
            })
            .unwrap_or(0.0_f32);

            position_ime_windows(
                hwnd,
                (layout::PADDING_H + cursor_x) as i32,
                layout::SEARCH_BAR_HEIGHT as i32,
            );

            // MUST call DefWindowProc for IME to work
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        WM_IME_COMPOSITION => {
            let gcs_flags = lparam.0 as u32;

            // Always reposition IME window on composition update
            let cursor_x = app_state::with_app_ref(|app| {
                app.renderer
                    .measure_text_width(app.input.text(), app.input.cursor())
            })
            .unwrap_or(0.0_f32);
            position_ime_windows(
                hwnd,
                (layout::PADDING_H + cursor_x) as i32,
                layout::SEARCH_BAR_HEIGHT as i32,
            );

            // Final committed string from IME
            if gcs_flags & GCS_RESULTSTR.0 != 0 {
                unsafe {
                    let himc = ImmGetContext(hwnd);
                    if !himc.is_invalid() {
                        let len = ImmGetCompositionStringW(himc, GCS_RESULTSTR, None, 0);
                        if len > 0 {
                            let mut buf = vec![0u16; (len as usize) / 2];
                            ImmGetCompositionStringW(
                                himc,
                                GCS_RESULTSTR,
                                Some(buf.as_mut_ptr() as *mut _),
                                len as u32,
                            );
                            let text = String::from_utf16_lossy(&buf);
                            app_state::with_app_mut(|app| {
                                app.pending_ime_char_suppression = app
                                    .pending_ime_char_suppression
                                    .saturating_add(text.chars().count());
                                app.input.insert_str(&text);
                                on_input_changed(app);
                            });
                            do_render();
                        }
                        let _ = ImmReleaseContext(hwnd, himc);
                    }
                }
                LRESULT(0)
            } else {
                // Let DefWindowProc handle composition display
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
        }

        WM_IME_ENDCOMPOSITION => {
            // Reset the suppression counter: if the IME ends composition
            // (e.g. user switches back to English input), any remaining
            // expected WM_CHAR messages will never arrive.  Without this
            // reset the counter stays >0 and blocks all subsequent typing.
            app_state::with_app_mut(|app| {
                app.pending_ime_char_suppression = 0;
            });
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }

        WM_KEYDOWN => {
            handle_keydown(wparam);
            LRESULT(0)
        }

        WM_ACTIVATE => {
            // Hide on deactivation (lost focus)
            let activation = (wparam.0 as u32) & 0xFFFF;
            if activation == 0 {
                // WA_INACTIVE
                let is_visible = app_state::with_app_ref(|app| app.visible).unwrap_or(false);
                if is_visible {
                    hide_window();
                    // Fallback: if hide_window couldn't acquire the borrow (RefCell contention),
                    // the window might still be visible. Force-hide it at the Win32 level so
                    // the user never sees a "ghost" window lingering without focus.
                    let still_visible = unsafe { IsWindowVisible(hwnd) }.as_bool();
                    if still_visible {
                        crate::log!(
                            "WM_ACTIVATE: hide_window failed (borrow contention), force hiding via Win32"
                        );
                        unsafe {
                            let _ = ShowWindow(hwnd, SW_HIDE);
                        }
                        // Mark visible=false when the borrow becomes available
                        app_state::with_app_mut(|app| {
                            if app.visible {
                                app.visible = false;
                                app.input.clear();
                                app.items.clear();
                                app.result_items.clear();
                                app.context_items.clear();
                                app.plugin_items.clear();
                                app.deferred_query = None;
                                app.plugin_router.reset_search_sessions();
                                app.selected_index = 0;
                                app.result_selected_index = 0;
                                app.context_selected_index = 0;
                                app.context_source_index = None;
                                app.view_mode = ViewMode::Results;
                                app.pending_ime_char_suppression = 0;
                                app.input_focused = true;
                            }
                        });
                    }
                }
            }
            LRESULT(0)
        }

        WM_PAINT => {
            do_render();
            unsafe {
                let _ = ValidateRect(Some(hwnd), None);
            }
            LRESULT(0)
        }

        WM_SIZE => {
            let width = (lparam.0 as u32) & 0xFFFF;
            let height = ((lparam.0 as u32) >> 16) & 0xFFFF;
            app_state::with_app_mut(|app| {
                app.renderer.resize(width, height);
            });
            LRESULT(0)
        }

        // ── Debounce timer ───────────────────────────────────────────────────
        WM_TIMER => {
            if wparam.0 == SEARCH_DEBOUNCE_TIMER_ID {
                unsafe {
                    let _ = KillTimer(Some(hwnd), SEARCH_DEBOUNCE_TIMER_ID);
                }
                app_state::with_app_mut(|app| {
                    run_debounced_search(app);
                });
                do_render();
            } else if wparam.0 == BUSY_ANIM_TIMER_ID {
                let keep_running = app_state::with_app_mut(|app| {
                    app.anim_frame = app.anim_frame.wrapping_add(1) % ANIM_TOTAL_FRAMES;
                    app.search_active || app.icon_cache.has_pending_loads()
                })
                .unwrap_or(false);
                if !keep_running {
                    unsafe {
                        let _ = KillTimer(Some(hwnd), BUSY_ANIM_TIMER_ID);
                    }
                }
                do_render();
            } else if wparam.0 == DEFERRED_POLL_TIMER_ID {
                // Poll for background plugin results (FileSearch)
                let should_stop =
                    app_state::with_app_mut(|app| super::search_flow::poll_deferred_results(app))
                        .unwrap_or(true);

                if should_stop {
                    app_state::with_app_mut(|app| {
                        app.deferred_query = None;
                        app.search_active = false;
                    });
                    unsafe {
                        let _ = KillTimer(Some(hwnd), DEFERRED_POLL_TIMER_ID);
                    }
                    do_render();
                }
            } else if wparam.0 == ANIM_TIMER_ID {
                // Advance animation frame
                let done = app_state::with_app_mut(|app| {
                    app.anim_frame = app.anim_frame.saturating_add(1);
                    app.anim_frame >= ANIM_TOTAL_FRAMES
                })
                .unwrap_or(true);
                if done {
                    unsafe {
                        let _ = KillTimer(Some(hwnd), ANIM_TIMER_ID);
                    }
                }
                do_render();
            } else if wparam.0 == SETTINGS_POLL_TIMER_ID {
                poll_settings_changes(hwnd);
            }
            LRESULT(0)
        }

        // ── Preview loaded from background thread ────────────────────────────
        m if m == WM_PREVIEW_READY => {
            let seq = wparam.0 as u64;
            let ptr = lparam.0 as *mut super::preview::PreviewInfo;
            // Reconstruct the Box to take ownership (and free on drop)
            let info = unsafe { Box::from_raw(ptr) };
            app_state::with_app_mut(|app| {
                // Only accept if seq matches (not stale)
                if app.preview_seq == seq && app.view_mode == ViewMode::ContextActions {
                    app.preview = Some(*info);
                    resize_for_results(app);
                }
            });
            do_render();
            LRESULT(0)
        }

        // ── Index ready notification ─────────────────────────────────────────
        m if m == WM_ICON_READY => {
            let ptr = lparam.0 as *mut IconReadyPayload;
            let payload = unsafe { Box::from_raw(ptr) };
            let should_render = app_state::with_app_mut(|app| {
                if let Some(rt) = app.renderer.render_target.clone() {
                    app.icon_cache
                        .finish_load(payload.request, payload.pixels, &rt);
                }
                payload.seq_id == app.current_search_seq && app.visible
            })
            .unwrap_or(false);
            if should_render {
                do_render();
            }
            LRESULT(0)
        }

        m if m == WM_INDEX_READY => {
            app_state::with_app_mut(|app| {
                app.index_ready = true;
                app.index_status.clear();
                app.index_error = None;
            });
            do_render();
            LRESULT(0)
        }

        // ── Engine progress/error events ─────────────────────────────────────
        m if m == WM_ENGINE_EVENT => {
            let evt_type = wparam.0;
            let data_ptr = lparam.0;

            app_state::with_app_mut(|app| match evt_type {
                ENGINE_EVT_DRIVE_INDEXING => {
                    let msg = unsafe { Box::from_raw(data_ptr as *mut EngineEventPayload) };
                    app.index_status = match *msg {
                        EngineEventPayload::DriveIndexing { drive } => app
                            .i18n
                            .get("search_status_indexing_drive")
                            .replace("{drive}", &drive.to_string()),
                        _ => String::new(),
                    };
                    app.index_error = None;
                }
                ENGINE_EVT_DRIVE_READY => {
                    let msg = unsafe { Box::from_raw(data_ptr as *mut EngineEventPayload) };
                    app.index_status = match *msg {
                        EngineEventPayload::DriveReady {
                            drive,
                            records,
                            seconds,
                        } => app
                            .i18n
                            .get("search_status_drive_ready")
                            .replace("{drive}", &drive.to_string())
                            .replace("{records}", &records.to_string())
                            .replace("{seconds}", &format!("{seconds:.1}")),
                        _ => String::new(),
                    };
                }
                ENGINE_EVT_DRIVE_ERROR => {
                    let msg = unsafe { Box::from_raw(data_ptr as *mut EngineEventPayload) };
                    let localized = match *msg {
                        EngineEventPayload::DriveError { drive, error } => app
                            .i18n
                            .get("search_status_drive_error")
                            .replace("{drive}", &drive.to_string())
                            .replace("{error}", &error),
                        _ => String::new(),
                    };
                    app.index_error = Some(localized.clone());
                    app.index_status = localized;
                }
                ENGINE_EVT_ALL_READY => {
                    app.index_ready = true;
                    app.index_status.clear();
                    app.index_error = None;
                }
                _ => {}
            });
            do_render();
            LRESULT(0)
        }

        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }

        // ── DPI change detection (monitor switch / user settings change) ────
        WM_DPICHANGED => {
            // lparam contains a pointer to a RECT with the suggested new position
            let suggested_rect = unsafe { &*(lparam.0 as *const RECT) };

            // Recalculate window size using the new DPI (GetDpiForWindow will
            // already reflect the new DPI when WM_DPICHANGED is received).
            app_state::with_app_mut(|app| {
                let has_preview =
                    app.preview.is_some() && app.view_mode == ViewMode::ContextActions;
                let new_width = layout::window_width_scaled(app.hwnd);
                let new_height = layout::window_height_with_preview_scaled(
                    app.items.len(),
                    has_preview,
                    app.hwnd,
                );

                // Use suggested position from Windows, but our own calculated size
                unsafe {
                    let _ = SetWindowPos(
                        app.hwnd,
                        None,
                        suggested_rect.left,
                        suggested_rect.top,
                        new_width,
                        new_height,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                }

                app.renderer.resize(new_width as u32, new_height as u32);
                crate::log!(
                    "WM_DPICHANGED: dpi_scale={:.2}, resized to {}x{}",
                    layout::dpi_scale(app.hwnd),
                    new_width,
                    new_height
                );
            });

            do_render();
            LRESULT(0)
        }

        // ── Theme change detection ──────────────────────────────────────────
        WM_SETTINGCHANGE => {
            // Windows broadcasts WM_SETTINGCHANGE when system theme changes.
            // Re-detect theme and update renderer.
            app_state::with_app_mut(|app| {
                app.renderer.theme = crate::theme::Theme::system();
            });
            do_render();
            LRESULT(0)
        }

        _ if msg == tray::WM_TRAY_ICON => {
            let event = (lparam.0 as u32) & 0xFFFF;
            if event == WM_LBUTTONDBLCLK {
                toggle_visibility();
            } else if event == WM_RBUTTONUP {
                // Show tray context menu on right-click
                tray::show_context_menu(hwnd);
            }
            LRESULT(0)
        }

        WM_COMMAND => {
            let cmd_id = (wparam.0 as u32) & 0xFFFF;
            match cmd_id {
                tray::IDM_SETTINGS => {
                    // Open settings.json in user's default editor
                    if let Some(settings) = crate::SHARED_SETTINGS.get() {
                        crate::settings::open_settings_file(settings.clone());
                    }
                }
                tray::IDM_EXIT => unsafe {
                    DestroyWindow(hwnd).ok();
                },
                _ => {}
            }
            LRESULT(0)
        }

        WM_LBUTTONUP | WM_RBUTTONUP => {
            let x = (lparam.0 as i16) as i32;
            let y = ((lparam.0 >> 16) as i16) as i32;
            let is_right_click = msg == WM_RBUTTONUP;

            let deferred = app_state::with_app_mut(|app| {
                let Some(index) = item_index_from_client_point(app, x, y) else {
                    return 0u8;
                };

                set_active_selection(app, index);
                app.input_focused = false;
                if is_right_click && app.view_mode == ViewMode::Results {
                    2
                } else {
                    1
                }
            })
            .unwrap_or(0u8);

            match deferred {
                1 => super::execution::execute_selected_safe(),
                2 => {
                    app_state::with_app_mut(|app| {
                        let _ = open_context_actions(app);
                    });
                }
                _ => {}
            }

            do_render();
            LRESULT(0)
        }

        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Handle keydown events.
#[cfg(windows)]
fn handle_keydown(wparam: WPARAM) {
    let cmd = app_state::with_app_ref(|app| {
        let idx = app.selected_index.min(app.items.len().saturating_sub(1));
        let is_hint = app.view_mode == ViewMode::Results
            && app.input.text().trim().is_empty()
            && !app.items.is_empty()
            && matches!(app.items[idx].action, easysearch_core::Action::None);
        let input_empty = app.input.text().trim().is_empty();
        let vm = app.view_mode;
        let focused = app.input_focused;
        (vm, input_empty, is_hint, focused)
    })
    .map(|(vm, empty, hint, focused)| {
        super::key_command::decode_key_command(wparam, vm, hint, empty, focused)
    })
    .unwrap_or(super::key_command::KeyCommand::None);

    let deferred = app_state::with_app_mut(|app| super::key_command::execute_key_command(app, cmd))
        .unwrap_or(super::key_command::DeferredAction::None);

    // Execute deferred actions AFTER borrow is released (Win32 calls can re-enter)
    match deferred {
        super::key_command::DeferredAction::Hide => hide_window(),
        super::key_command::DeferredAction::Execute => super::execution::execute_selected_safe(),
        super::key_command::DeferredAction::OpenFolder => {
            super::execution::open_folder_or_containing_safe()
        }
        super::key_command::DeferredAction::OpenContext => {
            app_state::with_app_mut(|app| {
                let _ = open_context_actions(app);
            });
        }
        super::key_command::DeferredAction::CloseContext => {
            app_state::with_app_mut(|app| {
                let _ = close_context_actions(app);
            });
        }
        super::key_command::DeferredAction::ShowNativeContextMenu => {
            show_native_context_menu_safe()
        }
        super::key_command::DeferredAction::None => {}
    }

    do_render();
}

/// Called when input text changes — queries plugins via Router.
///
/// Fast (non-background) plugins run immediately; background plugins (e.g.
/// FileSearch) are spawned on a separate thread and results are polled via
/// a Windows timer.
#[cfg(windows)]
fn on_input_changed(app: &mut AppState) {
    super::search_flow::on_input_changed(app);
}

#[cfg(windows)]
fn run_debounced_search(app: &mut AppState) {
    super::search_flow::run_debounced_search(app);
}

/// Resize window to fit the current number of results.
#[cfg(windows)]
fn resize_for_results(app: &mut AppState) {
    super::search_flow::resize_for_results(app);
}

/// Toggle window visibility.
#[cfg(windows)]
fn toggle_visibility() {
    super::visibility::toggle_visibility();
}

#[cfg(windows)]
fn hide_window() {
    super::visibility::hide_window();
}

/// Execute the currently selected item.
#[cfg(windows)]
#[allow(dead_code)]
fn execute_selected(app: &mut AppState) {
    if app.items.is_empty() {
        return;
    }

    let idx = app.selected_index.min(app.items.len() - 1);
    let action = app.items[idx].action.clone();

    // Record usage for frequency-based ranking
    let history_key = action_to_history_key_static(&action);
    app.history.record(&history_key);
    app.history.save();

    // Hide window first
    super::visibility::hide_window_inner(app);

    // Execute the action
    super::action::execute(&action);
}

// Preview functionality removed — fs::metadata on UI thread caused sluggishness.

/// Poll the settings file for changes and apply them.
/// Reads settings.json from disk and compares with in-memory state.
#[cfg(windows)]
fn poll_settings_changes(hwnd: HWND) {
    use crate::shared::settings_store::SettingsStore;

    let settings_path = easysearch_core::paths::settings_file();

    // Read current settings from the global shared state
    let current = match crate::SHARED_SETTINGS.get() {
        Some(s) => match s.read() {
            Ok(guard) => guard.clone(),
            Err(_) => return,
        },
        None => return,
    };

    // Load from disk
    let on_disk = SettingsStore::load(&settings_path);

    // Compare — if nothing changed, bail early
    if on_disk == current {
        return;
    }

    // Apply changes via settings_sync module
    let changed = app_state::with_app_mut(|app| {
        super::settings_sync::apply_settings_diff(app, hwnd, &current, &on_disk)
    })
    .unwrap_or(false);

    // Update global shared settings
    if changed {
        if let Some(s) = crate::SHARED_SETTINGS.get() {
            if let Ok(mut guard) = s.write() {
                *guard = on_disk;
            }
        }
        do_render();
    }
}

/// Trigger a re-render.
#[cfg(windows)]
fn do_render() {
    super::render_bridge::do_render();
}

/// Convert &str to null-terminated wide string.
#[cfg(windows)]
fn wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Apply DWM attributes for round corners and shadow (Windows 11+).
#[cfg(windows)]
fn apply_dwm_style(hwnd: HWND) {
    use windows::Win32::Graphics::Dwm::{
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmExtendFrameIntoClientArea,
        DwmSetWindowAttribute,
    };
    use windows::Win32::UI::Controls::MARGINS;

    // Round corners (Win11)
    let preference = DWMWCP_ROUND.0 as u32;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            std::ptr::from_ref(&preference) as *const _,
            std::mem::size_of::<u32>() as u32,
        );
    }

    // Extend frame into client area for shadow effect
    let margins = MARGINS {
        cxLeftWidth: -1,
        cxRightWidth: -1,
        cyTopHeight: -1,
        cyBottomHeight: -1,
    };
    unsafe {
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
    }
}
