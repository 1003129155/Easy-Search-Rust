// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Settings window iced application.

use std::sync::{Arc, RwLock};

use iced::keyboard;
use iced::widget::{button, container, row, text, Column};
use iced::{Element, Length, Size, Subscription, Task, Theme};

use super::view_models::page_about::{AboutMessage, AboutViewModel};
use super::view_models::page_general::{GeneralMessage, GeneralViewModel, Language};
use super::view_models::page_hotkey::{HotkeyMessage, HotkeyViewModel};
use super::view_models::page_plugin::{PluginInfo, PluginMessage, PluginViewModel};
use super::view_models::page_theme::{ThemeMessage, ThemeOption, ThemeViewModel};
use super::views;

use crate::i18n::engine::I18nEngine;
use crate::shared::settings_channel::{self, SettingsChange};
use crate::shared::settings_store::{Settings, SettingsStore};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsPage {
    General,
    Plugin,
    Theme,
    Hotkey,
    About,
}

impl SettingsPage {
    pub fn label_key(&self) -> &'static str {
        match self {
            Self::General => "settings_general",
            Self::Plugin => "settings_plugin",
            Self::Theme => "settings_theme",
            Self::Hotkey => "settings_hotkey",
            Self::About => "settings_about",
        }
    }

    pub const ALL: &'static [SettingsPage] = &[
        SettingsPage::General,
        SettingsPage::Plugin,
        SettingsPage::Theme,
        SettingsPage::Hotkey,
        SettingsPage::About,
    ];
}

#[derive(Debug, Clone)]
pub enum Message {
    NavigateTo(SettingsPage),
    CloseWindow,
    General(GeneralMessage),
    Theme(ThemeMessage),
    Hotkey(HotkeyMessage),
    Plugin(PluginMessage),
    About(AboutMessage),
}

pub struct SettingsApp {
    current_page: SettingsPage,
    general_vm: GeneralViewModel,
    theme_vm: ThemeViewModel,
    hotkey_vm: HotkeyViewModel,
    plugin_vm: PluginViewModel,
    about_vm: AboutViewModel,
    shared_settings: Arc<RwLock<Settings>>,
    i18n: I18nEngine,
}

