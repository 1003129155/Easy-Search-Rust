// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Win32 window creation, message loop, and event handling.
//! Includes IME support, debounce search, and integrated engine.

#[cfg(windows)]
use std::cell::RefCell;

#[cfg(windows)]
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
#[cfg(windows)]
use windows::Win32::Graphics::Gdi::ValidateRect;
#[cfg(windows)]
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
#[cfg(windows)]
use windows::Win32::UI::Input::Ime::{
    ImmGetContext, ImmReleaseContext, ImmSetCompositionWindow, ImmSetCandidateWindow,
    CFS_POINT, CFS_FORCE_POSITION, CFS_EXCLUDE, COMPOSITIONFORM, CANDIDATEFORM,
    ImmGetCompositionStringW, GCS_RESULTSTR,
};
#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, SetFocus, VK_BACK, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME,
    VK_LEFT, VK_MENU, VK_RETURN, VK_RIGHT, VK_SHIFT, VK_UP,
};
#[cfg(windows)]
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::*;
#[cfg(windows)]
use windows::core::PCWSTR;

#[cfg(windows)]
use crate::shared::hotkey;
#[cfg(windows)]
use crate::shared::icon_assets;
#[cfg(windows)]
use super::input::InputState;
#[cfg(windows)]
use super::layout;
#[cfg(windows)]
use super::renderer::{DisplayItem, Renderer};
#[cfg(windows)]
use crate::shared::tray;

#[cfg(windows)]
use easysearch_core::Router;
#[cfg(windows)]
use quick_launch_store::global_store;

/// Custom message: engine event (progress / error) — wparam encodes the event type.
#[cfg(windows)]
const WM_ENGINE_EVENT: u32 = WM_APP + 3;

/// Custom message: index build is complete.
#[cfg(windows)]
const WM_INDEX_READY: u32 = WM_APP + 2;

/// Deferred result poll timer ID.
#[cfg(windows)]
const DEFERRED_POLL_TIMER_ID: usize = 100;

/// Deferred result poll interval in milliseconds.
#[cfg(windows)]
const DEFERRED_POLL_MS: u32 = 50;

/// Animation timer ID for result list fade-in.
#[cfg(windows)]
const ANIM_TIMER_ID: usize = 101;

/// Settings change poll timer ID.
#[cfg(windows)]
const SETTINGS_POLL_TIMER_ID: usize = 102;

/// Settings poll interval in milliseconds (2 seconds for file-based settings reload).
#[cfg(windows)]
const SETTINGS_POLL_MS: u32 = 2000;

/// Animation frame interval (~60fps).
#[cfg(windows)]
const ANIM_FRAME_MS: u32 = 16;

/// Animation duration in frames (16ms × 10 = 160ms, matches Flow.Launcher).
#[cfg(windows)]
const ANIM_TOTAL_FRAMES: u8 = 10;

/// Engine event sub-types passed via WPARAM in WM_ENGINE_EVENT.
#[cfg(windows)]
const ENGINE_EVT_DRIVE_INDEXING: usize = 1;
#[cfg(windows)]
const ENGINE_EVT_DRIVE_READY: usize = 2;
#[cfg(windows)]
const ENGINE_EVT_DRIVE_ERROR: usize = 3;
#[cfg(windows)]
const ENGINE_EVT_ALL_READY: usize = 4;

#[cfg(windows)]
enum EngineEventPayload {
    DriveIndexing { drive: char },
    DriveReady { drive: char, records: usize, seconds: f64 },
    DriveError { drive: char, error: String },
}

/// Pending background (deferred) results from plugins like FileSearch.
/// On drop, the cancel token is set to abort the background thread.
#[cfg(windows)]
struct DeferredQuery {
    rx: std::sync::mpsc::Receiver<Vec<easysearch_core::PluginResult>>,
    seq_id: u64,
    #[allow(dead_code)]
    cancel: easysearch_core::CancelToken,
}

#[cfg(windows)]
impl Drop for DeferredQuery {
    fn drop(&mut self) {
        use std::sync::atomic::Ordering;
        self.cancel.store(true, Ordering::Relaxed);
    }
}

/// Application state stored in thread-local for the window procedure.
#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Results,
    ContextActions,
}

/// Application state stored in thread-local for the window procedure.
#[cfg(windows)]
struct AppState {
    hwnd: HWND,
    renderer: Renderer,
    input: InputState,
    view_mode: ViewMode,
    items: Vec<DisplayItem>,
    selected_index: usize,
    result_items: Vec<DisplayItem>,
    result_selected_index: usize,
    context_items: Vec<DisplayItem>,
    context_selected_index: usize,
    context_source_index: Option<usize>,
    visible: bool,
    plugin_router: Router,
    /// Items from local plugins (shown immediately).
    plugin_items: Vec<DisplayItem>,
    /// Pending deferred (background) query. Polled via timer.
    deferred_query: Option<DeferredQuery>,
    /// Current search sequence ID (incremented on each input change).
    current_search_seq: u64,
    /// Whether the file search index is ready.
    index_ready: bool,
    /// Usage history for frequency-based ranking.
    history: super::history::History,
    /// Internationalization strings.
    i18n: crate::i18n::engine::I18nEngine,
    /// Icon cache for rendering file/folder icons.
    icon_cache: super::icon::IconCache,
    /// Animation progress for result list (0 = start, ANIM_TOTAL_FRAMES = done).
    anim_frame: u8,
    /// Engine reference for hot-plug drive management.
    engine: Option<std::sync::Arc<easysearch_engine::SearchEngine>>,
    /// Preview info for the currently selected file (loaded on selection change).
    preview: Option<super::preview::PreviewInfo>,
    /// Index progress status text (e.g. "Indexing C:..." or error messages).
    index_status: String,
    /// Last indexing error message (if any).
    index_error: Option<String>,
    /// Number of committed IME chars whose follow-up `WM_CHAR` messages should
    /// be ignored to avoid duplicating CJK input.
    pending_ime_char_suppression: usize,
}

