// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Cross-thread settings change notification channel.
//!
//! When the settings window (iced thread) modifies a setting, it sends a
//! [`SettingsChange`] message to the search window (Win32 thread) so it can
//! react in real-time without restart.

use std::sync::mpsc;
use std::sync::OnceLock;

/// A setting that was changed by the user in the settings window.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum SettingsChange {
    /// Theme was changed (new theme name).
    ThemeChanged(String),
    /// Language was changed (new locale code, e.g. "en", "zh-CN", "ja").
    LanguageChanged(String),
    /// Hotkey binding was changed (new hotkey string, e.g. "Alt+Space").
    HotkeyChanged(String),
    /// Index drives were changed (new list of drive letters).
    DrivesChanged(Vec<char>),
    /// Autostart setting was toggled.
    AutostartChanged(bool),
}

/// Sender half — held by the settings window thread.
pub type SettingsChangeSender = mpsc::Sender<SettingsChange>;

/// Receiver half — polled by the search window thread.
pub type SettingsChangeReceiver = mpsc::Receiver<SettingsChange>;

/// Global sender stored so the settings window can access it.
static SETTINGS_CHANGE_TX: OnceLock<SettingsChangeSender> = OnceLock::new();

/// Create the global settings change channel. Returns the receiver.
/// Call this once during initialization (in main/run).
pub fn init_settings_channel() -> SettingsChangeReceiver {
    let (tx, rx) = mpsc::channel();
    SETTINGS_CHANGE_TX.set(tx).ok();
    rx
}

/// Get a clone of the global sender (for use by the settings window).
/// Returns None if the channel hasn't been initialized yet.
pub fn get_settings_sender() -> Option<SettingsChangeSender> {
    SETTINGS_CHANGE_TX.get().cloned()
}
