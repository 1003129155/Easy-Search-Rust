// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Welcome wizard iced application.

use iced::widget::{button, column, container, row, text, Space};
use iced::{Element, Length, Size, Task, Theme};

use super::pages;
use super::state::{WelcomePage, WelcomeState};
use crate::i18n::engine::I18nEngine;

#[derive(Debug, Clone)]
pub enum Message {
    Next,
    Back,
    Finish,
    SelectLanguage(String),
    SelectTheme(String),
    ToggleAutostart(bool),
}

pub struct WelcomeApp {
    state: WelcomeState,
    i18n: I18nEngine,
}

impl WelcomeApp {
    fn new() -> (Self, Task<Message>) {
        let state = WelcomeState::default();
        let app = Self {
            i18n: I18nEngine::with_locale(&state.language),
            state,
        };
        (app, Task::none())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Next => {
                if let Some(next) = self.state.current_page.next() {
                    self.state.current_page = next;
                }
                Task::none()
            }
            Message::Back => {
                if let Some(prev) = self.state.current_page.prev() {
                    self.state.current_page = prev;
                }
                Task::none()
            }
            Message::Finish => iced::exit(),
            Message::SelectLanguage(lang) => {
                self.i18n.set_locale(&lang);
                self.state.language = lang;
                Task::none()
            }
            Message::SelectTheme(theme) => {
                self.state.theme = theme;
                Task::none()
            }
            Message::ToggleAutostart(enabled) => {
                self.state.autostart = enabled;
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let page_content: Element<'_, Message> = match self.state.current_page {
            WelcomePage::Welcome => pages::page_welcome::view(&self.state, &self.i18n),
            WelcomePage::Hotkey => pages::page_hotkey::view(&self.state, &self.i18n),
            WelcomePage::Features => pages::page_features::view(&self.state, &self.i18n),
            WelcomePage::Theme => pages::page_theme::view(&self.state, &self.i18n),
            WelcomePage::Finish => pages::page_finish::view(&self.state, &self.i18n),
        };

        let nav_bar = self.navigation_bar();
        let content = column![page_content, nav_bar]
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fill);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(24)
            .into()
    }

    fn theme(&self) -> Theme {
        Theme::Light
    }

    fn title(&self) -> String {
        self.i18n.get("welcome_window_title").to_string()
    }

    fn navigation_bar(&self) -> Element<'_, Message> {
        let page = self.state.current_page;
        let page_indicator = text(format!("{} / {}", page.index() + 1, WelcomePage::COUNT)).size(13);

        let back_btn: Element<'_, Message> = if page.prev().is_some() {
            button(text(self.i18n.get("welcome_back")).size(14))
                .on_press(Message::Back)
                .padding([8, 20])
                .into()
        } else {
            Space::with_width(80).into()
        };

        let next_btn: Element<'_, Message> = if page == WelcomePage::Finish {
            button(text(self.i18n.get("welcome_finish")).size(14))
                .on_press(Message::Finish)
                .padding([8, 20])
                .into()
        } else {
            button(text(self.i18n.get("welcome_next")).size(14))
                .on_press(Message::Next)
                .padding([8, 20])
                .into()
        };

        row![
            back_btn,
            Space::with_width(Length::Fill),
            page_indicator,
            Space::with_width(Length::Fill),
            next_btn,
        ]
        .align_y(iced::Alignment::Center)
        .width(Length::Fill)
        .padding(iced::Padding {
            top: 16.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        })
        .into()
    }
}

#[allow(dead_code)]
pub struct WelcomeChoices {
    pub language: String,
    pub theme: String,
    pub autostart: bool,
}

pub fn run_welcome_app() -> iced::Result {
    iced::application(WelcomeApp::title, WelcomeApp::update, WelcomeApp::view)
        .theme(WelcomeApp::theme)
        .window_size(Size::new(700.0, 500.0))
        .resizable(false)
        .default_font(iced::Font {
            family: iced::font::Family::Name("Microsoft YaHei UI"),
            ..iced::Font::DEFAULT
        })
        .run_with(WelcomeApp::new)
}
