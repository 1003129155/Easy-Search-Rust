// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! General settings page view.

use iced::widget::{column, pick_list, row, text, text_input, toggler};
use iced::Element;

use crate::i18n::engine::I18nEngine;

use super::super::view_models::page_general::{GeneralMessage, GeneralViewModel, Language};

/// Render the general settings page.
pub fn view<'a>(vm: &'a GeneralViewModel, i18n: &'a I18nEngine) -> Element<'a, GeneralMessage> {
    let title = text(i18n.get("settings_general_title")).size(22);

    let lang_label = text(i18n.get("settings_language_label")).size(14);
    let lang_picker = pick_list(
        Language::ALL,
        Some(vm.selected_language),
        GeneralMessage::LanguageChanged,
    )
    .width(200);

    let lang_row = row![lang_label, lang_picker]
        .spacing(16)
        .align_y(iced::Alignment::Center);

    let autostart_toggle = toggler(vm.autostart_enabled)
        .label(i18n.get("settings_autostart_label"))
        .on_toggle(GeneralMessage::AutostartToggled);

    let drives_label = text(i18n.get("settings_drives_label")).size(14);
    let drives_str = vm.index_drives.join(",");
    let drives_input = text_input(i18n.get("settings_drives_placeholder"), &drives_str)
        .on_input(|input| {
            let drives: Vec<String> = input
                .split(',')
                .map(|s| s.trim().to_uppercase())
                .filter(|s| {
                    !s.is_empty() && s.len() == 1 && s.chars().next().unwrap().is_ascii_alphabetic()
                })
                .collect();
            GeneralMessage::DrivesChanged(drives)
        })
        .width(300);

    let drives_row = column![drives_label, drives_input].spacing(8);

    column![title, lang_row, autostart_toggle, drives_row]
        .spacing(20)
        .padding(24)
        .into()
}
