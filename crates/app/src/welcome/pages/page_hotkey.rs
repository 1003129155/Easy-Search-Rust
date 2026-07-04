// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Welcome wizard — Page 2: Display activation hotkey.

use iced::widget::{column, text, Space};
use iced::{Element, Length};

use crate::welcome::app::Message;
use crate::welcome::state::WelcomeState;

/// Render the hotkey page.
pub fn view(_state: &WelcomeState) -> Element<'_, Message> {
    let title = text("唤起热键").size(24);
    let desc = text("使用以下热键随时唤起 EasySearch 搜索窗口：").size(14);
    let hotkey_display = text("Alt + Space").size(32);
    let note = text("你可以稍后在设置中修改热键绑定。").size(12);

    column![
        Space::with_height(40),
        title,
        Space::with_height(16),
        desc,
        Space::with_height(40),
        hotkey_display,
        Space::with_height(24),
        note,
        Space::with_height(Length::Fill),
    ]
    .width(Length::Fill)
    .into()
}
