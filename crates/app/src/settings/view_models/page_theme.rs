// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Theme settings page view model — theme list and selection.
//!
//! # Requirements
//! - Req 2.5: User selects new theme in Setting_Window
//! - Req 2.6: Built-in themes: Win11Light, Win11Dark, System
//! - Req 3.5: Each Setting_Page has independent View + ViewModel

/// Available theme options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeOption {
    Win11Light,
    Win11Dark,
    System,
}

impl ThemeOption {
    /// Display label for the theme.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Win11Light => "Win11 Light",
            Self::Win11Dark => "Win11 Dark",
            Self::System => "跟随系统",
        }
    }

    /// All available themes.
    pub const ALL: &'static [ThemeOption] = &[
        ThemeOption::Win11Light,
        ThemeOption::Win11Dark,
        ThemeOption::System,
    ];
}

impl std::fmt::Display for ThemeOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// State for the theme settings page.
#[derive(Debug, Clone)]
pub struct ThemeViewModel {
    /// Currently selected theme.
    pub selected_theme: ThemeOption,
}

/// Messages for the theme settings page.
#[derive(Debug, Clone)]
pub enum ThemeMessage {
    /// User selected a different theme.
    ThemeSelected(ThemeOption),
}

impl ThemeViewModel {
    /// Create a new ThemeViewModel with default (System).
    pub fn new() -> Self {
        Self {
            selected_theme: ThemeOption::System,
        }
    }

    /// Handle an incoming message.
    pub fn update(&mut self, msg: ThemeMessage) {
        match msg {
            ThemeMessage::ThemeSelected(theme) => {
                self.selected_theme = theme;
            }
        }
    }
}
