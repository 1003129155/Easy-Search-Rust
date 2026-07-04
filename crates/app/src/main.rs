// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! EasySearch GUI — Win32 + Direct2D frontend with integrated search engine.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod search;
mod shared;
mod theme;
mod i18n;
mod settings;
mod welcome;

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

/// Initialize the log file next to the exe.
fn init_log() {
    let log_path = std::env::current_exe()
        .unwrap_or_default()
        .with_file_name("easysearch.log");

    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)
        .expect("failed to open log file");

    LOG_FILE.set(Mutex::new(file)).ok();
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

    // ── Subprocess mode: --settings launches only the settings window ────────
    if std::env::args().any(|a| a == "--settings") {
        run_settings_subprocess();
        return;
    }    log!("=== EasySearch GUI starting ===");

    // ── Load user settings ──────────────────────────────────────────────────
    let settings_path = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("EasySearch")
        .join("settings.json");

    // Check if first run (settings.json didn't exist → first launch)
    let is_first_run = !settings_path.exists();

    let settings = SettingsStore::load(&settings_path);
    let shared_settings = Arc::new(RwLock::new(settings));

    // ── Welcome wizard (first run only) ─────────────────────────────────────
    if is_first_run {
        log!("First run detected, launching welcome wizard");
        if let Err(e) = welcome::run_welcome() {
            log!("Welcome wizard error: {e}");
        }
    }

    // Store in global OnceLock so tray menu handler can access it
    SHARED_SETTINGS.set(shared_settings.clone()).ok();

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

/// Run in subprocess mode: only the settings window, then exit.
/// Called when the exe is invoked with `--settings`.
fn run_settings_subprocess() {
    log!("=== Settings subprocess starting ===");

    let settings_path = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("EasySearch")
        .join("settings.json");

    let settings = SettingsStore::load(&settings_path);
    let shared_settings = Arc::new(RwLock::new(settings));

    // Build plugin info list for the settings UI
    let plugin_infos = build_plugin_infos();

    // Run the iced settings app (blocks until window closes)
    match settings::app::run_settings_app(shared_settings, plugin_infos) {
        Ok(()) => log!("Settings window closed normally"),
        Err(e) => log!("Settings window error: {e}"),
    }
}

/// Build plugin info list for the settings UI by instantiating all plugins
/// and extracting their metadata + settings schema.
#[cfg(windows)]
fn build_plugin_infos() -> Vec<settings::view_models::page_plugin::PluginInfo> {
    use easysearch_core::Plugin;
    use settings::view_models::page_plugin::PluginInfo;

    let plugins: Vec<Box<dyn Plugin>> = vec![
        Box::new(plugin_bookmark::BookmarkPlugin::new()),
        Box::new(plugin_program::ProgramPlugin::new()),
        Box::new(plugin_sys_cmd::SysCmdPlugin::new()),
        Box::new(plugin_win_settings::WinSettingsPlugin::new()),
    ];

    plugins
        .iter()
        .map(|p| PluginInfo {
            name: p.name().to_string(),
            description: p.description().to_string(),
            icon: p.icon().to_string(),
            keyword: p.default_keyword().map(|s| s.to_string()),
            settings_schema: p.settings_schema(),
            setting_values: p.setting_values(),
        })
        .collect()
}

#[cfg(not(windows))]
fn build_plugin_infos() -> Vec<settings::view_models::page_plugin::PluginInfo> {
    Vec::new()
}
