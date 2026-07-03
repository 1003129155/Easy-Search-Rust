// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! I18n engine — JSON-based multi-language support with locale detection and fallback chain.

use std::collections::HashMap;

/// Built-in locale JSON data, embedded at compile time.
const EN_JSON: &str = include_str!("locales/en.json");
const ZH_CN_JSON: &str = include_str!("locales/zh-CN.json");
const JA_JSON: &str = include_str!("locales/ja.json");

/// Multi-language internationalization engine.
///
/// Loads translations from embedded JSON locale files and provides key-based
/// text lookup with a fallback chain: current locale → English → key itself.
pub struct I18nEngine {
    /// locale_code → (key → translated value)
    translations: HashMap<String, HashMap<String, String>>,
    /// Currently active locale code (e.g. "en", "zh-CN", "ja")
    current_locale: String,
    /// List of loaded locale codes
    available_locales: Vec<String>,
}

impl I18nEngine {
    /// Create a new I18nEngine with system-detected locale.
    ///
    /// Loads all built-in locale files and selects the locale matching the
    /// operating system's regional settings.
    pub fn new() -> Self {
        let mut engine = Self {
            translations: HashMap::new(),
            current_locale: String::from("en"),
            available_locales: Vec::new(),
        };
        engine.load_builtin_locales();

        let system_locale = Self::detect_system_locale();
        engine.set_locale(&system_locale);
        engine
    }

    /// Create a new I18nEngine with a specific locale.
    pub fn with_locale(locale: &str) -> Self {
        let mut engine = Self {
            translations: HashMap::new(),
            current_locale: String::from("en"),
            available_locales: Vec::new(),
        };
        engine.load_builtin_locales();
        engine.set_locale(locale);
        engine
    }

    /// Get translated text for the given key.
    ///
    /// Fallback chain:
    /// 1. Look up in current locale
    /// 2. If not found, look up in English ("en")
    /// 3. If not found in English either, return the key itself
    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        // Try current locale
        if let Some(locale_map) = self.translations.get(&self.current_locale) {
            if let Some(value) = locale_map.get(key) {
                return value.as_str();
            }
        }

        // Fallback to English
        if self.current_locale != "en" {
            if let Some(en_map) = self.translations.get("en") {
                if let Some(value) = en_map.get(key) {
                    return value.as_str();
                }
            }
        }

        // Final fallback: return the key itself
        key
    }

    /// Switch language at runtime.
    ///
    /// Matching rules:
    /// 1. Exact match (e.g., "zh-CN" matches "zh-CN")
    /// 2. Prefix match (e.g., "zh-TW" matches "zh-CN" since both start with "zh")
    /// 3. If no match, default to "en"
    pub fn set_locale(&mut self, locale: &str) {
        // 1. Exact match
        if self.translations.contains_key(locale) {
            self.current_locale = locale.to_string();
            return;
        }

        // 2. Prefix match — extract language prefix (part before '-')
        let prefix = locale.split('-').next().unwrap_or(locale);
        for available in &self.available_locales {
            let available_prefix = available.split('-').next().unwrap_or(available);
            if available_prefix.eq_ignore_ascii_case(prefix) {
                self.current_locale = available.clone();
                return;
            }
        }

        // 3. Default to English
        self.current_locale = String::from("en");
    }

    /// Returns the currently active locale code.
    pub fn current_locale(&self) -> &str {
        &self.current_locale
    }

    /// Returns the list of available locale codes.
    pub fn available_locales(&self) -> &[String] {
        &self.available_locales
    }

    /// Detect system locale from Windows API.
    ///
    /// Reads the user's locale from the Windows registry
    /// (`HKCU\Control Panel\International\LocaleName`).
    /// Returns a locale string like "en-US", "zh-CN", "ja-JP", etc.
    /// Falls back to "en" if detection fails.
    pub fn detect_system_locale() -> String {
        #[cfg(windows)]
        {
            use std::ffi::OsStr;
            use std::os::windows::ffi::OsStrExt;

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
                    HKEY_CURRENT_USER, KEY_READ, REG_SZ, RegCloseKey, RegOpenKeyExW,
                    RegQueryValueExW,
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
                    return String::from("en");
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
                    return String::from_utf16_lossy(&data[..len]);
                }
            }
        }

        String::from("en")
    }

    /// Load all built-in locale JSON data into the translations map.
    fn load_builtin_locales(&mut self) {
        self.load_locale_json("en", EN_JSON);
        self.load_locale_json("zh-CN", ZH_CN_JSON);
        self.load_locale_json("ja", JA_JSON);
    }

    /// Parse a JSON string and insert its key-value pairs for the given locale.
    fn load_locale_json(&mut self, locale_code: &str, json_data: &str) {
        if let Ok(map) = serde_json::from_str::<HashMap<String, String>>(json_data) {
            self.translations.insert(locale_code.to_string(), map);
            self.available_locales.push(locale_code.to_string());
        }
    }
}

impl Default for I18nEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_engine_has_all_locales() {
        let engine = I18nEngine::with_locale("en");
        let locales = engine.available_locales();
        assert!(locales.contains(&String::from("en")));
        assert!(locales.contains(&String::from("zh-CN")));
        assert!(locales.contains(&String::from("ja")));
    }

    #[test]
    fn test_get_returns_translation() {
        let engine = I18nEngine::with_locale("en");
        assert_eq!(engine.get("placeholder_ready"), "Type to search...");
    }

    #[test]
    fn test_get_fallback_to_en() {
        let engine = I18nEngine::with_locale("ja");
        // Key exists in ja
        assert_eq!(engine.get("placeholder_ready"), "検索...");
        // Key doesn't exist anywhere — returns key itself
        assert_eq!(engine.get("nonexistent_key"), "nonexistent_key");
    }

    #[test]
    fn test_get_returns_key_when_missing() {
        let engine = I18nEngine::with_locale("en");
        assert_eq!(engine.get("totally_missing"), "totally_missing");
    }

    #[test]
    fn test_set_locale_exact_match() {
        let mut engine = I18nEngine::with_locale("en");
        engine.set_locale("zh-CN");
        assert_eq!(engine.current_locale(), "zh-CN");
        assert_eq!(engine.get("tray_exit"), "退出");
    }

    #[test]
    fn test_set_locale_prefix_match() {
        let mut engine = I18nEngine::with_locale("en");
        // "zh-TW" should prefix-match to "zh-CN"
        engine.set_locale("zh-TW");
        assert_eq!(engine.current_locale(), "zh-CN");
    }

    #[test]
    fn test_set_locale_no_match_defaults_to_en() {
        let mut engine = I18nEngine::with_locale("zh-CN");
        engine.set_locale("fr-FR");
        assert_eq!(engine.current_locale(), "en");
    }

    #[test]
    fn test_with_locale_constructor() {
        let engine = I18nEngine::with_locale("ja");
        assert_eq!(engine.current_locale(), "ja");
        assert_eq!(engine.get("tray_show"), "表示");
    }
}
