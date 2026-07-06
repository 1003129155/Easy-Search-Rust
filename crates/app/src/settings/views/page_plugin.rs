// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Plugin management page view.

use easysearch_core::{SettingControl, SettingItem};
use iced::widget::{button, column, container, image, pick_list, row, text, toggler, Column};
use iced::{Element, Length, Theme};

use crate::i18n::engine::I18nEngine;
use crate::shared::icon_assets;

use super::super::view_models::page_plugin::{PluginMessage, PluginViewModel};

/// Render the plugin management page.
pub fn view<'a>(vm: &'a PluginViewModel, i18n: &'a I18nEngine) -> Element<'a, PluginMessage> {
    let title = text(i18n.get("settings_plugin_title")).size(22);
    let subtitle = text(i18n.get("settings_plugin_subtitle")).size(12);

    let plugin_items: Vec<Element<'a, PluginMessage>> = vm
        .plugins
        .iter()
        .enumerate()
        .map(|(index, plugin)| render_plugin_entry(index, plugin, i18n))
        .collect();

    let plugin_list = Column::with_children(plugin_items).spacing(4);

    column![title, subtitle, plugin_list]
        .spacing(16)
        .padding(24)
        .into()
}

fn render_plugin_entry<'a>(
    index: usize,
    plugin: &'a super::super::view_models::page_plugin::PluginEntry,
    i18n: &'a I18nEngine,
) -> Element<'a, PluginMessage> {
    let expand_icon = if plugin.expanded { "v" } else { ">" };

    let expand_btn = button(text(expand_icon).size(12))
        .on_press(PluginMessage::ToggleExpanded { index })
        .padding([4, 8])
        .style(|_theme: &Theme, _status| button::Style {
            background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
            text_color: iced::Color::from_rgb(0.3, 0.3, 0.3),
            ..Default::default()
        });

    let name_btn = button(text(&plugin.name).size(14))
        .on_press(PluginMessage::ToggleExpanded { index })
        .padding([4, 8])
        .style(|_theme: &Theme, _status| button::Style {
            background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
            text_color: iced::Color::from_rgb(0.1, 0.1, 0.1),
            ..Default::default()
        });

    let icon_view: Element<'a, PluginMessage> =
        if let Some(bytes) = icon_assets::named_icon_bytes(&plugin.icon) {
            image(image::Handle::from_bytes(bytes.to_vec()))
                .width(Length::Fixed(18.0))
                .height(Length::Fixed(18.0))
                .into()
        } else {
            container(text(""))
                .width(Length::Fixed(18.0))
                .height(Length::Fixed(18.0))
                .into()
        };

    let keyword_text = match &plugin.keyword {
        Some(kw) => format!("{}: {}", i18n.get("settings_plugin_keyword"), kw.trim()),
        None => i18n.get("settings_plugin_auto").to_string(),
    };
    let keyword_label = text(keyword_text).size(11);

    let toggle = toggler(plugin.enabled)
        .on_toggle(move |enabled| PluginMessage::TogglePlugin { index, enabled });

    let header = row![expand_btn, icon_view, name_btn, keyword_label, toggle]
        .spacing(12)
        .align_y(iced::Alignment::Center);

    let mut entry_items: Vec<Element<'a, PluginMessage>> = vec![header.into()];

    if plugin.expanded {
        if !plugin.description.is_empty() {
            let desc = text(&plugin.description).size(12);
            let desc_container = container(desc).padding(8);
            entry_items.push(desc_container.into());
        }

        if let Some(schema) = &plugin.settings_schema {
            let settings_items: Vec<Element<'a, PluginMessage>> = schema
                .iter()
                .map(|item| render_setting_item(index, item, &plugin.setting_values, i18n))
                .collect();

            let settings_panel = Column::with_children(settings_items)
                .spacing(10)
                .padding(16);

            let bordered_panel = container(settings_panel)
                .style(|_theme: &Theme| container::Style {
                    background: Some(iced::Background::Color(iced::Color::from_rgb(
                        0.96, 0.97, 0.98,
                    ))),
                    border: iced::Border {
                        radius: 6.0.into(),
                        width: 1.0,
                        color: iced::Color::from_rgb(0.88, 0.88, 0.90),
                    },
                    ..Default::default()
                })
                .width(Length::Fill);

            entry_items.push(bordered_panel.into());
        }
    }

    let entry_column = Column::with_children(entry_items).spacing(4);

    container(entry_column)
        .width(Length::Fill)
        .padding([8, 0])
        .style(|_theme: &Theme| container::Style {
            border: iced::Border {
                width: 0.0,
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn render_setting_item<'a>(
    plugin_index: usize,
    item: &'a SettingItem,
    current_values: &'a [(String, String)],
    i18n: &'a I18nEngine,
) -> Element<'a, PluginMessage> {
    let current_value = current_values
        .iter()
        .find(|(k, _)| k == &item.key)
        .map(|(_, v)| v.as_str())
        .unwrap_or("");

    let label = text(&item.label).size(13);

    let control: Element<'a, PluginMessage> = match &item.control {
        SettingControl::Toggle { default } => {
            let is_on = if current_value.is_empty() {
                *default
            } else {
                current_value == "true"
            };
            let key = item.key.clone();
            toggler(is_on)
                .on_toggle(move |v| PluginMessage::SettingChanged {
                    plugin_index,
                    key: key.clone(),
                    value: v.to_string(),
                })
                .into()
        }
        SettingControl::Dropdown { options, default } => {
            let selected_value = if current_value.is_empty() {
                default.trim_matches('"').to_string()
            } else {
                current_value.trim_matches('"').to_string()
            };

            let display_options: Vec<String> = options.iter().map(|(_, label)| label.clone()).collect();
            let selected_label = options
                .iter()
                .find(|(v, _)| *v == selected_value)
                .map(|(_, l)| l.clone());

            let key = item.key.clone();
            let opts = options.clone();
            pick_list(display_options, selected_label, move |chosen_label: String| {
                let val = opts
                    .iter()
                    .find(|(_, l)| *l == chosen_label)
                    .map(|(v, _)| format!("\"{}\"", v))
                    .unwrap_or_default();
                PluginMessage::SettingChanged {
                    plugin_index,
                    key: key.clone(),
                    value: val,
                }
            })
            .into()
        }
        SettingControl::Number { default, .. } => {
            text(current_value.parse::<i64>().unwrap_or(*default).to_string())
                .size(13)
                .into()
        }
        SettingControl::TextInput { .. } => {
            let display_value = if current_value.is_empty() {
                i18n.get("settings_plugin_unset")
            } else {
                current_value
            };
            text(display_value).size(13).into()
        }
    };

    let desc: Element<'a, PluginMessage> = if item.description.is_empty() {
        text("").size(12).into()
    } else {
        text(&item.description).size(11).into()
    };

    let label_col = column![label, desc].spacing(2);

    row![label_col, control]
        .spacing(16)
        .align_y(iced::Alignment::Center)
        .into()
}
