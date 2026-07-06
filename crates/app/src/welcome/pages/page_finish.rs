// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Welcome wizard page 5.

use iced::widget::{checkbox, column, text, Space};
use iced::{Element, Length};

use crate::i18n::engine::I18nEngine;
use crate::welcome::app::Message;
use crate::welcome::state::WelcomeState;

pub fn view<'a>(state: &'a WelcomeState, i18n: &'a I18nEngine) -> Element<'a, Message> {
    let title = text(i18n.get("welcome_finish_title")).size(24);
    let desc = text(i18n.get("welcome_finish_desc")).size(14);

    let autostart_toggle = checkbox(i18n.get("welcome_finish_autostart"), state.autostart)
        .on_toggle(Message::ToggleAutostart)
        .size(18);

    let hint = text(i18n.get("welcome_finish_hint")).size(12);

    column![
        Space::with_height(40),
        title,
        Space::with_height(16),
        desc,
        Space::with_height(40),
        autostart_toggle,
        Space::with_height(24),
        hint,
        Space::with_height(Length::Fill),
    ]
    .width(Length::Fill)
    .into()
}
