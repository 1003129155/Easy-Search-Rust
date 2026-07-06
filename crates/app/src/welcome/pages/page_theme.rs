// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Welcome wizard page 4.

use iced::widget::{button, column, row, text, Space};
use iced::{Element, Length};

use crate::i18n::engine::I18nEngine;
use crate::welcome::app::Message;
use crate::welcome::state::WelcomeState;

const THEMES: &[&str] = &["win11_light", "win11_dark"];

fn theme_label_key(theme: &str) -> &'static str {
    match theme {
        "win11_dark" => "welcome_theme_dark",
        _ => "welcome_theme_light",
    }
}

pub fn view<'a>(state: &'a WelcomeState, i18n: &'a I18nEngine) -> Element<'a, Message> {
    let title = text(i18n.get("welcome_theme_title")).size(24);
    let desc = text(i18n.get("welcome_theme_desc")).size(14);

    let theme_buttons: Vec<Element<'_, Message>> = THEMES
        .iter()
        .map(|&id| {
            let is_selected = state.theme == id;
            let label = i18n.get(theme_label_key(id));
            let label_text = if is_selected {
                format!("* {label}")
            } else {
                label.to_string()
            };

            button(text(label_text).size(14))
                .on_press(Message::SelectTheme(id.to_string()))
                .padding([12, 24])
                .into()
        })
        .collect();

    let theme_row = row(theme_buttons).spacing(16);

    column![
        Space::with_height(40),
        title,
        Space::with_height(16),
        desc,
        Space::with_height(32),
        theme_row,
        Space::with_height(Length::Fill),
    ]
    .width(Length::Fill)
    .into()
}
