// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Hotkey settings page view.

use iced::widget::{button, column, row, text};
use iced::Element;

use crate::i18n::engine::I18nEngine;

use super::super::view_models::page_hotkey::{HotkeyMessage, HotkeyViewModel};

/// Render the hotkey settings page.
pub fn view<'a>(vm: &'a HotkeyViewModel, i18n: &'a I18nEngine) -> Element<'a, HotkeyMessage> {
    let title = text(i18n.get("settings_hotkey_title")).size(22);

    let hotkey_label = text(i18n.get("settings_hotkey_current")).size(14);
    let hotkey_value = text(&vm.current_hotkey).size(16);
    let hotkey_row = row![hotkey_label, hotkey_value]
        .spacing(16)
        .align_y(iced::Alignment::Center);

    let action_row: Element<'a, HotkeyMessage> = if vm.is_recording {
        let recording_text = text(i18n.get("settings_hotkey_recording")).size(14);
        let cancel_btn = button(text(i18n.get("settings_cancel")).size(13))
            .on_press(HotkeyMessage::CancelRecording);
        row![recording_text, cancel_btn]
            .spacing(16)
            .align_y(iced::Alignment::Center)
            .into()
    } else {
        button(text(i18n.get("settings_hotkey_record")).size(13))
            .on_press(HotkeyMessage::StartRecording)
            .into()
    };

    column![title, hotkey_row, action_row]
        .spacing(20)
        .padding(24)
        .into()
}
