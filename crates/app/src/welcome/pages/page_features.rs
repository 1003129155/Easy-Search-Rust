// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Welcome wizard page 3.

use iced::widget::{column, text, Space};
use iced::{Element, Length};

use crate::i18n::engine::I18nEngine;
use crate::welcome::app::Message;
use crate::welcome::state::WelcomeState;

pub fn view<'a>(_state: &'a WelcomeState, i18n: &'a I18nEngine) -> Element<'a, Message> {
    let title = text(i18n.get("welcome_features_title")).size(24);

    let features = column![
        text(i18n.get("welcome_feature_fast")).size(14),
        Space::with_height(8),
        text(i18n.get("welcome_feature_pinyin")).size(14),
        Space::with_height(8),
        text(i18n.get("welcome_feature_theme")).size(14),
        Space::with_height(8),
        text(i18n.get("welcome_feature_plugins")).size(14),
        Space::with_height(8),
        text(i18n.get("welcome_feature_languages")).size(14),
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
