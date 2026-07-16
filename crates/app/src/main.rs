// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! EasySearch GUI — Win32 + Direct2D frontend with integrated search engine.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod search;
mod settings;
mod shared;
mod theme;
mod i18n;

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Arc, Mutex, RwLock};

use shared::settings_store::{Settings, SettingsStore};
use theme::engine::ThemeEngine;
use i18n::engine::I18nEngine;

/// Global shared settings — accessible from tray menu handler and other modules.
pub static SHARED_SETTINGS: std::sync::OnceLock<Arc<RwLock<Settings>>> = std::sync::OnceLock::new();

/// Global log file handle — all log!() macro calls write here.
static LOG_FILE: std::sync::OnceLock<Mutex<std::fs::File>> = std::sync::OnceLock::new();

/// Max log file size before rotation (2 MB).
const LOG_MAX_SIZE: u64 = 2 * 1024 * 1024;

/// Initialize the log file in the app data directory.
/// If the existing log exceeds [`LOG_MAX_SIZE`], rename it to `.log.old`
/// and start a fresh file.
fn init_log() {
    let log_dir = easysearch_core::paths::app_root_dir();
    // Ensure the directory exists
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("easysearch.log");

    // Simple rotation: keep at most one old file
    if let Ok(meta) = std::fs::metadata(&log_path) {
        if meta.len() > LOG_MAX_SIZE {
            let old_path = log_path.with_extension("log.old");
            let _ = std::fs::rename(&log_path, &old_path);
        }
    }

    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(&log_path);

    if let Ok(f) = file {
        LOG_FILE.set(Mutex::new(f)).ok();
    }
    // If it fails, logging is silently disabled (program still runs)
}

/// Write a line to the log file.
#[allow(dead_code)]
pub fn log_write(msg: &str) {
    if let Some(mtx) = LOG_FILE.get() {
        if let Ok(mut f) = mtx.lock() {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let _ = writeln!(f, "[{now}] {msg}");
            let _ = f.flush();
        }
    }
    // Also stderr for debug builds
    eprintln!("{msg}");
}

/// Convenience macro for logging.
#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        $crate::log_write(&format!($($arg)*))
    };
}

fn main() {
    init_log();

    // Set up panic hook to log panics to file
    std::panic::set_hook(Box::new(|info| {
        let msg = format!("PANIC: {info}");
        log_write(&msg);
        // Also write a backtrace if available
        let bt = std::backtrace::Backtrace::force_capture();
        log_write(&format!("Backtrace:\n{bt}"));
    }));

    log!("=== EasySearch GUI starting ===");

    // ── Load user settings ──────────────────────────────────────────────────
    let settings_path = easysearch_core::paths::settings_file();

    let settings = if settings_path.exists() {
        log!("Loading settings from {:?}", settings_path);
        SettingsStore::load(&settings_path)
    } else {
        log!("No settings file found, using defaults");
        // Don't block startup by writing — save lazily on first change
        Settings::default()
    };
    log!("Settings ready (drives={:?})", settings.index_drives);
    let shared_settings = Arc::new(RwLock::new(settings));

    // Store in global OnceLock so tray menu handler can access it
    SHARED_SETTINGS.set(shared_settings.clone()).ok();
    log!("Shared settings initialized");

    // ── Initialize theme engine (loaded for validation, search window uses its own) ──
    let _theme_engine = ThemeEngine::new();
    log!("ThemeEngine initialized with {} built-in themes", _theme_engine.available_themes().len());

    // ── Log i18n locale (actual engine is created per-window) ──────────────
    let locale = {
        let settings_read = shared_settings.read().unwrap();
        if settings_read.language.is_empty() {
            I18nEngine::detect_system_locale()
        } else {
            settings_read.language.clone()
        }
    };
    log!("Language locale: {}", locale);

    // Sync locale to core context_labels so plugins produce localized strings.
    easysearch_core::context_labels::set_locale(&locale);

    // Set DPI awareness before creating any windows
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::HiDpi::{
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
        };
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        log!("DPI awareness set");
    }

    #[cfg(windows)]
    {
        match search::run() {
            Ok(()) => log!("Window::run() exited normally"),
            Err(e) => {
                log!("FATAL: window::run() returned error: {e}");
                // Show a message box so the user knows why the process exits
                #[cfg(windows)]
                {
                    use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
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
        log!("easysearch-gui requires Windows");
        std::process::exit(1);
    }
}
