// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Settings window — iced-based UI for application configuration.
//!
//! This module manages the lifecycle of the settings window thread:
//! - Singleton enforcement (only one settings window at a time)
//! - Thread spawning with panic recovery
//! - Resource cleanup when the window closes

pub mod app;
pub mod views;
pub mod view_models;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;

use crate::shared::settings_store::Settings;

/// Global flag indicating whether the settings window is currently open.
static SETTINGS_OPEN: AtomicBool = AtomicBool::new(false);

/// Open the settings window in a dedicated thread.
///
/// Due to iced/winit's limitation of one event loop per process, we spawn
/// a new instance of ourselves with `--settings` argument. This avoids the
/// `RecreationAttempt` panic when the welcome wizard has already consumed
/// the process's event loop slot.
///
/// # Requirements
/// - Req 1.2: Settings window runs independently
/// - Req 1.7: If settings process crashes, search window stays alive
pub fn open_settings_window(settings: Arc<RwLock<Settings>>) {
    // Singleton check: don't open a second instance
    if SETTINGS_OPEN.swap(true, Ordering::SeqCst) {
        return;
    }

    // Save current settings to disk first so the subprocess can read them
    let settings_path = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("EasySearch")
        .join("settings.json");
    if let Ok(s) = settings.read() {
        let _ = crate::shared::settings_store::SettingsStore::save(&settings_path, &s);
    }

    thread::Builder::new()
        .name("settings-launcher".to_string())
        .spawn(move || {
            // Launch self with --settings flag
            let exe = std::env::current_exe().unwrap_or_default();
            let result = std::process::Command::new(&exe)
                .arg("--settings")
                .status();

            SETTINGS_OPEN.store(false, Ordering::SeqCst);

            match result {
                Ok(status) => {
                    if !status.success() {
                        eprintln!("[settings] process exited with: {status}");
                    }
                    // Reload settings from disk (subprocess may have changed them)
                    let new_settings = crate::shared::settings_store::SettingsStore::load(&settings_path);
                    let theme_name = new_settings.theme.clone();
                    let language = new_settings.language.clone();
                    let hotkey = new_settings.hotkey.clone();
                    let drives: Vec<char> = new_settings.index_drives.iter()
                        .filter_map(|s| s.chars().next().map(|c| c.to_ascii_uppercase()))
                        .collect();

                    if let Ok(mut s) = settings.write() {
                        *s = new_settings;
                    }

                    // Notify the search window about changes via channel
                    use crate::shared::settings_channel::{self, SettingsChange};
                    if let Some(tx) = settings_channel::get_settings_sender() {
                        let _ = tx.send(SettingsChange::ThemeChanged(theme_name));
                        let _ = tx.send(SettingsChange::LanguageChanged(language));
                        let _ = tx.send(SettingsChange::HotkeyChanged(hotkey));
                        let _ = tx.send(SettingsChange::DrivesChanged(drives));
                    }
                }
                Err(e) => {
                    eprintln!("[settings] failed to spawn: {e}");
                }
            }
        })
        .expect("failed to spawn settings-launcher thread");
}

/// Returns whether the settings window is currently open.
#[allow(dead_code)]
pub fn is_settings_open() -> bool {
    SETTINGS_OPEN.load(Ordering::SeqCst)
}
