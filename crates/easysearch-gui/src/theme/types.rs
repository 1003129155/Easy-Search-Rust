// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Theme type system — Color, ThemeMeta, ThemeColors, and Theme definitions.
//!
//! Colors are stored internally as f32 RGBA (0.0–1.0) and serialized as
//! `#RRGGBB` (opaque) or `#AARRGGBB` (with alpha) hex strings.

use serde::{Deserialize, Serialize};
use std::fmt;

// ─── Error Types ───────────────────────────────────────────────────────────────

/// Errors that can occur when parsing or loading themes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeError {
    /// The hex color string is invalid.
    InvalidColor { field: String, value: String },
    /// A required metadata field is missing.
    MissingField(String),
    /// The JSON is malformed.
    InvalidJson(String),
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThemeError::InvalidColor { field, value } => {
                write!(f, "invalid color for field '{}': '{}'", field, value)
            }
            ThemeError::MissingField(name) => {
                write!(f, "missing required field: '{}'", name)
            }
            ThemeError::InvalidJson(msg) => {
                write!(f, "invalid JSON: {}", msg)
            }
        }
    }
}

impl std::error::Error for ThemeError {}

// ─── Color ─────────────────────────────────────────────────────────────────────

/// RGBA color with f32 channels (0.0–1.0).
///
/// Serialized as `#RRGGBB` when alpha is 1.0 (255), or `#AARRGGBB` otherwise.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    /// Create an opaque color from 8-bit RGB values.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        }
    }

    /// Create a color from 8-bit RGBA values.
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        }
    }

    /// Parse a color from a hex string.
    ///
    /// Accepts:
    /// - `#RRGGBB` — opaque color (alpha = 1.0)
    /// - `#AARRGGBB` — color with alpha
    ///
    /// Hex characters are case-insensitive.
    pub fn from_hex(s: &str) -> Result<Self, ThemeError> {
        let s = s.trim();
        if !s.starts_with('#') {
            return Err(ThemeError::InvalidColor {
                field: String::new(),
                value: s.to_string(),
            });
        }

        let hex = &s[1..];
        match hex.len() {
            6 => {
                // #RRGGBB
                let r = parse_hex_byte(&hex[0..2], s)?;
                let g = parse_hex_byte(&hex[2..4], s)?;
                let b = parse_hex_byte(&hex[4..6], s)?;
                Ok(Color::rgb(r, g, b))
            }
            8 => {
                // #AARRGGBB
                let a = parse_hex_byte(&hex[0..2], s)?;
                let r = parse_hex_byte(&hex[2..4], s)?;
                let g = parse_hex_byte(&hex[4..6], s)?;
                let b = parse_hex_byte(&hex[6..8], s)?;
                Ok(Color::rgba(r, g, b, a))
            }
            _ => Err(ThemeError::InvalidColor {
                field: String::new(),
                value: s.to_string(),
            }),
        }
    }

    /// Format this color as a hex string.
    ///
    /// Returns `#RRGGBB` if alpha rounds to 255, otherwise `#AARRGGBB`.
    pub fn to_hex(&self) -> String {
        let r = (self.r * 255.0).round() as u8;
        let g = (self.g * 255.0).round() as u8;
        let b = (self.b * 255.0).round() as u8;
        let a = (self.a * 255.0).round() as u8;

        if a == 255 {
            format!("#{:02X}{:02X}{:02X}", r, g, b)
        } else {
            format!("#{:02X}{:02X}{:02X}{:02X}", a, r, g, b)
        }
    }
}

/// Parse a 2-character hex string into a u8.
fn parse_hex_byte(hex: &str, original: &str) -> Result<u8, ThemeError> {
    u8::from_str_radix(hex, 16).map_err(|_| ThemeError::InvalidColor {
        field: String::new(),
        value: original.to_string(),
    })
}

// ─── Color Serde ───────────────────────────────────────────────────────────────

impl Serialize for Color {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Color::from_hex(&s).map_err(|e| serde::de::Error::custom(e.to_string()))
    }
}

// ─── ThemeMeta ─────────────────────────────────────────────────────────────────

/// Theme metadata describing the theme's identity and features.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThemeMeta {
    /// Display name of the theme (max 64 characters).
    pub name: String,
    /// Whether this is a dark theme.
    pub is_dark: bool,
    /// Whether this theme supports window blur/transparency.
    pub has_blur: bool,
}

// ─── ThemeColors (partial, for JSON deserialization) ────────────────────────────

