// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Welcome wizard — Page 4: Theme selection.

use iced::widget::{button, column, row, text, Space};
use iced::{Element, Length};

use crate::welcome::app::Message;
use crate::welcome::state::WelcomeState;

/// Available theme options.
const THEMES: &[(&str, &str)] = &[
    ("win11_light", "浅色模式"),
    ("win11_dark", "深色模式"),
];

/// Render the theme selection page.
pub fn view(state: &WelcomeState) -> Element<'_, Message> {
    let title = text("选择主题").size(24);
    let desc = text("选择一个你喜欢的外观风格：").size(14);

    let theme_buttons: Vec<Element<'_, Message>> = THEMES
        .iter()
        .map(|&(id, label)| {
            let is_selected = state.theme == id;
            let label_text = if is_selected {
                format!("✓ {label}")
            } else {
                label.to_string()
            };

            let btn = button(text(label_text).size(14))
                .on_press(Message::SelectTheme(id.to_string()))
                .padding([12, 24]);

            btn.into()
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
