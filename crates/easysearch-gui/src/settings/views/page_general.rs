// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! General settings page view — language dropdown, autostart toggle, drives config.
//!
//! # Requirements
//! - Req 3.5: Each Setting_Page has independent View + ViewModel
//! - Req 5.2: Support en, zh-CN, ja languages

use iced::widget::{column, pick_list, row, text, text_input, toggler};
use iced::Element;

use super::super::view_models::page_general::{GeneralMessage, GeneralViewModel, Language};

/// Render the general settings page.
pub fn view<'a>(vm: &'a GeneralViewModel) -> Element<'a, GeneralMessage> {
    let title = text("通用设置").size(22);

    // Language selection
    let lang_label = text("语言 / Language").size(14);
    let lang_picker = pick_list(
        Language::ALL,
        Some(vm.selected_language),
        GeneralMessage::LanguageChanged,
    )
    .width(200);

    let lang_row = row![lang_label, lang_picker].spacing(16).align_y(iced::Alignment::Center);

    // Autostart toggle
    let autostart_toggle = toggler(vm.autostart_enabled)
        .label("开机自动启动")
        .on_toggle(GeneralMessage::AutostartToggled);

    // Index drives configuration
    let drives_label = text("索引盘符 (逗号分隔, 如 C,D,E)").size(14);
    let drives_str = vm.index_drives.join(",");
    let drives_input = text_input("C,D,E", &drives_str)
        .on_input(|input| {
            let drives: Vec<String> = input
                .split(',')
                .map(|s| s.trim().to_uppercase())
                .filter(|s| !s.is_empty() && s.len() == 1 && s.chars().next().unwrap().is_ascii_alphabetic())
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
