// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Legacy I18n types — temporary bridge until the full JSON i18n engine is implemented.

use std::collections::HashMap;

/// Supported languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    ChineseSimplified,
    Japanese,
}

/// Simple internationalization struct with key-value lookup.
pub struct I18n {
    lang: Language,
    strings: HashMap<&'static str, &'static str>,
}

impl I18n {
    /// Create a new I18n instance with auto-detected system language.
    pub fn new() -> Self {
        let lang = detect_system_language();
        let strings = load_strings(lang);
        Self { lang, strings }
    }

    /// Get a translated string by key. Returns the key itself if not found.
    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        self.strings.get(key).copied().unwrap_or(key)
    }

    /// Get the current language.
    #[allow(dead_code)]
    pub fn language(&self) -> Language {
        self.lang
    }
}

impl Default for I18n {
    fn default() -> Self {
        Self::new()
    }
}

/// Load translation strings for a given language.
fn load_strings(lang: Language) -> HashMap<&'static str, &'static str> {
    let mut m = HashMap::new();
    match lang {
        Language::English => {
            m.insert("placeholder_ready", "Type to search...");
            m.insert("placeholder_indexing", "Building index...");
            m.insert("tray_show", "Show");
            m.insert("tray_settings", "Settings");
            m.insert("tray_exit", "Exit");
        }
        Language::ChineseSimplified => {
            m.insert(
                "placeholder_ready",
                "\u{8F93}\u{5165}\u{4EE5}\u{641C}\u{7D22}...",
            );
            m.insert(
                "placeholder_indexing",
                "\u{6B63}\u{5728}\u{6784}\u{5EFA}\u{7D22}\u{5F15}...",
            );
            m.insert("tray_show", "\u{663E}\u{793A}");
            m.insert("tray_settings", "\u{8BBE}\u{7F6E}");
            m.insert("tray_exit", "\u{9000}\u{51FA}");
        }
        Language::Japanese => {
            m.insert("placeholder_ready", "\u{691C}\u{7D22}...");
            m.insert(
                "placeholder_indexing",
                "\u{30A4}\u{30F3}\u{30C7}\u{30C3}\u{30AF}\u{30B9}\u{69CB}\u{7BC9}\u{4E2D}...",
            );
            m.insert("tray_show", "\u{8868}\u{793A}");
            m.insert("tray_settings", "\u{8A2D}\u{5B9A}");
            m.insert("tray_exit", "\u{7D42}\u{4E86}");
        }
    }
    m
}

/// Detect system language from Windows locale via registry.
fn detect_system_language() -> Language {
    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        // Read locale from registry: HKCU\Control Panel\International\LocaleName
        let subkey: Vec<u16> = OsStr::new(r"Control Panel\International")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        let value_name: Vec<u16> = OsStr::new("LocaleName")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            use windows::Win32::System::Registry::{
                HKEY_CURRENT_USER, KEY_READ, REG_SZ, RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
            };

            let mut hkey = Default::default();
            let status = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                windows::core::PCWSTR(subkey.as_ptr()),
                Some(0),
                KEY_READ,
                &mut hkey,
            );
            if status.is_err() {
                return Language::English;
            }

            let mut data = vec![0u16; 85];
            let mut data_size = (data.len() * 2) as u32;
            let mut data_type = REG_SZ;
            let result = RegQueryValueExW(
                hkey,
                windows::core::PCWSTR(value_name.as_ptr()),
                None,
                Some(&mut data_type),
                Some(data.as_mut_ptr() as *mut u8),
                Some(&mut data_size),
            );
            let _ = RegCloseKey(hkey);

            if result.is_ok() {
                let len = (data_size as usize / 2).saturating_sub(1);
                let locale = String::from_utf16_lossy(&data[..len]).to_lowercase();

                if locale.starts_with("zh") {
                    return Language::ChineseSimplified;
                } else if locale.starts_with("ja") {
                    return Language::Japanese;
                }
            }
        }
    }
    Language::English
}
