// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Welcome wizard page 2.

use iced::widget::{column, text, Space};
use iced::{Element, Length};

use crate::i18n::engine::I18nEngine;
use crate::welcome::app::Message;
use crate::welcome::state::WelcomeState;

pub fn view<'a>(_state: &'a WelcomeState, i18n: &'a I18nEngine) -> Element<'a, Message> {
    let title = text(i18n.get("welcome_hotkey_title")).size(24);
    let desc = text(i18n.get("welcome_hotkey_desc")).size(14);
    let hotkey_display = text("Alt + Space").size(32);
    let note = text(i18n.get("welcome_hotkey_note")).size(12);

    column![
        Space::with_height(40),
        title,
        Space::with_height(16),
        desc,
        Space::with_height(40),
        hotkey_display,
        Space::with_height(24),
        note,
        Space::with_height(Length::Fill),
    ]
    .width(Length::Fill)
    .into()
}
