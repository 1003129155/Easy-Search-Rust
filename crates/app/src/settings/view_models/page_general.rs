// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! General settings page view model.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    Chinese,
    Japanese,
}

impl Language {
    pub fn label(&self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Chinese => "中文",
            Self::Japanese => "日本語",
        }
    }

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

#[derive(Debug, Clone)]
pub struct GeneralViewModel {
    pub selected_language: Language,
    pub autostart_enabled: bool,
    pub index_drives: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum GeneralMessage {
    LanguageChanged(Language),
    AutostartToggled(bool),
    DrivesChanged(Vec<String>),
}

impl GeneralViewModel {
    pub fn new() -> Self {
        Self {
            selected_language: Language::English,
            autostart_enabled: false,
            index_drives: Vec::new(),
        }
    }

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
