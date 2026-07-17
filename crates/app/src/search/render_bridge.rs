// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Rendering orchestration for the search window.
//!
//! The renderer itself stays in `renderer.rs`; this module translates current
//! `AppState` into renderer parameters, starts icon-loading animation timers,
//! and forwards async icon load completions back to the window thread.

#[cfg(windows)]
use std::sync::atomic::{AtomicUsize, Ordering};
#[cfg(windows)]
use std::sync::{OnceLock, mpsc};

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
    ANIM_FRAME_MS, ANIM_TOTAL_FRAMES, BUSY_ANIM_TIMER_ID, IconReadyPayload, WM_ICON_READY,
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
            // When input is focused, pass 0 to keep scroll at top but use a flag
            // to prevent highlighting. We use a sentinel: items.len() won't match
            // any valid index (0..len-1), so no item gets highlighted, but we need
            // scroll at top — so pass 0 when input_focused.
            if app.input_focused {
                0
            } else {
                app.selected_index
            },
            placeholder,
            &mut app.icon_cache,
            anim_progress,
            app.search_active,
            preview_param,
            app.input_focused,
            app.cursor_moved_at,
        );

        let icon_requests = app.icon_cache.take_load_requests();
        if !icon_requests.is_empty() {
            unsafe {
                let _ = SetTimer(Some(app.hwnd), BUSY_ANIM_TIMER_ID, ANIM_FRAME_MS, None);
            }
            spawn_icon_loads(app.hwnd, app.current_search_seq, icon_requests);
        }
    });
}

#[cfg(windows)]
struct IconLoadJob {
    hwnd_raw: usize,
    seq_id: u64,
    request: super::icon::IconLoadRequest,
}

#[cfg(windows)]
struct IconLoadPool {
    senders: Vec<mpsc::Sender<IconLoadJob>>,
    next_worker: AtomicUsize,
}

#[cfg(windows)]
impl IconLoadPool {
    fn new() -> Self {
        const WORKER_COUNT: usize = 2;
        let mut senders = Vec::with_capacity(WORKER_COUNT);

        for worker_index in 0..WORKER_COUNT {
            let (tx, rx) = mpsc::channel::<IconLoadJob>();
            std::thread::Builder::new()
                .name(format!("icon-loader-{worker_index}"))
                .spawn(move || {
                    while let Ok(job) = rx.recv() {
                        let pixels = super::icon::load_icon_pixels(&job.request);
                        let payload = Box::new(IconReadyPayload {
                            request: job.request,
                            pixels,
                            seq_id: job.seq_id,
                        });
                        unsafe {
                            let hwnd = HWND(job.hwnd_raw as *mut _);
                            let _ = PostMessageW(
                                Some(hwnd),
                                WM_ICON_READY,
                                WPARAM(0),
                                LPARAM(Box::into_raw(payload) as isize),
                            );
                        }
                    }
                })
                .expect("failed to spawn icon loader worker");
            senders.push(tx);
        }

        Self {
            senders,
            next_worker: AtomicUsize::new(0),
        }
    }

    fn submit(&self, hwnd: HWND, seq_id: u64, requests: Vec<super::icon::IconLoadRequest>) {
        let hwnd_raw = hwnd.0 as usize;
        for request in requests {
            let worker = self.next_worker.fetch_add(1, Ordering::Relaxed) % self.senders.len();
            let _ = self.senders[worker].send(IconLoadJob {
                hwnd_raw,
                seq_id,
                request,
            });
        }
    }
}

#[cfg(windows)]
static ICON_LOAD_POOL: OnceLock<IconLoadPool> = OnceLock::new();

#[cfg(windows)]
fn spawn_icon_loads(hwnd: HWND, seq_id: u64, requests: Vec<super::icon::IconLoadRequest>) {
    ICON_LOAD_POOL
        .get_or_init(IconLoadPool::new)
        .submit(hwnd, seq_id, requests);
}
