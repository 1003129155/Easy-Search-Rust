// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Engine initialization and event forwarding to the Win32 message loop.

#[cfg(windows)]
use std::sync::Arc;

#[cfg(windows)]
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

#[cfg(windows)]
use super::messages::*;

/// Initialize the search engine from settings and spawn the event-forwarding thread.
///
/// Returns the engine wrapped in `Arc` so it can be shared with the plugin router.
/// The background thread listens on the engine event channel and posts `WM_ENGINE_EVENT`
/// messages to `hwnd` so the window procedure can update UI state.
#[cfg(windows)]
pub(super) fn start_engine(hwnd: HWND) -> Arc<easysearch_engine::SearchEngine> {
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

    // Start background indexing
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
                    easysearch_engine::EngineEvent::DriveReady {
                        drive,
                        records,
                        elapsed,
                    } => {
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
                    easysearch_engine::EngineEvent::UsnUpdate {
                        drive,
                        events_applied,
                    } => {
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
                    let _ =
                        PostMessageW(Some(h), WM_ENGINE_EVENT, WPARAM(evt_type), LPARAM(data_ptr));
                }
            }
        })
        .ok();

    Arc::new(engine)
}
