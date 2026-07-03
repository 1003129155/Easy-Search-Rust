// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Welcome wizard state — tracks the current page and user choices.

/// All pages in the welcome wizard, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WelcomePage {
    /// Page 1: Welcome text + language selection.
    Welcome,
    /// Page 2: Set activation hotkey.
    Hotkey,
    /// Page 3: Feature introduction.
    Features,
    /// Page 4: Theme selection.
    Theme,
    /// Page 5: Autostart toggle + done.
    Finish,
}

impl WelcomePage {
    /// Total number of wizard pages.
    pub const COUNT: usize = 5;

    /// Get the page index (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Welcome => 0,
            Self::Hotkey => 1,
            Self::Features => 2,
            Self::Theme => 3,
            Self::Finish => 4,
        }
    }

    /// Get the page from an index (clamped to valid range).
    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => Self::Welcome,
            1 => Self::Hotkey,
            2 => Self::Features,
            3 => Self::Theme,
            _ => Self::Finish,
        }
    }

    /// Get the next page, or None if this is the last.
    pub fn next(&self) -> Option<Self> {
        let idx = self.index();
        if idx + 1 < Self::COUNT {
            Some(Self::from_index(idx + 1))
        } else {
            None
        }
    }

    /// Get the previous page, or None if this is the first.
    pub fn prev(&self) -> Option<Self> {
        let idx = self.index();
        if idx > 0 {
            Some(Self::from_index(idx - 1))
        } else {
            None
        }
    }
}

/// State for the welcome wizard — user choices accumulated across pages.
#[derive(Debug, Clone)]
pub struct WelcomeState {
    /// Current wizard page.
    pub current_page: WelcomePage,
    /// Selected language code (e.g. "zh-CN", "en", "ja").
    pub language: String,
    /// Selected theme name (e.g. "win11_light", "win11_dark").
    pub theme: String,
    /// Whether to enable autostart.
    pub autostart: bool,
}

impl Default for WelcomeState {
    fn default() -> Self {
        Self {
            current_page: WelcomePage::Welcome,
            language: "zh-CN".to_string(),
            theme: "win11_light".to_string(),
            autostart: true,
        }
    }
}
