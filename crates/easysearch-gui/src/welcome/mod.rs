// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Welcome wizard window — shown on first run to guide the user through
//! initial configuration (language, hotkey, theme, autostart).
//!
//! The wizard runs as a blocking iced application before the search window
//! starts, so there are no threading concerns.

pub mod app;
pub mod pages;
pub mod state;

/// Run the welcome wizard (blocking).
///
/// This should be called before `search::run()` on first launch.
/// Returns `Ok(())` when the wizard finishes normally.
pub fn run_welcome() -> Result<(), String> {
    app::run_welcome_app().map_err(|e| format!("Welcome wizard error: {e}"))
}
