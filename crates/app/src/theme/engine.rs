// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Theme engine — loads, resolves, and manages themes at runtime.
//!
//! The engine supports:
//! - Built-in themes (Win11Light, Win11Dark, and a system-auto theme)
//! - User-defined themes loaded from a directory (max 50, sorted by filename)
//! - Base theme inheritance: missing color fields are inherited from the
//!   light or dark base depending on `is_dark`

use std::collections::BTreeMap;
use std::path::Path;

use super::types::{Color, ResolvedThemeColors, Theme, ThemeError, ThemeFile, ThemeMeta};

#[cfg(test)]
use super::types::ThemeColors;

// ─── Default Base Colors ───────────────────────────────────────────────────────

/// Default light base colors (Win11 Light style).
fn default_light_colors() -> ResolvedThemeColors {
    ResolvedThemeColors {
        background: Color::rgb(252, 252, 252),
        search_bg: Color::rgb(252, 252, 252),
        text_primary: Color::rgb(32, 32, 32),
        text_secondary: Color::rgb(96, 96, 96),
        selected_bg: Color::rgba(0, 0, 0, 15),
        accent: Color::rgb(0, 103, 192),
        separator: Color::rgba(0, 0, 0, 20),
        placeholder: Color::rgb(150, 150, 150),
        border: Color::rgba(0, 0, 0, 30),
        hotkey_bg: Color::rgba(0, 0, 0, 12),
        hotkey_text: Color::rgb(96, 96, 96),
        selection: Color::rgba(0, 103, 192, 60),
    }
}

/// Default dark base colors (Win11 Dark style).
fn default_dark_colors() -> ResolvedThemeColors {
    ResolvedThemeColors {
        background: Color::rgb(40, 40, 40),
        search_bg: Color::rgb(40, 40, 40),
        text_primary: Color::rgb(255, 255, 255),
        text_secondary: Color::rgb(180, 180, 180),
        selected_bg: Color::rgba(255, 255, 255, 20),
        accent: Color::rgb(96, 205, 255),
        separator: Color::rgba(255, 255, 255, 20),
        placeholder: Color::rgb(128, 128, 128),
        border: Color::rgba(255, 255, 255, 30),
        hotkey_bg: Color::rgba(255, 255, 255, 15),
        hotkey_text: Color::rgb(180, 180, 180),
        selection: Color::rgba(96, 205, 255, 60),
    }
}

// ─── Theme Resolution ──────────────────────────────────────────────────────────

/// Resolve a `ThemeFile` into a fully populated `Theme` by inheriting missing
/// colors from the appropriate base (light or dark).
pub fn resolve_theme(
    file: &ThemeFile,
    light_base: &ResolvedThemeColors,
    dark_base: &ResolvedThemeColors,
) -> Theme {
    let base = if file.is_dark { dark_base } else { light_base };
    let colors = &file.colors;

    let resolved = ResolvedThemeColors {
        background: colors.background.unwrap_or(base.background),
        search_bg: colors.search_bg.unwrap_or(base.search_bg),
        text_primary: colors.text_primary.unwrap_or(base.text_primary),
        text_secondary: colors.text_secondary.unwrap_or(base.text_secondary),
        selected_bg: colors.selected_bg.unwrap_or(base.selected_bg),
        accent: colors.accent.unwrap_or(base.accent),
        separator: colors.separator.unwrap_or(base.separator),
        placeholder: colors.placeholder.unwrap_or(base.placeholder),
        border: colors.border.unwrap_or(base.border),
        hotkey_bg: colors.hotkey_bg.unwrap_or(base.hotkey_bg),
        hotkey_text: colors.hotkey_text.unwrap_or(base.hotkey_text),
        selection: colors.selection.unwrap_or(base.selection),
    };

    Theme {
        meta: file.meta(),
        colors: resolved,
    }
}

// ─── ThemeEngine ───────────────────────────────────────────────────────────────

