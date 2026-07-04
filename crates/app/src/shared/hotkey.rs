// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Global hotkey registration (Alt+Space).

#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT,
};
#[cfg(windows)]
use windows::Win32::Foundation::HWND;

/// Hotkey ID used in WM_HOTKEY messages.
pub const HOTKEY_ID: i32 = 1;

/// Virtual key code for Space.
const VK_SPACE: u32 = 0x20;

/// Register the global hotkey (Alt+Space).
#[cfg(windows)]
pub fn register(hwnd: HWND) -> bool {
    unsafe {
        RegisterHotKey(
            Some(hwnd),
            HOTKEY_ID,
            HOT_KEY_MODIFIERS(MOD_ALT.0),
            VK_SPACE,
        )
        .is_ok()
    }
}

/// Unregister the global hotkey.
#[cfg(windows)]
pub fn unregister(hwnd: HWND) {
    unsafe {
        let _ = UnregisterHotKey(Some(hwnd), HOTKEY_ID);
    }
}
