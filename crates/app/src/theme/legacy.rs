// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Legacy theme types — temporary bridge until the full JSON theme engine is implemented.

/// RGBA color with f32 channels (0.0–1.0).
#[derive(Debug, Clone, Copy)]
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
}

/// Theme with 12 semantic color slots (mirrors Flow.Launcher Win11 themes).
#[derive(Debug, Clone)]
pub struct Theme {
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

impl Theme {
    /// Light theme (Win11 Light style).
    pub fn light() -> Self {
        Self {
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

    /// Dark theme (Win11 Dark style).
    pub fn dark() -> Self {
        Self {
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

    /// Choose light or dark based on system setting.
    pub fn system() -> Self {
        if is_dark_mode() {
            Self::dark()
        } else {
            Self::light()
        }
    }
}

/// Detect Windows system dark mode via registry.
fn is_dark_mode() -> bool {
    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        // Read HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme
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
