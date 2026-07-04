// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Welcome wizard — Page 5: Autostart toggle + finish.

use iced::widget::{checkbox, column, text, Space};
use iced::{Element, Length};

use crate::welcome::app::Message;
use crate::welcome::state::WelcomeState;

/// Render the finish page.
pub fn view(state: &WelcomeState) -> Element<'_, Message> {
    let title = text("准备就绪！").size(24);
    let desc = text("EasySearch 已经准备好为你服务。").size(14);

    let autostart_toggle = checkbox("开机时自动启动 EasySearch", state.autostart)
        .on_toggle(Message::ToggleAutostart)
        .size(18);

    let hint = text("点击「完成」开始使用 EasySearch。").size(12);

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
