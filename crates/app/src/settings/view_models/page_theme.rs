// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Theme settings page view model.

/// Available theme options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeOption {
    Win11Light,
    Win11Dark,
    System,
}

impl ThemeOption {
    /// Stable display label used by the pick list widget.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Win11Light => "Light",
            Self::Win11Dark => "Dark",
            Self::System => "System",
        }
    }

    /// Translation key for the theme label.
    pub fn label_key(&self) -> &'static str {
        match self {
            Self::Win11Light => "settings_theme_light",
            Self::Win11Dark => "settings_theme_dark",
            Self::System => "settings_theme_system",
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
