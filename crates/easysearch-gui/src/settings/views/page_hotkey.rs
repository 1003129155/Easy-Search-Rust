// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Hotkey settings page view — current hotkey display + record button.
//!
//! # Requirements
//! - Req 3.5: Each Setting_Page has independent View + ViewModel

use iced::widget::{button, column, row, text};
use iced::Element;

use super::super::view_models::page_hotkey::{HotkeyMessage, HotkeyViewModel};

/// Render the hotkey settings page.
pub fn view<'a>(vm: &'a HotkeyViewModel) -> Element<'a, HotkeyMessage> {
    let title = text("快捷键设置").size(22);

    // Current hotkey display
    let hotkey_label = text("唤起热键").size(14);
    let hotkey_value = text(&vm.current_hotkey).size(16);
    let hotkey_row = row![hotkey_label, hotkey_value].spacing(16).align_y(iced::Alignment::Center);

    // Record button or recording state
    let action_row: Element<'a, HotkeyMessage> = if vm.is_recording {
        let recording_text = text("请按下新的快捷键组合...").size(14);
        let cancel_btn = button(text("取消").size(13))
            .on_press(HotkeyMessage::CancelRecording);
        row![recording_text, cancel_btn].spacing(16).align_y(iced::Alignment::Center).into()
    } else {
        let record_btn = button(text("录制新快捷键").size(13))
            .on_press(HotkeyMessage::StartRecording);
        record_btn.into()
    };

    column![title, hotkey_row, action_row]
        .spacing(20)
        .padding(24)
        .into()
}
