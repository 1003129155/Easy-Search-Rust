// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Theme settings page view.

use iced::widget::{column, pick_list, row, text};
use iced::Element;

use crate::i18n::engine::I18nEngine;

use super::super::view_models::page_theme::{ThemeMessage, ThemeOption, ThemeViewModel};

/// Render the theme settings page.
pub fn view<'a>(vm: &'a ThemeViewModel, i18n: &'a I18nEngine) -> Element<'a, ThemeMessage> {
    let title = text(i18n.get("settings_theme_title")).size(22);
    let theme_label = text(i18n.get("settings_theme_label")).size(14);
    let theme_picker = pick_list(
        ThemeOption::ALL,
        Some(vm.selected_theme),
        ThemeMessage::ThemeSelected,
    )
    .width(200);

    let theme_row = row![theme_label, theme_picker]
        .spacing(16)
        .align_y(iced::Alignment::Center);

    let preview_label = text(format!(
        "{}: {}",
        i18n.get("settings_theme_current"),
        i18n.get(vm.selected_theme.label_key())
    ))
    .size(13);

    column![title, theme_row, preview_label]
        .spacing(20)
        .padding(24)
        .into()
}
