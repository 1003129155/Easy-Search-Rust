// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Welcome wizard — Page 3: Feature introduction.

use iced::widget::{column, text, Space};
use iced::{Element, Length};

use crate::welcome::app::Message;
use crate::welcome::state::WelcomeState;

/// Render the features introduction page.
pub fn view(_state: &WelcomeState) -> Element<'_, Message> {
    let title = text("功能亮点").size(24);

    let features = column![
        text("⚡  极速搜索 — 毫秒级 NTFS 文件检索").size(14),
        Space::with_height(8),
        text("🔍  拼音匹配 — 支持拼音首字母搜索中文文件名").size(14),
        Space::with_height(8),
        text("🎨  主题定制 — 多种内置主题，支持自定义配色").size(14),
        Space::with_height(8),
        text("🔌  插件扩展 — 计算器、网页搜索等丰富插件").size(14),
        Space::with_height(8),
        text("🌐  多语言 — 支持中文、英文、日文界面").size(14),
    ]
    .spacing(4);

    column![
        Space::with_height(40),
        title,
        Space::with_height(24),
        features,
        Space::with_height(Length::Fill),
    ]
    .width(Length::Fill)
    .into()
}
