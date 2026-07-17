// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Win32 message ids and payloads shared by the search window.

#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::WM_APP;

/// Custom message: index build is complete.
#[cfg(windows)]
pub(crate) const WM_INDEX_READY: u32 = WM_APP + 2;

/// Custom message: engine event (progress / error) - wparam encodes the event type.
#[cfg(windows)]
pub(crate) const WM_ENGINE_EVENT: u32 = WM_APP + 3;

/// Custom message: preview info loaded from background thread.
#[cfg(windows)]
pub(crate) const WM_PREVIEW_READY: u32 = WM_APP + 4;

#[cfg(windows)]
pub(crate) const WM_ICON_READY: u32 = WM_APP + 5;

/// Deferred result poll timer ID.
#[cfg(windows)]
pub(crate) const DEFERRED_POLL_TIMER_ID: usize = 100;

/// Deferred result poll interval in milliseconds.
#[cfg(windows)]
pub(crate) const DEFERRED_POLL_MS: u32 = 50;

/// Animation timer ID for result list fade-in.
#[cfg(windows)]
pub(crate) const ANIM_TIMER_ID: usize = 101;

/// Settings change poll timer ID.
#[cfg(windows)]
pub(crate) const SETTINGS_POLL_TIMER_ID: usize = 102;

/// Debounce timer for text input before running a search.
#[cfg(windows)]
pub(crate) const SEARCH_DEBOUNCE_TIMER_ID: usize = 103;

/// Lightweight animation timer for search progress and icon loading.
#[cfg(windows)]
pub(crate) const BUSY_ANIM_TIMER_ID: usize = 104;

#[cfg(windows)]
pub(crate) const SETTINGS_POLL_MS: u32 = 2000;

#[cfg(windows)]
pub(crate) const SEARCH_DEBOUNCE_MS: u32 = 100;

/// Lightweight progress/icon animation interval (~30fps).
#[cfg(windows)]
pub(crate) const ANIM_FRAME_MS: u32 = 33;

/// Animation duration in frames.
#[cfg(windows)]
pub(crate) const ANIM_TOTAL_FRAMES: u8 = 10;

/// Engine event sub-types passed via WPARAM in WM_ENGINE_EVENT.
#[cfg(windows)]
pub(crate) const ENGINE_EVT_DRIVE_INDEXING: usize = 1;
#[cfg(windows)]
pub(crate) const ENGINE_EVT_DRIVE_READY: usize = 2;
#[cfg(windows)]
pub(crate) const ENGINE_EVT_DRIVE_ERROR: usize = 3;
#[cfg(windows)]
pub(crate) const ENGINE_EVT_ALL_READY: usize = 4;

#[cfg(windows)]
pub(crate) enum EngineEventPayload {
    DriveIndexing {
        drive: char,
    },
    DriveReady {
        drive: char,
        records: usize,
        seconds: f64,
    },
    DriveError {
        drive: char,
        error: String,
    },
}

#[cfg(windows)]
pub(crate) struct IconReadyPayload {
    pub(crate) request: super::icon::IconLoadRequest,
    pub(crate) pixels: Option<super::icon::IconPixels>,
    pub(crate) seq_id: u64,
}
