// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Welcome wizard page 1.

use iced::widget::{column, pick_list, text, Space};
use iced::{Element, Length};

use crate::i18n::engine::I18nEngine;
use crate::welcome::app::Message;
use crate::welcome::state::WelcomeState;

const LANGUAGES: &[&str] = &["zh-CN", "en", "ja"];

pub fn view<'a>(state: &'a WelcomeState, i18n: &'a I18nEngine) -> Element<'a, Message> {
    let title = text(i18n.get("welcome_title")).size(28);
    let subtitle = text(i18n.get("welcome_subtitle")).size(16);
    let lang_label = text(i18n.get("welcome_language_label")).size(14);

    let selected = LANGUAGES.iter().find(|&&l| l == state.language).copied();

    let lang_picker = pick_list(
        LANGUAGES.to_vec(),
        selected,
        |s: &str| Message::SelectLanguage(s.to_string()),
    )
    .placeholder(i18n.get("welcome_language_placeholder"));

    column![
        Space::with_height(40),
        title,
        Space::with_height(12),
        subtitle,
        Space::with_height(40),
        lang_label,
        Space::with_height(8),
        lang_picker,
        Space::with_height(Length::Fill),
    ]
    .width(Length::Fill)
    .into()
}