/// Partial theme colors — all fields are optional to support inheritance.
///
/// When loading from JSON, missing fields will be inherited from the base theme.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ThemeColors {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_bg: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_primary: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_secondary: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_bg: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub separator: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hotkey_bg: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hotkey_text: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<Color>,
}

// ─── ResolvedThemeColors (complete, for runtime use) ───────────────────────────

/// Fully resolved theme colors — all 12 fields guaranteed present.
///
/// Produced after the theme engine resolves inheritance from the base theme.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResolvedThemeColors {
    pub background: Color,
    pub search_bg: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub selected_bg: Color,
    pub accent: Color,
    pub separator: Color,
    pub placeholder: Color,
    pub border: Color,
    pub hotkey_bg: Color,
    pub hotkey_text: Color,
    pub selection: Color,
}

// ─── ThemeFile (JSON file representation) ──────────────────────────────────────

/// Represents the raw JSON theme file structure.
///
/// This is what gets deserialized from disk. Colors may be partial
/// (fields can be missing for inheritance from base).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThemeFile {
    /// Theme display name.
    pub name: String,
    /// Whether this is a dark theme.
    pub is_dark: bool,
    /// Whether this theme supports blur effects.
    pub has_blur: bool,
    /// Color definitions (partial — missing fields inherit from base).
    #[serde(default)]
    pub colors: ThemeColors,
}

impl ThemeFile {
    /// Extract metadata from this theme file.
    pub fn meta(&self) -> ThemeMeta {
        ThemeMeta {
            name: self.name.clone(),
            is_dark: self.is_dark,
            has_blur: self.has_blur,
        }
    }
}

// ─── Theme (resolved, runtime representation) ──────────────────────────────────

/// A fully resolved theme ready for runtime use.
///
/// All color fields are guaranteed to have values (after inheritance resolution).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Theme {
    /// Theme metadata.
    pub meta: ThemeMeta,
    /// Fully resolved color values.
    pub colors: ResolvedThemeColors,
}

