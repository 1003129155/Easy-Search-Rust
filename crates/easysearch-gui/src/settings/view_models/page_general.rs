// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! General settings page view model — language selection, autostart toggle, drive config.
//!
//! # Requirements
//! - Req 3.5: Each Setting_Page has independent View + ViewModel
//! - Req 5.2: Support en, zh-CN, ja languages
//! - Req 4.4: autostart setting

/// Available languages for the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    Chinese,
    Japanese,
}

impl Language {
    /// Display label for the language.
    pub fn label(&self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Chinese => "中文",
            Self::Japanese => "日本語",
        }
    }

    /// All available languages.
    pub const ALL: &'static [Language] = &[
        Language::English,
        Language::Chinese,
        Language::Japanese,
    ];
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// State for the general settings page.
#[derive(Debug, Clone)]
pub struct GeneralViewModel {
    /// Currently selected language.
    pub selected_language: Language,
    /// Whether autostart is enabled.
    pub autostart_enabled: bool,
    /// Drive letters to index (e.g. ["C", "D"]).
    pub index_drives: Vec<String>,
}

/// Messages for the general settings page.
#[derive(Debug, Clone)]
pub enum GeneralMessage {
    /// User selected a new language.
    LanguageChanged(Language),
    /// User toggled the autostart switch.
    AutostartToggled(bool),
    /// User changed the index drives list.
    DrivesChanged(Vec<String>),
}

impl GeneralViewModel {
    /// Create a new GeneralViewModel with defaults.
    pub fn new() -> Self {
        Self {
            selected_language: Language::English,
            autostart_enabled: false,
            index_drives: Vec::new(),
        }
    }

    /// Handle an incoming message.
    pub fn update(&mut self, msg: GeneralMessage) {
        match msg {
            GeneralMessage::LanguageChanged(lang) => {
                self.selected_language = lang;
            }
            GeneralMessage::AutostartToggled(enabled) => {
                self.autostart_enabled = enabled;
            }
            GeneralMessage::DrivesChanged(drives) => {
                self.index_drives = drives;
            }
        }
    }
}
