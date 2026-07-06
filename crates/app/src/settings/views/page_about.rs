// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! About page view.

use iced::widget::{column, text};
use iced::Element;

use crate::i18n::engine::I18nEngine;

use super::super::view_models::page_about::{AboutMessage, AboutViewModel};

/// Render the about page.
pub fn view<'a>(vm: &'a AboutViewModel, i18n: &'a I18nEngine) -> Element<'a, AboutMessage> {
    let title = text(i18n.get("settings_about_title")).size(22);
    let version_label = text(format!(
        "{}: {}",
        i18n.get("settings_about_version"),
        vm.version
    ))
    .size(14);
    let license_label = text(&vm.license).size(12);

    column![title, version_label, license_label]
        .spacing(20)
        .padding(24)
        .into()
}
