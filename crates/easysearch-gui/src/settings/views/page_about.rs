// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! About page view — version info + license text.
//!
//! # Requirements
//! - Req 3.5: Each Setting_Page has independent View + ViewModel

use iced::widget::{column, text};
use iced::Element;

use super::super::view_models::page_about::{AboutMessage, AboutViewModel};

/// Render the about page.
pub fn view<'a>(vm: &'a AboutViewModel) -> Element<'a, AboutMessage> {
    let title = text("关于").size(22);

    let version_label = text(format!("版本: {}", vm.version)).size(14);
    let license_label = text(&vm.license).size(12);

    column![title, version_label, license_label]
        .spacing(20)
        .padding(24)
        .into()
}