impl SettingsApp {
    fn new(shared_settings: Arc<RwLock<Settings>>, plugin_infos: Vec<PluginInfo>) -> (Self, Task<Message>) {
        let (general_vm, theme_vm, hotkey_vm, i18n) = {
            let settings = shared_settings.read().unwrap();

            let general_vm = GeneralViewModel {
                selected_language: language_from_code(&settings.language),
                autostart_enabled: settings.autostart,
                index_drives: settings.index_drives.clone(),
            };

            let theme_vm = ThemeViewModel {
                selected_theme: theme_from_name(&settings.theme),
            };

            let hotkey_vm = HotkeyViewModel {
                current_hotkey: settings.hotkey.clone(),
                is_recording: false,
            };

            let i18n = if settings.language.is_empty() {
                I18nEngine::new()
            } else {
                I18nEngine::with_locale(&settings.language)
            };

            (general_vm, theme_vm, hotkey_vm, i18n)
        };

        let mut plugin_vm = PluginViewModel::new();
        plugin_vm.populate_from_plugins(plugin_infos);

        let app = Self {
            current_page: SettingsPage::General,
            general_vm,
            theme_vm,
            hotkey_vm,
            plugin_vm,
            about_vm: AboutViewModel::new(),
            shared_settings,
            i18n,
        };
        (app, Task::none())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NavigateTo(page) => {
                self.current_page = page;
                Task::none()
            }
            Message::CloseWindow => iced::exit(),
            Message::General(msg) => {
                self.general_vm.update(msg.clone());
                if let GeneralMessage::LanguageChanged(lang) = &msg {
                    let locale = language_to_code(lang);
                    self.i18n.set_locale(&locale);
                    self.plugin_vm
                        .populate_from_plugins(crate::build_plugin_infos_for_locale(&locale));
                }
                self.persist_general(&msg);
                Task::none()
            }
            Message::Theme(msg) => {
                self.theme_vm.update(msg.clone());
                self.persist_theme(&msg);
                Task::none()
            }
            Message::Hotkey(msg) => {
                self.hotkey_vm.update(msg.clone());
                self.persist_hotkey(&msg);
                Task::none()
            }
            Message::Plugin(msg) => {
                self.plugin_vm.update(msg);
                Task::none()
            }
            Message::About(msg) => {
                self.about_vm.update(msg);
                Task::none()
            }
        }
    }

    fn persist_general(&self, msg: &GeneralMessage) {
        match msg {
            GeneralMessage::LanguageChanged(lang) => {
                let code = language_to_code(lang);
                if let Ok(mut s) = self.shared_settings.write() {
                    s.language = code.clone();
                }
                self.save_settings();
                self.notify(SettingsChange::LanguageChanged(code));
            }
            GeneralMessage::AutostartToggled(enabled) => {
                if let Ok(mut s) = self.shared_settings.write() {
                    s.autostart = *enabled;
                }
                self.save_settings();
                self.notify(SettingsChange::AutostartChanged(*enabled));

                #[cfg(windows)]
                {
                    if *enabled {
                        let _ = crate::shared::autostart::enable();
                    } else {
                        let _ = crate::shared::autostart::disable();
                    }
                }
            }
            GeneralMessage::DrivesChanged(drives) => {
                let drive_chars: Vec<char> = drives
                    .iter()
                    .filter_map(|s| s.chars().next().map(|c| c.to_ascii_uppercase()))
                    .collect();
                if let Ok(mut s) = self.shared_settings.write() {
                    s.index_drives = drives.clone();
                }
                self.save_settings();
                self.notify(SettingsChange::DrivesChanged(drive_chars));
            }
        }
    }

    fn persist_theme(&self, msg: &ThemeMessage) {
        match msg {
            ThemeMessage::ThemeSelected(theme) => {
                let name = theme_to_name(theme);
                if let Ok(mut s) = self.shared_settings.write() {
                    s.theme = name.clone();
                }
                self.save_settings();
                self.notify(SettingsChange::ThemeChanged(name));
            }
        }
    }

    fn persist_hotkey(&self, msg: &HotkeyMessage) {
        match msg {
            HotkeyMessage::HotkeyRecorded(hotkey) => {
                if let Ok(mut s) = self.shared_settings.write() {
                    s.hotkey = hotkey.clone();
                }
                self.save_settings();
                self.notify(SettingsChange::HotkeyChanged(hotkey.clone()));
            }
            _ => {}
        }
    }

    fn save_settings(&self) {
        let settings_path = easysearch_core::paths::settings_file();

        if let Ok(s) = self.shared_settings.read() {
            let _ = SettingsStore::save(&settings_path, &s);
        }
    }

    fn notify(&self, change: SettingsChange) {
        if let Some(tx) = settings_channel::get_settings_sender() {
            let _ = tx.send(change);
        }
    }

    fn view(&self) -> Element<'_, Message> {
        row![self.nav_panel(), self.content_area()].into()
    }

    fn subscription(&self) -> Subscription<Message> {
        keyboard::on_key_press(|key, _modifiers| match key {
            keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::CloseWindow),
            _ => None,
        })
    }

    fn theme(&self) -> Theme {
        match self.theme_vm.selected_theme {
            ThemeOption::Win11Light => Theme::Light,
            ThemeOption::Win11Dark => Theme::Dark,
            ThemeOption::System => {
                if is_system_dark_mode() {
                    Theme::Dark
                } else {
                    Theme::Light
                }
            }
        }
    }

    fn title(&self) -> String {
        self.i18n.get("settings_window_title").to_string()
    }

    fn nav_panel(&self) -> Element<'_, Message> {
        let nav_items: Vec<Element<'_, Message>> = SettingsPage::ALL
            .iter()
            .map(|&page| self.nav_item(page))
            .collect();

        let nav_column = Column::with_children(nav_items)
            .spacing(4)
            .padding(12)
            .width(Length::Fixed(240.0));

        container(nav_column)
            .height(Length::Fill)
            .style(|_theme: &Theme| container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgb(
                    0.95, 0.95, 0.96,
                ))),
                ..Default::default()
            })
            .into()
    }

    fn nav_item(&self, page: SettingsPage) -> Element<'_, Message> {
        let is_selected = self.current_page == page;
        let label = text(self.i18n.get(page.label_key())).size(14);

        let btn = button(label)
            .on_press(Message::NavigateTo(page))
            .width(Length::Fill)
            .padding([8, 16]);

        if is_selected {
            btn.style(|_theme: &Theme, _status| button::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgb(
                    0.85, 0.88, 0.95,
                ))),
                text_color: iced::Color::from_rgb(0.1, 0.1, 0.3),
                border: iced::Border {
                    radius: 6.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
        } else {
            btn.style(|_theme: &Theme, _status| button::Style {
                background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
                text_color: iced::Color::from_rgb(0.3, 0.3, 0.3),
                border: iced::Border {
                    radius: 6.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
        }
    }

    fn content_area(&self) -> Element<'_, Message> {
        let page_content: Element<'_, Message> = match self.current_page {
            SettingsPage::General => views::page_general::view(&self.general_vm, &self.i18n).map(Message::General),
            SettingsPage::Theme => views::page_theme::view(&self.theme_vm, &self.i18n).map(Message::Theme),
            SettingsPage::Hotkey => views::page_hotkey::view(&self.hotkey_vm, &self.i18n).map(Message::Hotkey),
            SettingsPage::Plugin => views::page_plugin::view(&self.plugin_vm, &self.i18n).map(Message::Plugin),
            SettingsPage::About => views::page_about::view(&self.about_vm, &self.i18n).map(Message::About),
        };

        container(page_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

pub fn run_settings_app(settings: Arc<RwLock<Settings>>, plugin_infos: Vec<PluginInfo>) -> iced::Result {
    iced::application(SettingsApp::title, SettingsApp::update, SettingsApp::view)
        .subscription(SettingsApp::subscription)
        .theme(SettingsApp::theme)
        .window_size(Size::new(940.0, 600.0))
        .default_font(iced::Font {
            family: iced::font::Family::Name("Microsoft YaHei UI"),
            ..iced::Font::DEFAULT
        })
        .run_with(move || SettingsApp::new(settings, plugin_infos))
}

fn language_from_code(code: &str) -> Language {
    match code {
        "zh-CN" | "zh" => Language::Chinese,
        "ja" => Language::Japanese,
        _ => Language::English,
    }
}

fn language_to_code(lang: &Language) -> String {
    match lang {
        Language::English => String::from("en"),
        Language::Chinese => String::from("zh-CN"),
        Language::Japanese => String::from("ja"),
    }
}

fn theme_from_name(name: &str) -> ThemeOption {
    match name {
        "Win11Light" => ThemeOption::Win11Light,
        "Win11Dark" => ThemeOption::Win11Dark,
        _ => ThemeOption::System,
    }
}

fn theme_to_name(theme: &ThemeOption) -> String {
    match theme {
        ThemeOption::Win11Light => String::from("Win11Light"),
        ThemeOption::Win11Dark => String::from("Win11Dark"),
        ThemeOption::System => String::from("System"),
    }
}

fn is_system_dark_mode() -> bool {
    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        let subkey: Vec<u16> = OsStr::new(
            r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
        )
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

        let value_name: Vec<u16> = OsStr::new("AppsUseLightTheme")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();

        unsafe {
            use windows::Win32::System::Registry::{
                HKEY_CURRENT_USER, KEY_READ, REG_DWORD, RegCloseKey, RegOpenKeyExW,
                RegQueryValueExW,
            };

            let mut hkey = Default::default();
            let status = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                windows::core::PCWSTR(subkey.as_ptr()),
                Some(0),
                KEY_READ,
                &mut hkey,
            );
            if status.is_err() {
                return false;
            }

            let mut data: u32 = 1;
            let mut data_size = std::mem::size_of::<u32>() as u32;
            let mut data_type = REG_DWORD;
            let result = RegQueryValueExW(
                hkey,
                windows::core::PCWSTR(value_name.as_ptr()),
                None,
                Some(&mut data_type),
                Some(&mut data as *mut u32 as *mut u8),
                Some(&mut data_size),
            );
            let _ = RegCloseKey(hkey);

            if result.is_ok() {
                return data == 0;
            }
        }
        false
    }

    #[cfg(not(windows))]
    {
        false
    }
}