/// The theme engine manages all loaded themes and provides the current active theme.
pub struct ThemeEngine {
    /// All loaded themes, keyed by name.
    themes: BTreeMap<String, Theme>,
    /// Name of the currently active theme.
    current: String,
    /// Light base colors for inheritance.
    light_base: ResolvedThemeColors,
    /// Dark base colors for inheritance.
    dark_base: ResolvedThemeColors,
}

impl ThemeEngine {
    /// Create a new `ThemeEngine` with built-in themes loaded.
    pub fn new() -> Self {
        let light_base = default_light_colors();
        let dark_base = default_dark_colors();

        let mut engine = Self {
            themes: BTreeMap::new(),
            current: String::from("Win11Light"),
            light_base,
            dark_base,
        };

        engine.load_builtin_themes();

        // Default to system mode
        if Self::detect_system_mode() {
            engine.current = String::from("Win11Dark");
        }

        engine
    }

    /// Load the three built-in themes: Win11Light, Win11Dark, and Auto (alias).
    pub fn load_builtin_themes(&mut self) {
        // Win11 Light — full base light theme
        let light_theme = Theme {
            meta: ThemeMeta {
                name: String::from("Win11Light"),
                is_dark: false,
                has_blur: false,
            },
            colors: self.light_base.clone(),
        };
        self.themes.insert(String::from("Win11Light"), light_theme);

        // Win11 Dark — full base dark theme
        let dark_theme = Theme {
            meta: ThemeMeta {
                name: String::from("Win11Dark"),
                is_dark: true,
                has_blur: false,
            },
            colors: self.dark_base.clone(),
        };
        self.themes.insert(String::from("Win11Dark"), dark_theme);

        // System Auto — resolves to light or dark based on system setting
        let is_dark = Self::detect_system_mode();
        let auto_theme = Theme {
            meta: ThemeMeta {
                name: String::from("System"),
                is_dark,
                has_blur: false,
            },
            colors: if is_dark {
                self.dark_base.clone()
            } else {
                self.light_base.clone()
            },
        };
        self.themes.insert(String::from("System"), auto_theme);
    }

    /// Scan a directory for `.json` theme files and load up to 50 (sorted by filename).
    ///
    /// Invalid files are silently skipped with a log-style message to stderr.
    pub fn load_user_themes(&mut self, dir: &Path) {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => return, // directory doesn't exist or can't be read
        };

