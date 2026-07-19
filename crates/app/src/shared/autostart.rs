// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Auto-start management via Windows Registry.
//!
//! Adds/removes `EasySearch` from `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.

#[cfg(windows)]
use windows::Win32::System::Registry::{
    HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ, RegCloseKey, RegDeleteValueW, RegOpenKeyExW,
    RegQueryValueExW, RegSetValueExW,
};
#[cfg(windows)]
use windows::core::PCWSTR;

/// Registry path for auto-start entries.
#[cfg(windows)]
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// Value name for our entry.
#[cfg(windows)]
const VALUE_NAME: &str = "EasySearch";

/// Check if auto-start is currently enabled.
#[cfg(windows)]
#[allow(dead_code)]
pub fn is_enabled() -> bool {
    unsafe {
        let key_wide = wide_null(RUN_KEY);
        let mut hkey = std::mem::zeroed();
        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_wide.as_ptr()),
            Some(0),
            KEY_READ,
            &mut hkey,
        );
        if result.is_err() {
            return false;
        }

        let name_wide = wide_null(VALUE_NAME);
        let mut data_size: u32 = 0;
        let exists = RegQueryValueExW(
            hkey,
            PCWSTR(name_wide.as_ptr()),
            None,
            None,
            None,
            Some(&mut data_size),
        );
        let _ = RegCloseKey(hkey);
        exists.is_ok() && data_size > 0
    }
}

/// Enable auto-start by writing the current exe path to the registry.
#[cfg(windows)]
pub fn enable() -> Result<(), String> {
    let exe_path = std::env::current_exe().map_err(|e| format!("Failed to get exe path: {e}"))?;
    let exe_str = exe_path.to_string_lossy();
    // Wrap in quotes for paths with spaces
    let value = format!("\"{exe_str}\"");

    unsafe {
        let key_wide = wide_null(RUN_KEY);
        let mut hkey = std::mem::zeroed();
        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_wide.as_ptr()),
            Some(0),
            KEY_WRITE,
            &mut hkey,
        );
        if result.is_err() {
            return Err("Failed to open Run registry key".to_string());
        }

        let name_wide = wide_null(VALUE_NAME);
        let value_wide = wide_null(&value);
        let data_bytes =
            std::slice::from_raw_parts(value_wide.as_ptr() as *const u8, value_wide.len() * 2);

        let set_result = RegSetValueExW(
            hkey,
            PCWSTR(name_wide.as_ptr()),
            Some(0),
            REG_SZ,
            Some(data_bytes),
        );
        let _ = RegCloseKey(hkey);

        if set_result.is_err() {
            return Err("Failed to set registry value".to_string());
        }
    }
    Ok(())
}

/// Disable auto-start by removing the registry entry.
#[cfg(windows)]
pub fn disable() -> Result<(), String> {
    unsafe {
        let key_wide = wide_null(RUN_KEY);
        let mut hkey = std::mem::zeroed();
        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_wide.as_ptr()),
            Some(0),
            KEY_WRITE,
            &mut hkey,
        );
        if result.is_err() {
            return Err("Failed to open Run registry key".to_string());
        }

        let name_wide = wide_null(VALUE_NAME);
        let del_result = RegDeleteValueW(hkey, PCWSTR(name_wide.as_ptr()));
        let _ = RegCloseKey(hkey);

        if del_result.is_err() {
            // Not an error if it doesn't exist
            return Ok(());
        }
    }
    Ok(())
}

/// Toggle auto-start state.
#[cfg(windows)]
#[allow(dead_code)]
pub fn toggle() -> Result<bool, String> {
    if is_enabled() {
        disable()?;
        Ok(false)
    } else {
        enable()?;
        Ok(true)
    }
}

#[cfg(windows)]
fn wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
