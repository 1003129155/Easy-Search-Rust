// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Welcome wizard state.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WelcomePage {
    Welcome,
    Hotkey,
    Features,
    Theme,
    Finish,
}

impl WelcomePage {
    pub const COUNT: usize = 5;

    pub fn index(&self) -> usize {
        match self {
            Self::Welcome => 0,
            Self::Hotkey => 1,
            Self::Features => 2,
            Self::Theme => 3,
            Self::Finish => 4,
        }
    }

    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => Self::Welcome,
            1 => Self::Hotkey,
            2 => Self::Features,
            3 => Self::Theme,
            _ => Self::Finish,
        }
    }

    pub fn next(&self) -> Option<Self> {
        let idx = self.index();
        if idx + 1 < Self::COUNT {
            Some(Self::from_index(idx + 1))
        } else {
            None
        }
    }

    pub fn prev(&self) -> Option<Self> {
        let idx = self.index();
        if idx > 0 {
            Some(Self::from_index(idx - 1))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct WelcomeState {
    pub current_page: WelcomePage,
    pub language: String,
    pub theme: String,
    pub autostart: bool,
}

impl Default for WelcomeState {
    fn default() -> Self {
        let detected = crate::i18n::engine::I18nEngine::detect_system_locale();
        let language = if detected.starts_with("zh") {
            "zh-CN"
        } else if detected.starts_with("ja") {
            "ja"
        } else {
            "en"
        };

        Self {
            current_page: WelcomePage::Welcome,
            language: language.to_string(),
            theme: "win11_light".to_string(),
            autostart: true,
        }
    }
}