        // Collect and sort .json file paths
        let mut json_files: Vec<std::path::PathBuf> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .map(|ext| ext.eq_ignore_ascii_case("json"))
                    .unwrap_or(false)
            })
            .collect();

        json_files.sort_by(|a, b| {
            a.file_name()
                .unwrap_or_default()
                .cmp(b.file_name().unwrap_or_default())
        });

        // Load up to 50
        for path in json_files.into_iter().take(50) {
            match self.load_theme_file(&path) {
                Ok(theme) => {
                    self.themes.insert(theme.meta.name.clone(), theme);
                }
                Err(_e) => {
                    // Silently skip invalid theme files in user directory
                    easysearch_core::log_warn!("failed to load theme file {:?}: {}", path, _e);
                }
            }
        }
    }

    /// Get a theme by name.
    pub fn get_theme(&self, name: &str) -> Result<&Theme, ThemeError> {
        self.themes
            .get(name)
            .ok_or_else(|| ThemeError::MissingField(format!("theme '{}' not found", name)))
    }

    /// Get the currently active theme.
    pub fn current_theme(&self) -> &Theme {
        self.themes
            .get(&self.current)
            .expect("current theme must always exist in the engine")
    }

    /// Switch the active theme to the one with the given name.
    pub fn set_theme(&mut self, name: &str) -> Result<(), ThemeError> {
        if !self.themes.contains_key(name) {
            return Err(ThemeError::MissingField(format!(
                "theme '{}' not found",
                name
            )));
        }
        self.current = name.to_string();
        Ok(())
    }

    /// List the names of all loaded themes.
    pub fn available_themes(&self) -> Vec<&str> {
        self.themes.keys().map(|s| s.as_str()).collect()
    }

    /// Detect whether the system is in dark mode.
    ///
    /// On Windows, reads the registry key
    /// `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme`.
    /// Returns `true` if system is in dark mode.
    pub fn detect_system_mode() -> bool {
        #[cfg(windows)]
        {
            use std::ffi::OsStr;
            use std::os::windows::ffi::OsStrExt;

            let subkey: Vec<u16> =
                OsStr::new(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();

            let value_name: Vec<u16> = OsStr::new("AppsUseLightTheme")
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            unsafe {
                use windows::Win32::System::Registry::{
                    HKEY_CURRENT_USER, KEY_READ, REG_DWORD, RegCloseKey, RegOpenKeyExW,
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
                    return false;
                }

                let mut data: u32 = 1;
                let mut data_size = std::mem::size_of::<u32>() as u32;
                let mut data_type = REG_DWORD;
                let result = RegQueryValueExW(
                    hkey,
                    windows::core::PCWSTR(value_name.as_ptr()),
                    None,
                    Some(&mut data_type),
                    Some(&mut data as *mut u32 as *mut u8),
                    Some(&mut data_size),
                );
                let _ = RegCloseKey(hkey);

                if result.is_ok() {
                    return data == 0; // 0 = dark mode, 1 = light mode
                }
            }
            false
        }

        #[cfg(not(windows))]
        {
            false
        }
    }

    // ─── Private helpers ───────────────────────────────────────────────────

    /// Load and validate a single theme file from disk.
    fn load_theme_file(&self, path: &Path) -> Result<Theme, ThemeError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ThemeError::InvalidJson(format!("failed to read file: {}", e)))?;

        self.parse_theme_json(&content)
    }

    /// Parse a JSON string into a resolved Theme.
    ///
    /// Validates:
    /// - JSON structure
    /// - Name length (max 64 characters)
    /// - Color hex values
    fn parse_theme_json(&self, json: &str) -> Result<Theme, ThemeError> {
        let file: ThemeFile =
            serde_json::from_str(json).map_err(|e| ThemeError::InvalidJson(e.to_string()))?;

        // Validate name length
        if file.name.len() > 64 {
            return Err(ThemeError::MissingField(format!(
                "name is too long ({}  chars, max 64)",
                file.name.len()
            )));
        }

        // Validate color hex values by checking each optional field
        validate_color_field(&file.colors.background, "background")?;
        validate_color_field(&file.colors.search_bg, "search_bg")?;
        validate_color_field(&file.colors.text_primary, "text_primary")?;
        validate_color_field(&file.colors.text_secondary, "text_secondary")?;
        validate_color_field(&file.colors.selected_bg, "selected_bg")?;
        validate_color_field(&file.colors.accent, "accent")?;
        validate_color_field(&file.colors.separator, "separator")?;
        validate_color_field(&file.colors.placeholder, "placeholder")?;
        validate_color_field(&file.colors.border, "border")?;
        validate_color_field(&file.colors.hotkey_bg, "hotkey_bg")?;
        validate_color_field(&file.colors.hotkey_text, "hotkey_text")?;
        validate_color_field(&file.colors.selection, "selection")?;

        Ok(resolve_theme(&file, &self.light_base, &self.dark_base))
    }
}

/// Validate that a color field (if present) has valid channel values.
/// Since colors are already parsed from hex by serde, this is a no-op for
/// successfully deserialized colors. This function exists as a structural
/// validation point for future extensions.
fn validate_color_field(_color: &Option<Color>, _field: &str) -> Result<(), ThemeError> {
    // Colors are validated during serde deserialization (from_hex).
    // If we reach here, the color is valid. No additional check needed.
    Ok(())
}

