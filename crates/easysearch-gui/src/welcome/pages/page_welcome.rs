// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Welcome wizard — Page 1: Welcome text + language selection.

use iced::widget::{column, pick_list, text, Space};
use iced::{Element, Length};

use crate::welcome::app::Message;
use crate::welcome::state::WelcomeState;

/// Available language options for the pick list.
const LANGUAGES: &[&str] = &["zh-CN", "en", "ja"];

/// Render the welcome page.
pub fn view(state: &WelcomeState) -> Element<'_, Message> {
    let title = text("欢迎使用 EasySearch").size(28);
    let subtitle = text("快速文件搜索工具，为你而设计").size(16);
    let lang_label = text("选择界面语言：").size(14);

    let selected = LANGUAGES
        .iter()
        .find(|&&l| l == state.language)
        .copied();

    let lang_picker = pick_list(
        LANGUAGES.to_vec(),
        selected,
        |s: &str| Message::SelectLanguage(s.to_string()),
    )
    .placeholder("选择语言");

    column![
        Space::with_height(40),
        title,
        Space::with_height(12),
        subtitle,
        Space::with_height(40),
        lang_label,
        Space::with_height(8),
        lang_picker,
        Space::with_height(Length::Fill),
    ]
    .width(Length::Fill)
    .into()
}
