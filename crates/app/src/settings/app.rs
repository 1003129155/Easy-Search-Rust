// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Settings window — iced application framework.
//!
//! Implements the main settings window with left navigation panel (240px)
//! and right content area. Supports page navigation, Escape to close,
//! and minimum window size of 940×600.
//!
//! Settings changes are persisted to disk and broadcast to the search window
//! via the global settings channel.
//!
//! # Requirements
//! - Req 3.1: Left Navigation_Panel (fixed 240px) + right content area
//! - Req 3.2: Nav items: 通用设置、插件管理、外观主题、快捷键设置、关于页面
//! - Req 3.3: Click nav item → switch right content
//! - Req 3.4: Default to "通用设置" page
//! - Req 3.5: MVVM — each page has independent View + ViewModel
//! - Req 3.6: Min window size 940×600, resizable
//! - Req 3.7: Escape key closes window

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

use crate::shared::settings_channel::{self, SettingsChange};
use crate::shared::settings_store::{Settings, SettingsStore};

// ─── Navigation Pages ────────────────────────────────────────────────────────

/// All navigable pages in the settings window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsPage {
    /// 通用设置
    General,
    /// 插件管理
    Plugin,
    /// 外观主题
    Theme,
    /// 快捷键设置
    Hotkey,
    /// 关于页面
    About,
}

impl SettingsPage {
    /// Display label for the navigation item.
    pub fn label(&self) -> &'static str {
        match self {
            Self::General => "通用设置",
            Self::Plugin => "插件管理",
            Self::Theme => "外观主题",
            Self::Hotkey => "快捷键设置",
            Self::About => "关于",
        }
    }

    /// All pages in navigation order.
    pub const ALL: &'static [SettingsPage] = &[
        SettingsPage::General,
        SettingsPage::Plugin,
        SettingsPage::Theme,
        SettingsPage::Hotkey,
        SettingsPage::About,
    ];
}

// ─── Messages ────────────────────────────────────────────────────────────────

/// Messages handled by the settings application.
#[derive(Debug, Clone)]
pub enum Message {
    /// Navigate to a specific settings page.
    NavigateTo(SettingsPage),
    /// Close the settings window (triggered by Escape key).
    CloseWindow,
    /// Message from the general settings page.
    General(GeneralMessage),
    /// Message from the theme settings page.
    Theme(ThemeMessage),
    /// Message from the hotkey settings page.
    Hotkey(HotkeyMessage),
    /// Message from the plugin management page.
    Plugin(PluginMessage),
    /// Message from the about page.
    About(AboutMessage),
}

// ─── Application State ───────────────────────────────────────────────────────

/// Main state for the settings window.
pub struct SettingsApp {
    /// Currently selected navigation page.
    current_page: SettingsPage,
    /// General settings page view model.
    general_vm: GeneralViewModel,
    /// Theme settings page view model.
    theme_vm: ThemeViewModel,
    /// Hotkey settings page view model.
    hotkey_vm: HotkeyViewModel,
    /// Plugin management page view model.
    plugin_vm: PluginViewModel,
    /// About page view model.
    about_vm: AboutViewModel,
    /// Shared settings for persistence.
    shared_settings: Arc<RwLock<Settings>>,
}

impl SettingsApp {
    /// Create a new SettingsApp, loading initial values from shared settings.
    fn new(shared_settings: Arc<RwLock<Settings>>, plugin_infos: Vec<PluginInfo>) -> (Self, Task<Message>) {
        // Read current settings to initialize ViewModels
        let (general_vm, theme_vm, hotkey_vm) = {
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

            (general_vm, theme_vm, hotkey_vm)
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
        };
        (app, Task::none())
    }

    /// Handle incoming messages.
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NavigateTo(page) => {
                self.current_page = page;
                Task::none()
            }
            Message::CloseWindow => iced::exit(),
            Message::General(msg) => {
                self.general_vm.update(msg.clone());
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

    /// Persist general settings changes and notify search window.
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

                // Actually toggle autostart in registry
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

    /// Persist theme changes and notify search window.
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

    /// Persist hotkey changes and notify search window.
    fn persist_hotkey(&self, msg: &HotkeyMessage) {
        match msg {
            HotkeyMessage::HotkeyRecorded(hotkey) => {
                if let Ok(mut s) = self.shared_settings.write() {
                    s.hotkey = hotkey.clone();
                }
                self.save_settings();
                self.notify(SettingsChange::HotkeyChanged(hotkey.clone()));
            }
            _ => {} // StartRecording / CancelRecording don't need persistence
        }
    }

    /// Save current settings to disk.
    fn save_settings(&self) {
        let settings_path = dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("EasySearch")
            .join("settings.json");

        if let Ok(s) = self.shared_settings.read() {
            let _ = SettingsStore::save(&settings_path, &s);
        }
    }

    /// Send a change notification to the search window (best-effort).
    fn notify(&self, change: SettingsChange) {
        if let Some(tx) = settings_channel::get_settings_sender() {
            let _ = tx.send(change);
        }
    }

    /// Render the settings window UI.
    fn view(&self) -> Element<'_, Message> {
        let nav_panel = self.nav_panel();
        let content = self.content_area();

        row![nav_panel, content].into()
    }

    /// Keyboard subscription: Escape closes the window.
    fn subscription(&self) -> Subscription<Message> {
        keyboard::on_key_press(|key, _modifiers| match key {
            keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::CloseWindow),
            _ => None,
        })
    }

    /// Returns the iced Theme based on user's selection.
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

    // ─── UI Components ───────────────────────────────────────────────────────

    /// Left navigation panel (fixed 240px width).
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

    /// Single navigation item button.
    fn nav_item(&self, page: SettingsPage) -> Element<'_, Message> {
        let is_selected = self.current_page == page;
        let label = text(page.label()).size(14);

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

    /// Right content area — dispatches to the correct page view.
    fn content_area(&self) -> Element<'_, Message> {
        let page_content: Element<'_, Message> = match self.current_page {
            SettingsPage::General => {
                views::page_general::view(&self.general_vm).map(Message::General)
            }
            SettingsPage::Theme => {
                views::page_theme::view(&self.theme_vm).map(Message::Theme)
            }
            SettingsPage::Hotkey => {
                views::page_hotkey::view(&self.hotkey_vm).map(Message::Hotkey)
            }
            SettingsPage::Plugin => {
                views::page_plugin::view(&self.plugin_vm).map(Message::Plugin)
            }
            SettingsPage::About => {
                views::page_about::view(&self.about_vm).map(Message::About)
            }
        };

        container(page_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

// ─── Entry Point ─────────────────────────────────────────────────────────────

/// Launch the settings window as an iced application.
///
/// This function blocks until the window is closed.
pub fn run_settings_app(settings: Arc<RwLock<Settings>>, plugin_infos: Vec<PluginInfo>) -> iced::Result {
    iced::application("EasySearch 设置", SettingsApp::update, SettingsApp::view)
        .subscription(SettingsApp::subscription)
        .theme(SettingsApp::theme)
        .window_size(Size::new(940.0, 600.0))
        .default_font(iced::Font {
            family: iced::font::Family::Name("Microsoft YaHei UI"),
            ..iced::Font::DEFAULT
        })
        .run_with(move || SettingsApp::new(settings, plugin_infos))
}

// ─── Conversion Helpers ──────────────────────────────────────────────────────

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

/// Detect Windows system dark mode via registry.
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
                return data == 0; // 0 = dark mode, 1 = light mode
            }
        }
        false
    }

    #[cfg(not(windows))]
    {
        false
    }
}
