// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Application state definition and thread-local access helpers.

#[cfg(windows)]
use std::cell::RefCell;

#[cfg(windows)]
use windows::Win32::Foundation::HWND;

#[cfg(windows)]
use easysearch_core::Router;

#[cfg(windows)]
use super::input::InputState;
#[cfg(windows)]
use super::renderer::{DisplayItem, Renderer};

// ─── Types ──────────────────────────────────────────────────────────────────

/// Pending background (deferred) results from plugins like FileSearch.
/// On drop, the cancel token is set to abort the background thread.
#[cfg(windows)]
pub(super) struct DeferredQuery {
    pub(super) rx: std::sync::mpsc::Receiver<Vec<easysearch_core::PluginResult>>,
    pub(super) seq_id: u64,
    #[allow(dead_code)]
    pub(super) cancel: easysearch_core::CancelToken,
}

#[cfg(windows)]
impl Drop for DeferredQuery {
    fn drop(&mut self) {
        use std::sync::atomic::Ordering;
        self.cancel.store(true, Ordering::Relaxed);
    }
}

#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ViewMode {
    Results,
    ContextActions,
}

/// Central application state for the search window.
#[cfg(windows)]
pub(super) struct AppState {
    pub(super) hwnd: HWND,
    pub(super) renderer: Renderer,
    pub(super) input: InputState,
    pub(super) view_mode: ViewMode,
    pub(super) items: Vec<DisplayItem>,
    pub(super) selected_index: usize,
    pub(super) result_items: Vec<DisplayItem>,
    pub(super) result_selected_index: usize,
    pub(super) context_items: Vec<DisplayItem>,
    pub(super) context_selected_index: usize,
    pub(super) context_source_index: Option<usize>,
    pub(super) visible: bool,
    pub(super) plugin_router: Router,
    /// Items from local plugins (shown immediately).
    pub(super) plugin_items: Vec<DisplayItem>,
    /// Pending deferred (background) query. Polled via timer.
    pub(super) deferred_query: Option<DeferredQuery>,
    /// Current search sequence ID (incremented on each input change).
    pub(super) current_search_seq: u64,
    /// Whether the file search index is ready.
    pub(super) index_ready: bool,
    /// Usage history for frequency-based ranking.
    pub(super) history: super::history::History,
    /// Internationalization strings.
    pub(super) i18n: crate::i18n::engine::I18nEngine,
    /// Icon cache for rendering file/folder icons.
    pub(super) icon_cache: super::icon::IconCache,
    /// Animation progress for result list (0 = start, ANIM_TOTAL_FRAMES = done).
    pub(super) anim_frame: u8,
    /// True while a debounced or background search is in flight.
    pub(super) search_active: bool,
    /// Last window size applied via SetWindowPos, used to avoid redundant resizes.
    pub(super) last_window_size: (i32, i32),
    /// Engine reference for hot-plug drive management.
    pub(super) engine: Option<std::sync::Arc<easysearch_engine::SearchEngine>>,
    /// Preview info loaded asynchronously when context actions are opened.
    pub(super) preview: Option<super::preview::PreviewInfo>,
    /// Sequence ID for async preview loading (to discard stale results).
    pub(super) preview_seq: u64,
    /// Index progress status text (e.g. "Indexing C:..." or error messages).
    pub(super) index_status: String,
    /// Last indexing error message (if any).
    pub(super) index_error: Option<String>,
    /// Number of committed IME chars whose follow-up `WM_CHAR` messages should
    /// be ignored to avoid duplicating CJK input.
    pub(super) pending_ime_char_suppression: usize,
    /// Whether focus is on the input box (true) or the result list (false).
    /// When true, arrow left/right move cursor; up does nothing; only down
    /// transfers focus to results. Mirrors Flow.Launcher's behavior.
    pub(super) input_focused: bool,
    /// Timestamp (millis since epoch) when the cursor was last moved,
    /// used to keep the cursor visible immediately after movement.
    pub(super) cursor_moved_at: u128,
}

// ─── Thread-local storage ───────────────────────────────────────────────────

#[cfg(windows)]
thread_local! {
    static APP_STATE: RefCell<Option<AppState>> = const { RefCell::new(None) };
}

// ─── Access helpers ─────────────────────────────────────────────────────────

/// Execute a closure with mutable access to the app state.
/// Returns `None` if the state is not initialized or already borrowed.
#[cfg(windows)]
pub(super) fn with_app_mut<R>(f: impl FnOnce(&mut AppState) -> R) -> Option<R> {
    APP_STATE.with(|state| {
        let Ok(mut s) = state.try_borrow_mut() else {
            return None;
        };
        let Some(ref mut app) = *s else {
            return None;
        };
        Some(f(app))
    })
}

/// Execute a closure with shared (read-only) access to the app state.
/// Returns `None` if the state is not initialized or already mutably borrowed.
#[cfg(windows)]
pub(super) fn with_app_ref<R>(f: impl FnOnce(&AppState) -> R) -> Option<R> {
    APP_STATE.with(|state| {
        let Ok(s) = state.try_borrow() else {
            return None;
        };
        let Some(ref app) = *s else {
            return None;
        };
        Some(f(app))
    })
}

/// Initialize the app state. Called once during window creation.
#[cfg(windows)]
pub(super) fn init(app: AppState) {
    APP_STATE.with(|state| {
        *state.borrow_mut() = Some(app);
    });
}
