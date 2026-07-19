// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Settings module — opens settings.json in the user's default text editor.
//!
//! Users edit the JSON file directly to change configuration.

use std::sync::{Arc, RwLock};

use crate::shared::settings_store::Settings;

/// Open the settings.json file in the user's default editor.
///
/// After the editor closes (or immediately, since ShellExecute is async),
/// the search window will pick up changes on its next settings poll cycle.
pub fn open_settings_file(_settings: Arc<RwLock<Settings>>) {
    let settings_path = easysearch_core::paths::settings_file();

    // Ensure settings file exists before opening
    if !settings_path.exists() {
        let defaults = Settings::default();
        let _ = crate::shared::settings_store::SettingsStore::save(&settings_path, &defaults);
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        use windows::core::PCWSTR;

        let path_wide: Vec<u16> = settings_path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let verb: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            ShellExecuteW(
                None,
                PCWSTR(verb.as_ptr()),
                PCWSTR(path_wide.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            );
        }
    }
}
