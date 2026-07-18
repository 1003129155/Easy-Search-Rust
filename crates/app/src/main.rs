// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! EasySearch GUI — Win32 + Direct2D frontend with integrated search engine.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod i18n;
mod search;
mod settings;
mod shared;
mod theme;

use std::sync::{Arc, RwLock};

use easysearch_core::{log_debug, log_error, log_info};

use i18n::engine::I18nEngine;
use shared::settings_store::{Settings, SettingsStore};
use theme::engine::ThemeEngine;

/// Global shared settings — accessible from tray menu handler and other modules.
pub static SHARED_SETTINGS: std::sync::OnceLock<Arc<RwLock<Settings>>> = std::sync::OnceLock::new();

fn main() {
    easysearch_core::logging::init();

    // Set up panic hook to log panics to file
    std::panic::set_hook(Box::new(|info| {
        log_error!("PANIC: {info}");
        // Also write a backtrace if available
        let bt = std::backtrace::Backtrace::force_capture();
        log_error!("Backtrace:\n{bt}");
    }));

    let settings_path = easysearch_core::paths::settings_file();

    let settings = if settings_path.exists() {
        let loaded = SettingsStore::load(&settings_path);
        easysearch_core::logging::set_level_from_str(&loaded.log_level);
        log_debug!("Loading settings from {:?}", settings_path);
        loaded
    } else {
        let defaults = Settings::default();
        easysearch_core::logging::set_level_from_str(&defaults.log_level);
        log_info!("No settings file found, using defaults");
        defaults
    };
    log_info!("=== EasySearch GUI starting ===");
    log_info!("Settings ready (drives={:?})", settings.index_drives);
    let shared_settings = Arc::new(RwLock::new(settings));

    // Store in global OnceLock so tray menu handler can access it
    SHARED_SETTINGS.set(shared_settings.clone()).ok();
    log_debug!("Shared settings initialized");

    // ── Initialize theme engine (loaded for validation, search window uses its own) ──
    let _theme_engine = ThemeEngine::new();
    log_debug!(
        "ThemeEngine initialized with {} built-in themes",
        _theme_engine.available_themes().len()
    );

    // ── Log i18n locale (actual engine is created per-window) ──────────────
    let locale = {
        let settings_read = shared_settings.read().unwrap();
        if settings_read.language.is_empty() {
            I18nEngine::detect_system_locale()
        } else {
            settings_read.language.clone()
        }
    };
    log_debug!("Language locale: {}", locale);

    // Sync locale to core context_labels so plugins produce localized strings.
    easysearch_core::context_labels::set_locale(&locale);

    // Set DPI awareness before creating any windows
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::HiDpi::{
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
        };
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        log_debug!("DPI awareness set");
    }

    #[cfg(windows)]
    {
        match search::run() {
            Ok(()) => log_debug!("Window::run() exited normally"),
            Err(e) => {
                log_error!("FATAL: window::run() returned error: {e}");
                // Show a message box so the user knows why the process exits
                #[cfg(windows)]
                {
                    use windows::Win32::UI::WindowsAndMessaging::{
                        MB_ICONERROR, MB_OK, MessageBoxW,
                    };
                    use windows::core::PCWSTR;
                    let msg: Vec<u16> = format!("EasySearch 启动失败:\n{e}\0")
                        .encode_utf16()
                        .collect();
                    let title: Vec<u16> = "EasySearch Error\0".encode_utf16().collect();
                    unsafe {
                        MessageBoxW(
                            None,
                            PCWSTR(msg.as_ptr()),
                            PCWSTR(title.as_ptr()),
                            MB_OK | MB_ICONERROR,
                        );
                    }
                }
                std::process::exit(1);
            }
        }
    }

    #[cfg(not(windows))]
    {
        log_debug!("easysearch-gui requires Windows");
        std::process::exit(1);
    }
}