// ─── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_from_hex_rrggbb() {
        let c = Color::from_hex("#FF8000").unwrap();
        assert_eq!((c.r * 255.0).round() as u8, 255);
        assert_eq!((c.g * 255.0).round() as u8, 128);
        assert_eq!((c.b * 255.0).round() as u8, 0);
        assert_eq!((c.a * 255.0).round() as u8, 255);
    }

    #[test]
    fn color_from_hex_aarrggbb() {
        let c = Color::from_hex("#80FF0000").unwrap();
        assert_eq!((c.a * 255.0).round() as u8, 128);
        assert_eq!((c.r * 255.0).round() as u8, 255);
        assert_eq!((c.g * 255.0).round() as u8, 0);
        assert_eq!((c.b * 255.0).round() as u8, 0);
    }

    #[test]
    fn color_from_hex_case_insensitive() {
        let c1 = Color::from_hex("#aabbcc").unwrap();
        let c2 = Color::from_hex("#AABBCC").unwrap();
        assert_eq!(c1.r, c2.r);
        assert_eq!(c1.g, c2.g);
        assert_eq!(c1.b, c2.b);
    }

    #[test]
    fn color_from_hex_invalid() {
        assert!(Color::from_hex("FF8000").is_err()); // missing #
        assert!(Color::from_hex("#GG0000").is_err()); // invalid hex
        assert!(Color::from_hex("#FFF").is_err()); // wrong length
        assert!(Color::from_hex("#FFFFFFFFF").is_err()); // too long
    }

    #[test]
    fn color_to_hex_opaque() {
        let c = Color::rgb(255, 128, 0);
        assert_eq!(c.to_hex(), "#FF8000");
    }

    #[test]
    fn color_to_hex_with_alpha() {
        let c = Color::rgba(255, 0, 0, 128);
        assert_eq!(c.to_hex(), "#80FF0000");
    }

    #[test]
    fn color_roundtrip() {
        let original = Color::rgba(100, 200, 50, 180);
        let hex = original.to_hex();
        let parsed = Color::from_hex(&hex).unwrap();

        // Allow ≤ 1/255 difference due to f32 rounding
        let eps = 1.0 / 255.0 + f32::EPSILON;
        assert!((original.r - parsed.r).abs() <= eps);
        assert!((original.g - parsed.g).abs() <= eps);
        assert!((original.b - parsed.b).abs() <= eps);
        assert!((original.a - parsed.a).abs() <= eps);
    }

    #[test]
    fn color_serde_roundtrip() {
        let c = Color::rgba(200, 100, 50, 180);
        let json = serde_json::to_string(&c).unwrap();
        let parsed: Color = serde_json::from_str(&json).unwrap();

        let eps = 1.0 / 255.0 + f32::EPSILON;
        assert!((c.r - parsed.r).abs() <= eps);
        assert!((c.g - parsed.g).abs() <= eps);
        assert!((c.b - parsed.b).abs() <= eps);
        assert!((c.a - parsed.a).abs() <= eps);
    }

    #[test]
    fn theme_file_deserialize_full() {
        let json = r##"{
            "name": "Test Theme",
            "is_dark": true,
            "has_blur": false,
            "colors": {
                "background": "#282828",
                "search_bg": "#282828",
                "text_primary": "#FFFFFF",
                "text_secondary": "#B4B4B4",
                "selected_bg": "#14FFFFFF",
                "accent": "#60CDFF",
                "separator": "#14FFFFFF",
                "placeholder": "#808080",
                "border": "#1EFFFFFF",
                "hotkey_bg": "#0FFFFFFF",
                "hotkey_text": "#B4B4B4",
                "selection": "#3C60CDFF"
            }
        }"##;

        let theme_file: ThemeFile = serde_json::from_str(json).unwrap();
        assert_eq!(theme_file.name, "Test Theme");
        assert!(theme_file.is_dark);
        assert!(!theme_file.has_blur);
        assert!(theme_file.colors.background.is_some());
        assert!(theme_file.colors.selection.is_some());
    }

    #[test]
    fn theme_file_deserialize_partial() {
        let json = r##"{
            "name": "Partial Theme",
            "is_dark": false,
            "has_blur": true,
            "colors": {
                "background": "#FCFCFC",
                "accent": "#0067C0"
            }
        }"##;

        let theme_file: ThemeFile = serde_json::from_str(json).unwrap();
        assert_eq!(theme_file.name, "Partial Theme");
        assert!(theme_file.colors.background.is_some());
        assert!(theme_file.colors.accent.is_some());
        // Missing fields should be None
        assert!(theme_file.colors.search_bg.is_none());
        assert!(theme_file.colors.text_primary.is_none());
        assert!(theme_file.colors.separator.is_none());
    }

    #[test]
    fn theme_file_serialize_roundtrip() {
        let theme_file = ThemeFile {
            name: "Round Trip".to_string(),
            is_dark: true,
            has_blur: false,
            colors: ThemeColors {
                background: Some(Color::rgb(40, 40, 40)),
                search_bg: Some(Color::rgb(40, 40, 40)),
                text_primary: Some(Color::rgb(255, 255, 255)),
                text_secondary: Some(Color::rgb(180, 180, 180)),
                selected_bg: Some(Color::rgba(255, 255, 255, 20)),
                accent: Some(Color::rgb(96, 205, 255)),
                separator: Some(Color::rgba(255, 255, 255, 20)),
                placeholder: Some(Color::rgb(128, 128, 128)),
                border: Some(Color::rgba(255, 255, 255, 30)),
                hotkey_bg: Some(Color::rgba(255, 255, 255, 15)),
                hotkey_text: Some(Color::rgb(180, 180, 180)),
                selection: Some(Color::rgba(96, 205, 255, 60)),
            },
        };

        let json = serde_json::to_string_pretty(&theme_file).unwrap();
        let parsed: ThemeFile = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.name, theme_file.name);
        assert_eq!(parsed.is_dark, theme_file.is_dark);
        assert_eq!(parsed.has_blur, theme_file.has_blur);

        // Check color roundtrip within tolerance
        let eps = 1.0 / 255.0 + f32::EPSILON;
        let orig_bg = theme_file.colors.background.unwrap();
        let parsed_bg = parsed.colors.background.unwrap();
        assert!((orig_bg.r - parsed_bg.r).abs() <= eps);
        assert!((orig_bg.g - parsed_bg.g).abs() <= eps);
        assert!((orig_bg.b - parsed_bg.b).abs() <= eps);
        assert!((orig_bg.a - parsed_bg.a).abs() <= eps);
    }

    #[test]
    fn theme_meta_extraction() {
        let tf = ThemeFile {
            name: "My Theme".to_string(),
            is_dark: false,
            has_blur: true,
            colors: ThemeColors::default(),
        };
        let meta = tf.meta();
        assert_eq!(meta.name, "My Theme");
        assert!(!meta.is_dark);
        assert!(meta.has_blur);
    }
}