#[cfg(windows)]
thread_local! {
    static APP_STATE: RefCell<Option<AppState>> = const { RefCell::new(None) };
}

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

    let hinstance = unsafe { GetModuleHandleW(None) }
        .map_err(|e| format!("GetModuleHandleW failed: {e}"))?;

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
    let width = layout::WINDOW_WIDTH as i32;
    let height = layout::SEARCH_BAR_HEIGHT as i32;

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

    // Initialize renderer
    let mut renderer = Renderer::new()?;
    crate::log!("Renderer created");
    renderer.create_render_target(hwnd, width as u32, height as u32)?;
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
    crate::log!("Window created, tray icon added. Entering message loop...");

    // ── Initialize search engine ──────────────────────────────────────────────
    // Use drives from settings if configured, otherwise fall back to env/default
    let engine_config = if let Some(settings) = crate::SHARED_SETTINGS.get() {
        let settings_read = settings.read().unwrap();
        if settings_read.index_drives.is_empty() {
            easysearch_engine::EngineConfig::default()
        } else {
            let drives: Vec<char> = settings_read
                .index_drives
                .iter()
                .filter_map(|s| s.chars().next().map(|c| c.to_ascii_uppercase()))
                .collect();
            easysearch_engine::EngineConfig::with_drives(&drives)
        }
    } else {
        easysearch_engine::EngineConfig::default()
    };
    let (engine, event_rx) = easysearch_engine::SearchEngine::with_events(engine_config);

    // Start background indexing — notify UI when ready via event channel
    // HWND is not Send, so we transmit it as a raw usize.
    let hwnd_raw = hwnd.0 as usize;
    engine.start_background();

    // Spawn a thread to listen for engine events and notify the window
    std::thread::Builder::new()
        .name("engine-events".to_string())
        .spawn(move || {
            for event in event_rx {
                let (evt_type, data_ptr) = match event {
                    easysearch_engine::EngineEvent::DriveIndexing { drive } => {
                        crate::log_write(&format!("[engine] {drive}: indexing started"));
                        let boxed = Box::new(EngineEventPayload::DriveIndexing { drive });
                        (ENGINE_EVT_DRIVE_INDEXING, Box::into_raw(boxed) as isize)
                    }
                    easysearch_engine::EngineEvent::DriveReady { drive, records, elapsed } => {
                        crate::log_write(&format!(
                            "[engine] {drive}: ready ({records} records, {:.2}s)",
                            elapsed.as_secs_f64()
                        ));
                        let boxed = Box::new(EngineEventPayload::DriveReady {
                            drive,
                            records: records as usize,
                            seconds: elapsed.as_secs_f64(),
                        });
                        (ENGINE_EVT_DRIVE_READY, Box::into_raw(boxed) as isize)
                    }
                    easysearch_engine::EngineEvent::DriveError { drive, error } => {
                        crate::log_write(&format!("[engine] {drive}: ERROR - {error}"));
                        let boxed = Box::new(EngineEventPayload::DriveError {
                            drive,
                            error: error.to_string(),
                        });
                        (ENGINE_EVT_DRIVE_ERROR, Box::into_raw(boxed) as isize)
                    }
                    easysearch_engine::EngineEvent::AllReady => {
                        crate::log_write("[engine] all drives ready, USN polling active");
                        (ENGINE_EVT_ALL_READY, 0)
                    }
                    easysearch_engine::EngineEvent::Shutdown => break,
                    easysearch_engine::EngineEvent::UsnUpdate { drive, events_applied } => {
                        if events_applied > 0 {
                            crate::log_write(&format!(
                                "[engine] {drive}: USN update applied {events_applied} events"
                            ));
                        }
                        continue;
                    }
                    easysearch_engine::EngineEvent::Log { message } => {
                        crate::log_write(&message);
                        continue;
                    }
                    _ => continue, // DriveAdded, DriveRemoved — skip for UI
                };

                unsafe {
                    let h = HWND(hwnd_raw as *mut _);
                    let _ = PostMessageW(
                        Some(h),
                        WM_ENGINE_EVENT,
                        WPARAM(evt_type),
                        LPARAM(data_ptr),
                    );
                }

                if evt_type == ENGINE_EVT_ALL_READY {
                    // Keep listening after AllReady for hot-add events
                }
            }
        })
        .ok();

    // Create engine Arc (shared with FileSearchPlugin via Router)
    let engine_for_search = std::sync::Arc::new(engine);

    // Store app state — build router AFTER engine so FileSearchPlugin can be registered
    APP_STATE.with(|state| {
        *state.borrow_mut() = Some(AppState {
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
                let settings_read = crate::SHARED_SETTINGS.get()
                    .and_then(|s| s.read().ok());
                match settings_read {
                    Some(ref s) if !s.language.is_empty() => {
                        crate::i18n::engine::I18nEngine::with_locale(&s.language)
                    }
                    _ => crate::i18n::engine::I18nEngine::new(),
                }
            },
            icon_cache: super::icon::IconCache::new(),
            anim_frame: ANIM_TOTAL_FRAMES, // Start fully visible (no animation on first load)
            engine: Some(engine_for_search.clone()),
            preview: None,
            index_status: String::new(),
            index_error: None,
            pending_ime_char_suppression: 0,
        });
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

/// Convert a PluginResult batch into DisplayItems with shortcut assignment
/// and history frequency boost applied.
#[cfg(windows)]
fn plugin_results_to_display(
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
fn resolve_display_icon_ref(
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
fn build_plugin_router(engine: Option<std::sync::Arc<easysearch_engine::SearchEngine>>) -> Router {
    let mut router = Router::new();
    router.register(Box::new(plugin_bookmark::BookmarkPlugin::new()));
    router.register(Box::new(plugin_program::ProgramPlugin::new()));
    router.register(Box::new(plugin_sys_cmd::SysCmdPlugin::new()));
    router.register(Box::new(plugin_win_settings::WinSettingsPlugin::new()));
    router.register(Box::new(plugin_quick_launch::QuickLaunchPlugin::new()));

    // FileSearchPlugin — the file search engine as a normal plugin (runs on background thread)
    if let Some(eng) = engine {
        router.register(Box::new(plugin_file_search::FileSearchPlugin::new(eng)));
    }

    // Plugin Indicator — shows keyword hints (must be registered last so it
    // only activates when no keyword-plugin claimed the query).
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

/// Build the "home screen" plugin hint list shown when the search box is empty.
///
/// Lists every enabled keyword-triggered plugin (e.g. `>`, `kill`, `b`, `s`)
/// with its name and description, matching Flow.Launcher's PluginIndicator
/// behavior. Selecting a hint fills its keyword into the input box.
#[cfg(windows)]
fn build_home_hints(router: &Router, locale: &str) -> Vec<DisplayItem> {
    router
        .plugin_infos_for_locale(locale)
        .into_iter()
        .filter(|info| info.enabled && info.keyword.as_deref().map_or(false, |k| !k.trim().is_empty()))
        .map(|info| {
            let keyword = info.keyword.clone().unwrap_or_default();
            let desc = if info.description.is_empty() {
                info.name.clone()
            } else {
                format!("{} — {}", info.name, info.description)
            };
            DisplayItem {
                title: keyword.trim().to_string(),
                subtitle: desc,
                icon: info.icon.clone(),
                shortcut: String::new(),
                // Informational hint: Enter fills the keyword into the box
                // (handled specially in handle_keydown when the query is empty).
                action: easysearch_core::Action::None,
                context_actions: Vec::new(),
                context_data: None,
                icon_path: resolve_display_icon_ref(&info.icon, &easysearch_core::Action::None, false),
                is_directory: false,
                highlight: Vec::new(),
                score: 100,
            }
        })
        .collect()
}

/// Build the combined home screen when the search box is empty:
/// top-1 recent item → plugin keyword hints → remaining recent (max 10 total).
#[cfg(windows)]
fn build_home_screen(
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

    // Plugin keyword hints in the middle.
    items.extend(build_home_hints(router, locale));

    // Remaining history items.
    for r in recent.iter().skip(1) {
        items.push(recent_to_display(r));
    }

    items
}

/// Convert a [`RecentItem`] into a [`DisplayItem`] for the home screen.
#[cfg(windows)]
fn recent_to_display(r: &super::history::RecentItem) -> DisplayItem {
    let action = history_key_to_action(&r.action_key);
    let icon_path = resolve_display_icon_ref(&r.icon, &action, r.is_directory);
    DisplayItem {
        title: r.title.clone(),
        subtitle: r.subtitle.clone(),
        icon: r.icon.clone(),
        shortcut: String::new(),
        action,
        context_actions: Vec::new(),
        context_data: None,
        icon_path,
        is_directory: r.is_directory,
        highlight: Vec::new(),
        score: 0,
    }
}

/// Inverse of [`action_to_history_key`]: parse an action key back to an [`Action`].
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

#[cfg(windows)]
fn sync_active_items(app: &mut AppState) {
    match app.view_mode {
        ViewMode::Results => {
            app.items = app.result_items.clone();
            app.selected_index = app.result_selected_index.min(app.items.len().saturating_sub(1));
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
fn open_context_actions(app: &mut AppState) -> bool {
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
    sync_active_items(app);
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
    sync_active_items(app);
    true
}

#[cfg(windows)]
fn set_active_selection(app: &mut AppState, index: usize) {
    app.selected_index = index.min(app.items.len().saturating_sub(1));
    match app.view_mode {
        ViewMode::Results => {
            app.result_selected_index = app.selected_index;
            update_preview(app);
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
fn show_native_context_menu_safe() {
    let request = APP_STATE.with(|state| {
        let Ok(s) = state.try_borrow() else { return None; };
        let Some(ref app) = *s else { return None; };
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
    });

    let Some((hwnd, path, _is_dir, point)) = request else {
        return;
    };

    let _ = super::shell_context_menu::show_for_path(hwnd, &path, Some(point));
}

/// Convert an action to a history key (non-mut version for search thread).
#[cfg(windows)]
fn action_to_history_key_static(action: &easysearch_core::Action) -> String {
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
                        APP_STATE.with(|state| {
                            if let Ok(mut s) = state.try_borrow_mut() {
                                if let Some(ref mut app) = *s {
                                    if app.pending_ime_char_suppression > 0 {
                                        app.pending_ime_char_suppression -= 1;
                                        return;
                                    }
                                    app.input.insert_char(ch);
                                    on_input_changed(app);
                                }
                            }
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
            let cursor_x = APP_STATE.with(|state| {
                if let Ok(s) = state.try_borrow() {
                    if let Some(ref app) = *s {
                        return app.renderer.measure_text_width(
                            app.input.text(),
                            app.input.cursor(),
                        );
                    }
                }
                0.0_f32 // fallback if borrow fails
            });

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
            let cursor_x = APP_STATE.with(|state| {
                if let Ok(s) = state.try_borrow() {
                    if let Some(ref app) = *s {
                        return app.renderer.measure_text_width(
                            app.input.text(),
                            app.input.cursor(),
                        );
                    }
                }
                0.0_f32
            });
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
                            APP_STATE.with(|state| {
                                if let Ok(mut s) = state.try_borrow_mut() {
                                    if let Some(ref mut app) = *s {
                                        app.pending_ime_char_suppression = app
                                            .pending_ime_char_suppression
                                            .saturating_add(text.chars().count());
                                        app.input.insert_str(&text);
                                        on_input_changed(app);
                                    }
                                }
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
            APP_STATE.with(|state| {
                if let Ok(mut s) = state.try_borrow_mut() {
                    if let Some(ref mut app) = *s {
                        app.pending_ime_char_suppression = 0;
                    }
                }
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
                let is_visible = APP_STATE.with(|state| {
                    state.borrow().as_ref().map_or(false, |app| app.visible)
                });
                if is_visible {
                    hide_window();
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
            APP_STATE.with(|state| {
                if let Ok(mut s) = state.try_borrow_mut() {
                    if let Some(ref mut app) = *s {
                        app.renderer.resize(width, height);
                    }
                }
            });
            LRESULT(0)
        }

        // ── Debounce timer ───────────────────────────────────────────────────
        WM_TIMER => {
            if wparam.0 == DEFERRED_POLL_TIMER_ID {
                // Poll for background plugin results (FileSearch)
                let should_stop = APP_STATE.with(|state| {
                    if let Ok(mut s) = state.try_borrow_mut() {
                        if let Some(ref mut app) = *s {
                            match &mut app.deferred_query {
                                Some(dq) => {
                                    match dq.rx.try_recv() {
                                        Ok(plugin_results) => {
                                            // Only accept results matching current seq_id
                                            if dq.seq_id == app.current_search_seq {
                                                let query = app.input.text().to_string();
                                                let new_items = plugin_results_to_display(
                                                    plugin_results,
                                                    &app.history,
                                                );

                                                // Apply history boost to immediate plugin items too
                                                let mut immediate_items: Vec<DisplayItem> = app.plugin_items.iter().map(|item| {
                                                    let mut item = item.clone();
                                                    let key = action_to_history_key_static(&item.action);
                                                    item.score = item.score.saturating_add(app.history.boost_score(&key));
                                                    item
                                                }).collect();

                                                // Merge all results
                                                immediate_items.extend(new_items);

                                                // Deduplicate by (title, subtitle): keep the entry with higher score
                                                {
                                                    let mut seen = std::collections::HashMap::new();
                                                    immediate_items.retain(|item| {
                                                        let key = (item.title.clone(), item.subtitle.clone());
                                                        match seen.get(&key) {
                                                            Some(&existing_score) if existing_score >= item.score => false,
                                                            _ => {
                                                                seen.insert(key, item.score);
                                                                true
                                                            }
                                                        }
                                                    });
                                                }

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
                                                    app.history.pinned_position(&query, &key).unwrap_or(usize::MAX)
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
                                    }
                                }
                                None => true, // no deferred query, stop timer
                            }
                        } else {
                            true
                        }
                    } else {
                        true
                    }
                });

                if should_stop {
                    APP_STATE.with(|state| {
                        if let Ok(mut s) = state.try_borrow_mut() {
                            if let Some(ref mut app) = *s {
                                app.deferred_query = None;
                            }
                        }
                    });
                    unsafe { let _ = KillTimer(Some(hwnd), DEFERRED_POLL_TIMER_ID); }
                }
                do_render();
            } else if wparam.0 == ANIM_TIMER_ID {
                // Advance animation frame
                let done = APP_STATE.with(|state| {
                    if let Ok(mut s) = state.try_borrow_mut() {
                        if let Some(ref mut app) = *s {
                            app.anim_frame = app.anim_frame.saturating_add(1);
                            app.anim_frame >= ANIM_TOTAL_FRAMES
                        } else {
                            true
                        }
                    } else {
                        true
                    }
                });
                if done {
                    unsafe { let _ = KillTimer(Some(hwnd), ANIM_TIMER_ID); }
                }
                do_render();
            } else if wparam.0 == SETTINGS_POLL_TIMER_ID {
                poll_settings_changes(hwnd);
            }
            LRESULT(0)
        }

        // ── Index ready notification ─────────────────────────────────────────
        m if m == WM_INDEX_READY => {
            APP_STATE.with(|state| {
                if let Ok(mut s) = state.try_borrow_mut() {
                    if let Some(ref mut app) = *s {
                        app.index_ready = true;
                        app.index_status.clear();
                        app.index_error = None;
                    }
                }
            });
            do_render();
            LRESULT(0)
        }

        // ── Engine progress/error events ─────────────────────────────────────
        m if m == WM_ENGINE_EVENT => {
            let evt_type = wparam.0;
            let data_ptr = lparam.0;

            APP_STATE.with(|state| {
                if let Ok(mut s) = state.try_borrow_mut() {
                    if let Some(ref mut app) = *s {
                        match evt_type {
                            ENGINE_EVT_DRIVE_INDEXING => {
                                let msg =
                                    unsafe { Box::from_raw(data_ptr as *mut EngineEventPayload) };
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
                                let msg =
                                    unsafe { Box::from_raw(data_ptr as *mut EngineEventPayload) };
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
                                let msg =
                                    unsafe { Box::from_raw(data_ptr as *mut EngineEventPayload) };
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
                        }
                    }
                }
            });
            do_render();
            LRESULT(0)
        }

        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }

        // ── Theme change detection ──────────────────────────────────────────
        WM_SETTINGCHANGE => {
            // Windows broadcasts WM_SETTINGCHANGE when system theme changes.
            // Re-detect theme and update renderer.
            APP_STATE.with(|state| {
                if let Ok(mut s) = state.try_borrow_mut() {
                    if let Some(ref mut app) = *s {
                        app.renderer.theme = crate::theme::Theme::system();
                    }
                }
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
                tray::IDM_EXIT => {
                    unsafe { DestroyWindow(hwnd).ok(); }
                }
                _ => {}
            }
            LRESULT(0)
        }

        WM_LBUTTONUP | WM_RBUTTONUP => {
            let x = (lparam.0 as i16) as i32;
            let y = ((lparam.0 >> 16) as i16) as i32;
            let is_right_click = msg == WM_RBUTTONUP;

            let deferred = APP_STATE.with(|state| {
                let Ok(mut s) = state.try_borrow_mut() else { return 0u8; };
                let Some(ref mut app) = *s else { return 0u8; };
                let Some(index) = item_index_from_client_point(app, x, y) else {
                    return 0u8;
                };

                set_active_selection(app, index);
                if is_right_click && app.view_mode == ViewMode::Results {
                    2
                } else {
                    1
                }
            });

            match deferred {
                1 => execute_selected_safe(),
                2 => {
                    APP_STATE.with(|state| {
                        if let Ok(mut s) = state.try_borrow_mut() {
                            if let Some(ref mut app) = *s {
                                let _ = open_context_actions(app);
                            }
                        }
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
    let vk = wparam.0 as u16;
    let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
    let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
    let alt = unsafe { GetKeyState(VK_MENU.0 as i32) } < 0;

    // Deferred actions that require Win32 calls (must be done AFTER releasing borrow)
    enum DeferredAction {
        None,
        Hide,
        Execute,
        OpenFolder,
        OpenContext,
        CloseContext,
        ShowNativeContextMenu,
    }

    let deferred = APP_STATE.with(|state| {
        let Ok(mut s) = state.try_borrow_mut() else { return DeferredAction::None; };
        let Some(ref mut app) = *s else { return DeferredAction::None; };

        match vk {
            // Escape — hide window
            v if v == VK_ESCAPE.0 => {
                return if app.view_mode == ViewMode::ContextActions {
                    DeferredAction::CloseContext
                } else {
                    DeferredAction::Hide
                };
            }
            // Enter — execute selected item; Ctrl+Enter — open containing folder
            v if v == VK_RETURN.0 => {
                if alt {
                    return DeferredAction::ShowNativeContextMenu;
                }
                if shift {
                    return DeferredAction::OpenContext;
                }
                if ctrl {
                    return if app.view_mode == ViewMode::Results {
                        DeferredAction::OpenFolder
                    } else {
                        DeferredAction::Execute
                    };
                }
                // Home-screen hint: fill the selected plugin's keyword into the
                // input box (so the user can keep typing) instead of executing.
                if app.view_mode == ViewMode::Results
                    && app.input.text().trim().is_empty()
                    && !app.items.is_empty()
                {
                    let idx = app.selected_index.min(app.items.len() - 1);
                    let keyword = app.items[idx].title.trim().to_string();
                    if !keyword.is_empty() {
                        app.input.set_text(&format!("{keyword} "));
                        app.input.move_end(false);
                        on_input_changed(app);
                        return DeferredAction::None;
                    }
                }
                return DeferredAction::Execute;
            }
            v if ctrl && v == 0x4F => {
                return DeferredAction::OpenContext;
            }
            // Up arrow — select previous (wrap to bottom)
            v if v == VK_UP.0 => {
                if app.items.is_empty() {
                    // no-op
                } else if app.selected_index > 0 {
                    app.selected_index -= 1;
                } else {
                    app.selected_index = app.items.len() - 1;
                }
                match app.view_mode {
                    ViewMode::Results => {
                        app.result_selected_index = app.selected_index;
                        update_preview(app);
                    }
                    ViewMode::ContextActions => {
                        app.context_selected_index = app.selected_index;
                    }
                }
            }
            // Down arrow — select next (wrap to top)
            v if v == VK_DOWN.0 => {
                if !app.items.is_empty() {
                    if app.selected_index < app.items.len() - 1 {
                        app.selected_index += 1;
                    } else {
                        app.selected_index = 0;
                    }
                }
                match app.view_mode {
                    ViewMode::Results => {
                        app.result_selected_index = app.selected_index;
                        update_preview(app);
                    }
                    ViewMode::ContextActions => {
                        app.context_selected_index = app.selected_index;
                    }
                }
            }
            // Backspace
            v if v == VK_BACK.0 => {
                app.input.backspace();
                on_input_changed(app);
            }
            // Delete
            v if v == VK_DELETE.0 => {
                app.input.delete();
                on_input_changed(app);
            }
            // Home
            v if v == VK_HOME.0 => {
                app.input.move_home(shift);
            }
            // End
            v if v == VK_END.0 => {
                app.input.move_end(shift);
            }
            // Left
            v if v == VK_LEFT.0 => {
                if app.view_mode == ViewMode::ContextActions {
                    return DeferredAction::CloseContext;
                }
                app.input.move_left(shift);
            }
            // Right
            v if v == VK_RIGHT.0 => {
                if app.view_mode == ViewMode::Results {
                    return DeferredAction::OpenContext;
                }
                app.input.move_right(shift);
            }
            // Ctrl+A — select all
            v if ctrl && v == 0x41 => {
                app.input.select_all();
            }
            // Ctrl+V — paste from clipboard
            v if ctrl && v == 0x56 => {
                if let Some(text) = clipboard_get_text(app.hwnd) {
                    if !text.is_empty() {
                        app.input.insert_str(&text);
                        on_input_changed(app);
                    }
                }
            }
            // Ctrl+C — copy selection to clipboard
            v if ctrl && v == 0x43 => {
                if app.input.has_selection() {
                    let selected = app.input.selected_text().to_string();
                    clipboard_set_text(app.hwnd, &selected);
                }
            }
            // Ctrl+X — cut selection to clipboard
            v if ctrl && v == 0x58 => {
                if app.input.has_selection() {
                    let selected = app.input.selected_text().to_string();
                    clipboard_set_text(app.hwnd, &selected);
                    app.input.backspace(); // delete_selection via backspace
                    on_input_changed(app);
                }
            }
            // Tab — autocomplete with selected item's title
            v if v == 0x09 => {
                if !app.items.is_empty() {
                    let idx = app.selected_index.min(app.items.len() - 1);
                    let title = app.items[idx].title.clone();
                    if !title.is_empty() {
                        app.input.set_text(&title);
                        on_input_changed(app);
                    }
                }
            }
            _ => {}
        }
        DeferredAction::None
    });

    // Execute deferred actions AFTER borrow is released (Win32 calls can re-enter)
    match deferred {
        DeferredAction::Hide => {
            // Safe to call — will do try_borrow_mut internally
            hide_window();
        }
        DeferredAction::Execute => {
            // Extract action, hide, then execute — all outside the borrow
            execute_selected_safe();
        }
        DeferredAction::OpenFolder => {
            // Open the containing folder of the selected item
            open_containing_folder_safe();
        }
        DeferredAction::OpenContext => {
            APP_STATE.with(|state| {
                if let Ok(mut s) = state.try_borrow_mut() {
                    if let Some(ref mut app) = *s {
                        let _ = open_context_actions(app);
                    }
                }
            });
        }
        DeferredAction::CloseContext => {
            APP_STATE.with(|state| {
                if let Ok(mut s) = state.try_borrow_mut() {
                    if let Some(ref mut app) = *s {
                        let _ = close_context_actions(app);
                    }
                }
            });
        }
        DeferredAction::ShowNativeContextMenu => {
            show_native_context_menu_safe();
        }
        DeferredAction::None => {}
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
    let query = app.input.text().to_string();
    app.view_mode = ViewMode::Results;
    app.selected_index = 0;
    app.result_selected_index = 0;
    app.context_items.clear();
    app.context_selected_index = 0;
    app.context_source_index = None;
    app.current_search_seq += 1;
    app.preview = None; // Clear preview on new input

    // Cancel any pending deferred query
    app.deferred_query = None;

    if query.trim().is_empty() {
        // Home screen: top-1 recent → plugin hints → remaining recent (max 10).
        app.plugin_items.clear();
        app.result_items = build_home_screen(
            &app.history,
            &app.plugin_router,
            app.i18n.current_locale(),
        );
        app.anim_frame = ANIM_TOTAL_FRAMES;
    } else {
        // ── Fast plugins: execute immediately (synchronous, < 1ms) ──────────
        let (immediate_results, keyword_matched) = app.plugin_router.query_immediate(&query);
        app.plugin_items = plugin_results_to_display(immediate_results, &app.history);
        app.result_items = app.plugin_items.clone();

        // Skip animation — show immediately
        app.anim_frame = ANIM_TOTAL_FRAMES;

        // ── Background plugins: dispatch immediately, cancel old via Drop ──
        // Replacing `deferred_query` drops the old one → its CancelToken flips
        // → the old thread exits early at its next check point.
        let current_seq = app.current_search_seq;
        if let Some((deferred_rx, cancel)) = app.plugin_router.query_background(&query, keyword_matched) {
            app.deferred_query = Some(DeferredQuery {
                rx: deferred_rx,
                seq_id: current_seq,
                cancel,
            });
            // Start poll timer to check for results
            unsafe {
                let _ = SetTimer(Some(app.hwnd), DEFERRED_POLL_TIMER_ID, DEFERRED_POLL_MS, None);
            }
        }
    }

    sync_active_items(app);

    // Resize window based on results
    resize_for_results(app);
}

/// Resize window to fit the current number of results.
#[cfg(windows)]
fn resize_for_results(app: &mut AppState) {
    let has_preview = app.preview.is_some() && !app.items.is_empty();
    let height = layout::window_height_with_preview(app.items.len(), has_preview) as i32;
    let width = layout::WINDOW_WIDTH as i32;

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

/// Toggle window visibility.
#[cfg(windows)]
fn toggle_visibility() {
    crate::log!("toggle_visibility called");

    // Read current visibility state without holding borrow during Win32 calls
    let (is_visible, hwnd) = APP_STATE.with(|state| {
        let s = state.borrow();
        match s.as_ref() {
            Some(app) => (app.visible, app.hwnd),
            None => (false, HWND::default()),
        }
    });

    if is_visible {
        crate::log!("  -> hiding window");
        hide_window();
    } else {
        crate::log!("  -> showing window");
        // Show: Win32 calls in show_window can trigger re-entrant messages,
        // so we must NOT hold the borrow_mut during those calls.
        show_window_safe(hwnd);
    }
}

/// Show the window — safe version that avoids RefCell re-entrancy.
/// Win32 calls (SetForegroundWindow, SetFocus) can trigger re-entrant messages
/// (IME initialization, WM_PAINT), so we must NOT hold the RefCell borrow.
#[cfg(windows)]
fn show_window_safe(hwnd: HWND) {
    crate::log!("show_window_safe: start");

    // Populate the home-screen plugin hints if the box is empty, and get the
    // resulting item count (used to size the window below).
    let item_count = APP_STATE.with(|state| {
        if let Ok(mut s) = state.try_borrow_mut() {
            if let Some(ref mut app) = *s {
                if app.input.text().trim().is_empty() {
                    app.result_items = build_home_screen(
                        &app.history,
                        &app.plugin_router,
                        app.i18n.current_locale(),
                    );
                    app.plugin_items.clear();
                    app.result_selected_index = 0;
                    sync_active_items(app);
                    app.anim_frame = ANIM_TOTAL_FRAMES;
                }
                return app.items.len();
            }
        }
        0
    });

    unsafe {
        // Multi-monitor support: show on the active monitor
        unsafe extern "system" {
            fn GetCursorPos(lp_point: *mut windows::Win32::Foundation::POINT) -> i32;
        }

        let mut cursor_pos = windows::Win32::Foundation::POINT { x: 0, y: 0 };
        GetCursorPos(&mut cursor_pos);
        crate::log!("show_window_safe: cursor at ({}, {})", cursor_pos.x, cursor_pos.y);

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

        let width = layout::WINDOW_WIDTH as i32;
        let height = layout::window_height(item_count) as i32;
        let x = work.left + (mon_width - width) / 2;
        let y = work.top + mon_height / 4;

        crate::log!("show_window_safe: SetWindowPos x={x} y={y} w={width} h={height}");
        let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), x, y, width, height, SWP_NOACTIVATE);

        crate::log!("show_window_safe: ShowWindow(SW_SHOW)");
        let _ = ShowWindow(hwnd, SW_SHOW);

        // Force foreground focus using AttachThreadInput trick.
        // SetForegroundWindow can silently fail if the calling thread doesn't
        // own the foreground lock. By attaching to the foreground window's thread
        // first, we inherit its foreground rights.
        crate::log!("show_window_safe: force foreground focus");
        force_foreground(hwnd);

        crate::log!("show_window_safe: SetFocus");
        let _ = SetFocus(Some(hwnd));

        // Ensure IME context is properly associated with the focused window.
        // Some IMEs lose context when a WS_POPUP window regains focus.
        {
            let himc = ImmGetContext(hwnd);
            if !himc.is_invalid() {
                let _ = ImmReleaseContext(hwnd, himc);
            }
        }
    }

    // Now it's safe to set visible=true (Win32 re-entrant messages have been processed)
    APP_STATE.with(|state| {
        if let Ok(mut s) = state.try_borrow_mut() {
            if let Some(ref mut app) = *s {
                app.visible = true;
            }
        }
    });

    // Render the home-screen hints (or whatever items exist) immediately.
    do_render();
    crate::log!("show_window_safe: done, visible=true");
}

/// Force a window to the foreground, working around Windows' restrictions.
///
/// `SetForegroundWindow` can silently fail if the calling thread doesn't hold
/// the "foreground lock." The standard workaround is to temporarily attach our
/// input queue to the thread that currently owns the foreground window; this
/// gives us the right to call `SetForegroundWindow` successfully.
#[cfg(windows)]
unsafe fn force_foreground(hwnd: HWND) {
    unsafe {
        let foreground = GetForegroundWindow();
        let our_tid = GetCurrentThreadId();

        if foreground.0 == std::ptr::null_mut() || foreground == hwnd {
            // No foreground window or we're already it — simple path
            let _ = SetForegroundWindow(hwnd);
            return;
        }

        let fg_tid = GetWindowThreadProcessId(foreground, None);

        if fg_tid != our_tid && fg_tid != 0 {
            // Attach to the foreground thread's input queue
            let _ = AttachThreadInput(our_tid, fg_tid, true);
            let _ = SetForegroundWindow(hwnd);
            let _ = AttachThreadInput(our_tid, fg_tid, false);
        } else {
            let _ = SetForegroundWindow(hwnd);
        }
    }
}

/// Show the window with fade-in animation.
/// Flow.Launcher uses CircleEase animation, 160-560ms.
/// We use AnimateWindow(AW_BLEND) for similar effect.
#[cfg(windows)]
#[allow(dead_code)]
fn show_window(app: &mut AppState) {
    unsafe {
        // Multi-monitor support: show on the active monitor
        unsafe extern "system" {
            fn GetCursorPos(lp_point: *mut windows::Win32::Foundation::POINT) -> i32;
        }

        let mut cursor_pos = windows::Win32::Foundation::POINT { x: 0, y: 0 };
        GetCursorPos(&mut cursor_pos);
        crate::log!("show_window: cursor at ({}, {})", cursor_pos.x, cursor_pos.y);

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

        let width = layout::WINDOW_WIDTH as i32;
        let height = layout::window_height(app.items.len()) as i32;
        let x = work.left + (mon_width - width) / 2;
        let y = work.top + mon_height / 4;

        crate::log!("show_window: SetWindowPos x={x} y={y} w={width} h={height}");
        let _ = SetWindowPos(app.hwnd, Some(HWND_TOPMOST), x, y, width, height, SWP_NOACTIVATE);

        crate::log!("show_window: ShowWindow(SW_SHOW)");
        let _ = ShowWindow(app.hwnd, SW_SHOW);

        crate::log!("show_window: force foreground focus");
        force_foreground(app.hwnd);

        crate::log!("show_window: SetFocus");
        let _ = SetFocus(Some(app.hwnd));
    }
    app.visible = true;
    crate::log!("show_window: done, visible=true");
}

/// Hide the window and clear input.
#[cfg(windows)]
fn hide_window() {
    // Step 1: Update state (brief borrow, no Win32 calls)
    let hwnd = APP_STATE.with(|state| {
        let Ok(mut s) = state.try_borrow_mut() else { return None; };
        let Some(ref mut app) = *s else { return None; };
        if !app.visible {
            return None;
        }
        let h = app.hwnd;
        app.visible = false;
        app.input.clear();
        app.items.clear();
        app.result_items.clear();
        app.context_items.clear();
        app.plugin_items.clear();
        app.deferred_query = None;
        app.selected_index = 0;
        app.result_selected_index = 0;
        app.context_selected_index = 0;
        app.context_source_index = None;
        app.view_mode = ViewMode::Results;
        app.pending_ime_char_suppression = 0;
        Some(h)
    });

    // Step 2: Call Win32 APIs AFTER releasing the borrow
    if let Some(hwnd) = hwnd {
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
            let _ = KillTimer(Some(hwnd), DEFERRED_POLL_TIMER_ID);
        }
    }
}

/// Hide with fade-out animation (old version — only call when already holding borrow
/// and you know ShowWindow won't be needed, or refactor the caller).
#[cfg(windows)]
#[allow(dead_code)]
fn hide_window_inner(app: &mut AppState) {
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
    app.selected_index = 0;
    app.result_selected_index = 0;
    app.context_selected_index = 0;
    app.context_source_index = None;
    app.pending_ime_char_suppression = 0;
    app.view_mode = ViewMode::Results;
}

/// Execute the currently selected item.
/// Execute the currently selected item — safe version that avoids RefCell re-entrancy.
/// Extracts the action with a brief borrow, then releases it before calling Win32 APIs.
#[cfg(windows)]
fn execute_selected_safe() {
    // Step 1: Extract the action and record history while briefly borrowing
    let item = APP_STATE.with(|state| {
        let Ok(mut s) = state.try_borrow_mut() else { return None; };
        let Some(ref mut app) = *s else { return None; };
        if app.items.is_empty() {
            return None;
        }
        let idx = app.selected_index.min(app.items.len() - 1);
        let item = app.items[idx].clone();

        // Record usage with full metadata for the home-screen recent panel.
        let history_key = action_to_history_key(&item.action);
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
    });

    let Some(item) = item else { return; };

    // Step 2: Hide window (outside borrow — ShowWindow can trigger re-entrant messages)

    // Step 3: Execute the action (outside borrow — ShellExecute etc.)
    execute_action_safe(item.action, item.title, item.context_data);
}

/// Open the containing folder of the currently selected item (Ctrl+Enter).
/// Uses Explorer's `/select,` syntax to highlight the file in its parent folder.
#[cfg(windows)]
fn open_containing_folder_safe() {
    // Step 1: Extract the file path from the selected item
    let path = APP_STATE.with(|state| {
        let Ok(s) = state.try_borrow() else { return None; };
        let Some(ref app) = *s else { return None; };
        if app.items.is_empty() {
            return None;
        }
        let idx = app.selected_index.min(app.items.len() - 1);
        app.items[idx]
            .context_data
            .as_ref()
            .map(|data| data.file_path.clone())
            .or_else(|| match &app.items[idx].action {
                easysearch_core::Action::Open(p)
                | easysearch_core::Action::OpenAsAdmin(p) => Some(p.clone()),
                _ => None,
            })
    });

    let Some(path) = path else { return; };

    // Step 2: Hide window
    hide_window();

    // Step 3: Open Explorer with the file selected
    super::fs_actions::open_containing_folder(&path);
}

#[cfg(windows)]
fn execute_action_safe(
    action: easysearch_core::Action,
    _title: String,
    context_data: Option<easysearch_core::ContextData>,
) {
    match action {
        easysearch_core::Action::DaemonSearch(query) => {
            APP_STATE.with(|state| {
                if let Ok(mut s) = state.try_borrow_mut() {
                    if let Some(ref mut app) = *s {
                        app.input.set_text(&query);
                        app.input.move_end(false);
                        on_input_changed(app);
                    }
                }
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
            APP_STATE.with(|state| {
                if let Ok(mut s) = state.try_borrow_mut() {
                    if let Some(ref mut app) = *s {
                        app.input.set_text(&query);
                        app.input.move_end(false);
                        on_input_changed(app);
                    }
                }
            });
        }
        easysearch_core::Action::ToggleQuickLaunch { path, title: item_title } => {
            let is_dir = context_data
                .as_ref()
                .map(|data| data.is_directory)
                .unwrap_or_else(|| super::fs_actions::is_directory(&path));
            {
                let mut store = global_store().lock().unwrap_or_else(|err| err.into_inner());
                let _ = store.toggle(&path, &item_title, is_dir);
                let _ = store.save();
            }

            APP_STATE.with(|state| {
                if let Ok(mut s) = state.try_borrow_mut() {
                    if let Some(ref mut app) = *s {
                        let was_context = app.view_mode == ViewMode::ContextActions;
                        let source_index = app.context_source_index;
                        on_input_changed(app);
                        if was_context {
                            if let Some(index) = source_index {
                                app.result_selected_index = index.min(app.result_items.len().saturating_sub(1));
                                sync_active_items(app);
                                let _ = open_context_actions(app);
                            }
                        }
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
                easysearch_core::Action::None
                    | easysearch_core::Action::ShowFileContextMenu { .. }
            );
            if should_hide {
                hide_window();
            }
            super::action::execute(&other);
        }
    }
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
    let history_key = action_to_history_key(&action);
    app.history.record(&history_key);
    app.history.save();

    // Hide window first
    hide_window_inner(app);

    // Execute the action
    super::action::execute(&action);
}

/// Convert an action to a history key string.
#[cfg(windows)]
fn action_to_history_key(action: &easysearch_core::Action) -> String {
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

/// Update the preview info for the currently selected item.
#[cfg(windows)]
fn update_preview(app: &mut AppState) {
    let old_has_preview = app.preview.is_some();

    if app.items.is_empty() {
        app.preview = None;
    } else {
        let idx = app.selected_index.min(app.items.len() - 1);
        // Only load preview for file/folder items (those with a path)
        if let Some(ref path) = app.items[idx].icon_path {
            app.preview = super::preview::PreviewInfo::from_path(path);
        } else {
            app.preview = None;
        }
    }

    // Resize window if preview visibility changed
    let new_has_preview = app.preview.is_some();
    if old_has_preview != new_has_preview {
        resize_for_results(app);
    }
}

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

    // Apply changes
    let mut changed = false;

    APP_STATE.with(|state| {
        let Ok(mut s) = state.try_borrow_mut() else { return; };
        let Some(ref mut app) = *s else { return; };

        // Theme
        if on_disk.theme != current.theme {
            app.renderer.theme = match on_disk.theme.as_str() {
                "Win11Light" => crate::theme::Theme::light(),
                "Win11Dark" => crate::theme::Theme::dark(),
                _ => crate::theme::Theme::system(),
            };
            changed = true;
        }

        // Language
        if on_disk.language != current.language {
            let locale = if on_disk.language.is_empty() {
                crate::i18n::engine::I18nEngine::detect_system_locale()
            } else {
                on_disk.language.clone()
            };
            app.i18n.set_locale(&locale);
            easysearch_core::context_labels::set_locale(&locale);
            app.plugin_router = build_plugin_router(app.engine.clone());
            changed = true;
        }

        // Hotkey
        if on_disk.hotkey != current.hotkey {
            hotkey::unregister(hwnd);
            if let Some((modifiers, vk)) = parse_hotkey_string(&on_disk.hotkey) {
                unsafe {
                    use windows::Win32::UI::Input::KeyboardAndMouse::{
                        RegisterHotKey, HOT_KEY_MODIFIERS,
                    };
                    let _ = RegisterHotKey(
                        Some(hwnd),
                        hotkey::HOTKEY_ID,
                        HOT_KEY_MODIFIERS(modifiers),
                        vk,
                    );
                }
            } else {
                hotkey::register(hwnd);
            }
            changed = true;
        }

        // Drives
        if on_disk.index_drives != current.index_drives {
            if let Some(ref engine) = app.engine {
                let new_drives: Vec<char> = on_disk.index_drives.iter()
                    .filter_map(|s| s.chars().next().map(|c| c.to_ascii_uppercase()))
                    .collect();

                let current_labels = engine.drive_labels();
                let current_drives: Vec<char> = current_labels
                    .iter()
                    .filter_map(|s| s.chars().next())
                    .collect();

                for &d in &new_drives {
                    if !current_drives.contains(&d) {
                        engine.add_drive(d);
                    }
                }
                for &d in &current_drives {
                    if !new_drives.contains(&d) {
                        engine.remove_drive(d);
                    }
                }
            }
            changed = true;
        }

        // Autostart
        if on_disk.autostart != current.autostart {
            #[cfg(windows)]
            {
                if on_disk.autostart {
                    let _ = crate::shared::autostart::enable();
                } else {
                    let _ = crate::shared::autostart::disable();
                }
            }
            changed = true;
        }
    });

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

/// Parse a hotkey string like "Alt+Space", "Ctrl+Shift+F" into (modifiers, vk_code).
/// Returns None if the string can't be parsed.
#[cfg(windows)]
fn parse_hotkey_string(s: &str) -> Option<(u32, u32)> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN};

    let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
    if parts.is_empty() {
        return None;
    }

    let mut modifiers: u32 = 0;
    let mut key_part = "";

    for part in &parts {
        match part.to_lowercase().as_str() {
            "alt" => modifiers |= MOD_ALT.0,
            "ctrl" | "control" => modifiers |= MOD_CONTROL.0,
            "shift" => modifiers |= MOD_SHIFT.0,
            "win" | "super" => modifiers |= MOD_WIN.0,
            _ => key_part = part,
        }
    }

    let vk = match key_part.to_lowercase().as_str() {
        "space" => 0x20u32,
        "enter" | "return" => 0x0D,
        "tab" => 0x09,
        "escape" | "esc" => 0x1B,
        "backspace" => 0x08,
        "delete" | "del" => 0x2E,
        "insert" | "ins" => 0x2D,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" | "pgup" => 0x21,
        "pagedown" | "pgdn" => 0x22,
        "up" => 0x26,
        "down" => 0x28,
        "left" => 0x25,
        "right" => 0x27,
        s if s.len() == 1 => {
            let c = s.chars().next()?;
            if c.is_ascii_alphabetic() {
                c.to_ascii_uppercase() as u32
            } else if c.is_ascii_digit() {
                c as u32
            } else {
                return None;
            }
        }
        s if s.starts_with('f') && s[1..].parse::<u32>().is_ok() => {
            let n: u32 = s[1..].parse().ok()?;
            if n >= 1 && n <= 24 {
                0x6F + n // VK_F1 = 0x70
            } else {
                return None;
            }
        }
        _ => return None,
    };

    Some((modifiers, vk))
}

/// Trigger a re-render.
#[cfg(windows)]
fn do_render() {
    APP_STATE.with(|state| {
        // Use try_borrow_mut to avoid panic on re-entrant calls
        let Ok(mut state) = state.try_borrow_mut() else {
            // Already borrowed — we're in a re-entrant call, skip this render
            return;
        };
        if let Some(ref mut app) = *state {
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

            // Prepare preview info to pass into the single render frame
            let preview_param = if let Some(ref preview) = app.preview {
                if !app.items.is_empty() {
                    let results_height = layout::window_height(app.items.len());
                    Some((preview, results_height))
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
                preview_param,
            );
        }
    });
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
        DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE,
        DWMWCP_ROUND,
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

/// Get text from the system clipboard.
#[cfg(windows)]
fn clipboard_get_text(hwnd: HWND) -> Option<String> {
    use windows::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, OpenClipboard,
    };
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    use windows::Win32::System::Ole::CF_UNICODETEXT;

    unsafe {
        if OpenClipboard(Some(hwnd)).is_err() {
            return None;
        }

        let result = (|| -> Option<String> {
            let handle = GetClipboardData(CF_UNICODETEXT.0 as u32).ok()?;
            // HANDLE and HGLOBAL are both pointer-sized, reinterpret for GlobalLock
            let hglobal = windows::Win32::Foundation::HGLOBAL(handle.0 as *mut _);
            let ptr = GlobalLock(hglobal) as *const u16;
            if ptr.is_null() {
                return None;
            }

            // Find null terminator
            let mut len = 0;
            while *ptr.add(len) != 0 {
                len += 1;
            }

            let slice = std::slice::from_raw_parts(ptr, len);
            let text = String::from_utf16_lossy(slice);

            let _ = GlobalUnlock(hglobal);
            Some(text)
        })();

        let _ = CloseClipboard();
        result
    }
}

/// Set text to the system clipboard.
#[cfg(windows)]
fn clipboard_set_text(hwnd: HWND, text: &str) {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
    };
    use windows::Win32::System::Ole::CF_UNICODETEXT;

    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let byte_len = wide.len() * 2;

    unsafe {
        if OpenClipboard(Some(hwnd)).is_err() {
            return;
        }

        let _ = EmptyClipboard();

        if let Ok(hmem) = GlobalAlloc(GMEM_MOVEABLE, byte_len) {
            let ptr = GlobalLock(hmem) as *mut u16;
            if !ptr.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
                let _ = GlobalUnlock(hmem);

                // SetClipboardData takes ownership of the memory handle
                let handle = HANDLE(hmem.0 as *mut _);
                let _ = SetClipboardData(CF_UNICODETEXT.0 as u32, Some(handle));
            }
        }

        let _ = CloseClipboard();
    }
}
