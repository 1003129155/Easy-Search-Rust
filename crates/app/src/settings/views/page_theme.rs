// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Theme settings page view — theme list with selection and preview.
//!
//! # Requirements
//! - Req 2.5: User selects new theme in Setting_Window
//! - Req 2.6: Built-in themes: Win11Light, Win11Dark, System

use iced::widget::{column, pick_list, row, text};
use iced::Element;

use super::super::view_models::page_theme::{ThemeMessage, ThemeOption, ThemeViewModel};

/// Render the theme settings page.
pub fn view<'a>(vm: &'a ThemeViewModel) -> Element<'a, ThemeMessage> {
    let title = text("外观主题").size(22);

    // Theme selection
    let theme_label = text("选择主题").size(14);
    let theme_picker = pick_list(
        ThemeOption::ALL,
        Some(vm.selected_theme),
        ThemeMessage::ThemeSelected,
    )
    .width(200);

    let theme_row = row![theme_label, theme_picker].spacing(16).align_y(iced::Alignment::Center);

    // Preview indicator
    let preview_label = text(format!("当前主题: {}", vm.selected_theme.label())).size(13);

    column![title, theme_row, preview_label]
        .spacing(20)
        .padding(24)
        .into()
}
