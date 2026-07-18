// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Settings file polling and runtime reload logic.

#[cfg(windows)]
use windows::Win32::Foundation::HWND;

#[cfg(windows)]
use super::app_state::AppState;
#[cfg(windows)]
use super::plugin_bridge::build_plugin_router;
#[cfg(windows)]
use crate::shared::hotkey;
#[cfg(windows)]
use crate::shared::settings_store::Settings;

/// Apply settings changes from `on_disk` compared to `current`.
/// Mutates `app` in place and returns `true` if any setting was applied.
#[cfg(windows)]
pub(super) fn apply_settings_diff(
    app: &mut AppState,
    hwnd: HWND,
    current: &Settings,
    on_disk: &Settings,
) -> bool {
    let mut changed = false;

    // Log level
    if on_disk.log_level != current.log_level {
        easysearch_core::logging::set_level_from_str(&on_disk.log_level);
        changed = true;
    }

    // Theme
    if on_disk.theme != current.theme {
        app.renderer.theme = match on_disk.theme.as_str() {
            "Win11Light" => crate::theme::Theme::light(),
            "Win11Dark" => crate::theme::Theme::dark(),
            _ => crate::theme::Theme::system(),
        };
        changed = true;
    }

    // Language
    if on_disk.language != current.language {
        let locale = if on_disk.language.is_empty() {
            crate::i18n::engine::I18nEngine::detect_system_locale()
        } else {
            on_disk.language.clone()
        };
        app.i18n.set_locale(&locale);
        easysearch_core::context_labels::set_locale(&locale);
        app.plugin_router = build_plugin_router(app.engine.clone());
        changed = true;
    }

    // Hotkey
    if on_disk.hotkey != current.hotkey {
        hotkey::unregister(hwnd);
        if let Some((modifiers, vk)) = parse_hotkey_string(&on_disk.hotkey) {
            unsafe {
                use windows::Win32::UI::Input::KeyboardAndMouse::{
                    HOT_KEY_MODIFIERS, RegisterHotKey,
                };
                let _ = RegisterHotKey(
                    Some(hwnd),
                    hotkey::HOTKEY_ID,
                    HOT_KEY_MODIFIERS(modifiers),
                    vk,
                );
            }
        } else {
            hotkey::register(hwnd);
        }
        changed = true;
    }

    // Drives
    if on_disk.index_drives != current.index_drives {
        if let Some(ref engine) = app.engine {
            let new_drives: Vec<char> = on_disk
                .index_drives
                .iter()
                .filter_map(|s| s.chars().next().map(|c| c.to_ascii_uppercase()))
                .collect();

            let current_labels = engine.drive_labels();
            let current_drives: Vec<char> = current_labels
                .iter()
                .filter_map(|s| s.chars().next())
                .collect();

            for &d in &new_drives {
                if !current_drives.contains(&d) {
                    engine.add_drive(d);
                }
            }
            for &d in &current_drives {
                if !new_drives.contains(&d) {
                    engine.remove_drive(d);
                }
            }
        }
        changed = true;
    }

    // Autostart
    if on_disk.autostart != current.autostart {
        #[cfg(windows)]
        {
            if on_disk.autostart {
                let _ = crate::shared::autostart::enable();
            } else {
                let _ = crate::shared::autostart::disable();
            }
        }
        changed = true;
    }

    changed
}

/// Parse a hotkey string like "Alt+Space", "Ctrl+Shift+F" into (modifiers, vk_code).
/// Returns None if the string can't be parsed.
#[cfg(windows)]
pub(super) fn parse_hotkey_string(s: &str) -> Option<(u32, u32)> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN};

    let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();
    if parts.is_empty() {
        return None;
    }

    let mut modifiers: u32 = 0;
    let mut key_part = "";

    for part in &parts {
        match part.to_lowercase().as_str() {
            "alt" => modifiers |= MOD_ALT.0,
            "ctrl" | "control" => modifiers |= MOD_CONTROL.0,
            "shift" => modifiers |= MOD_SHIFT.0,
            "win" | "super" => modifiers |= MOD_WIN.0,
            _ => key_part = part,
        }
    }

    let vk = match key_part.to_lowercase().as_str() {
        "space" => 0x20u32,
        "enter" | "return" => 0x0D,
        "tab" => 0x09,
        "escape" | "esc" => 0x1B,
        "backspace" => 0x08,
        "delete" | "del" => 0x2E,
        "insert" | "ins" => 0x2D,
        "home" => 0x24,
        "end" => 0x23,
        "pageup" | "pgup" => 0x21,
        "pagedown" | "pgdn" => 0x22,
        "up" => 0x26,
        "down" => 0x28,
        "left" => 0x25,
        "right" => 0x27,
        s if s.len() == 1 => {
            let c = s.chars().next()?;
            if c.is_ascii_alphabetic() {
                c.to_ascii_uppercase() as u32
            } else if c.is_ascii_digit() {
                c as u32
            } else {
                return None;
            }
        }
        s if s.starts_with('f') && s[1..].parse::<u32>().is_ok() => {
            let n: u32 = s[1..].parse().ok()?;
            if n >= 1 && n <= 24 {
                0x6F + n // VK_F1 = 0x70
            } else {
                return None;
            }
        }
        _ => return None,
    };

    Some((modifiers, vk))
}