// ─── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_new_has_builtin_themes() {
        let engine = ThemeEngine::new();
        let names = engine.available_themes();
        assert!(names.contains(&"Win11Light"));
        assert!(names.contains(&"Win11Dark"));
        assert!(names.contains(&"System"));
    }

    #[test]
    fn engine_get_theme_exists() {
        let engine = ThemeEngine::new();
        assert!(engine.get_theme("Win11Light").is_ok());
        assert!(engine.get_theme("Win11Dark").is_ok());
    }

    #[test]
    fn engine_get_theme_not_found() {
        let engine = ThemeEngine::new();
        assert!(engine.get_theme("NonExistent").is_err());
    }

    #[test]
    fn engine_set_theme_valid() {
        let mut engine = ThemeEngine::new();
        assert!(engine.set_theme("Win11Dark").is_ok());
        assert_eq!(engine.current_theme().meta.name, "Win11Dark");
    }

    #[test]
    fn engine_set_theme_invalid() {
        let mut engine = ThemeEngine::new();
        assert!(engine.set_theme("NoSuchTheme").is_err());
    }

    #[test]
    fn resolve_theme_inherits_missing_colors() {
        let light_base = default_light_colors();
        let dark_base = default_dark_colors();

        // A dark theme with only accent specified
        let file = ThemeFile {
            name: "Partial".to_string(),
            is_dark: true,
            has_blur: false,
            colors: ThemeColors {
                accent: Some(Color::rgb(255, 0, 0)),
                ..ThemeColors::default()
            },
        };

        let theme = resolve_theme(&file, &light_base, &dark_base);

        // Accent should be our custom value
        let eps = 1.0 / 255.0 + f32::EPSILON;
        assert!((theme.colors.accent.r - 1.0).abs() <= eps);

        // Background should be inherited from dark base
        assert!((theme.colors.background.r - dark_base.background.r).abs() <= eps);
        assert!((theme.colors.background.g - dark_base.background.g).abs() <= eps);
    }

    #[test]
    fn parse_invalid_json() {
        let engine = ThemeEngine::new();
        let result = engine.parse_theme_json("not json at all");
        assert!(result.is_err());
        match result.unwrap_err() {
            ThemeError::InvalidJson(_) => {}
            other => panic!("expected InvalidJson, got {:?}", other),
        }
    }

    #[test]
    fn parse_name_too_long() {
        let engine = ThemeEngine::new();
        let long_name = "A".repeat(65);
        let json = format!(
            r#"{{"name": "{}", "is_dark": false, "has_blur": false, "colors": {{}}}}"#,
            long_name
        );
        let result = engine.parse_theme_json(&json);
        assert!(result.is_err());
        match result.unwrap_err() {
            ThemeError::MissingField(_) => {}
            other => panic!("expected MissingField, got {:?}", other),
        }
    }

    #[test]
    fn load_user_themes_nonexistent_dir() {
        let mut engine = ThemeEngine::new();
        // Should not panic on nonexistent directory
        engine.load_user_themes(Path::new("/nonexistent/path/that/does/not/exist"));
        // Built-in themes still present
        assert!(engine.available_themes().contains(&"Win11Light"));
    }

    #[test]
    fn load_user_themes_from_temp_dir() {
        let dir = std::env::temp_dir().join("easysearch_theme_test");
        let _ = std::fs::create_dir_all(&dir);

        // Write a valid theme file
        let theme_json = r##"{
            "name": "TestCustom",
            "is_dark": false,
            "has_blur": true,
            "colors": {
                "accent": "#FF0000"
            }
        }"##;
        std::fs::write(dir.join("custom.json"), theme_json).unwrap();

        let mut engine = ThemeEngine::new();
        engine.load_user_themes(&dir);

        assert!(engine.available_themes().contains(&"TestCustom"));
        let custom = engine.get_theme("TestCustom").unwrap();
        assert!(!custom.meta.is_dark);
        assert!(custom.meta.has_blur);

        // Clean up
        let _ = std::fs::remove_dir_all(&dir);
    }
}
