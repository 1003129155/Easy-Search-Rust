// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Welcome wizard — iced Application implementation.
//!
//! A multi-page wizard with forward/back navigation.
//! Window size: 700×500, not resizable (wizard style).

use iced::widget::{button, column, container, row, text, Space};
use iced::{Element, Length, Size, Task, Theme};

use super::pages;
use super::state::{WelcomePage, WelcomeState};

// ─── Messages ────────────────────────────────────────────────────────────────

/// Messages handled by the welcome wizard.
#[derive(Debug, Clone)]
pub enum Message {
    /// Go to the next page.
    Next,
    /// Go to the previous page.
    Back,
    /// Finish the wizard (close window).
    Finish,
    /// Select a language (from page 1).
    SelectLanguage(String),
    /// Select a theme (from page 4).
    SelectTheme(String),
    /// Toggle autostart (from page 5).
    ToggleAutostart(bool),
}

// ─── Application State ───────────────────────────────────────────────────────

/// Main state for the welcome wizard window.
pub struct WelcomeApp {
    state: WelcomeState,
}

impl WelcomeApp {
    /// Create a new WelcomeApp at the first page.
    fn new() -> (Self, Task<Message>) {
        let app = Self {
            state: WelcomeState::default(),
        };
        (app, Task::none())
    }

    /// Handle incoming messages.
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

    /// Render the wizard UI.
    fn view(&self) -> Element<'_, Message> {
        let page_content: Element<'_, Message> = match self.state.current_page {
            WelcomePage::Welcome => pages::page_welcome::view(&self.state),
            WelcomePage::Hotkey => pages::page_hotkey::view(&self.state),
            WelcomePage::Features => pages::page_features::view(&self.state),
            WelcomePage::Theme => pages::page_theme::view(&self.state),
            WelcomePage::Finish => pages::page_finish::view(&self.state),
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

    /// Returns the iced Theme.
    fn theme(&self) -> Theme {
        Theme::Light
    }

    // ─── UI Components ───────────────────────────────────────────────────────

    /// Bottom navigation bar with Back/Next/Finish buttons.
    fn navigation_bar(&self) -> Element<'_, Message> {
        let page = self.state.current_page;
        let page_indicator = text(format!(
            "{} / {}",
            page.index() + 1,
            WelcomePage::COUNT
        ))
        .size(13);

        let back_btn: Element<'_, Message> = if page.prev().is_some() {
            button(text("上一步").size(14))
                .on_press(Message::Back)
                .padding([8, 20])
                .into()
        } else {
            // Invisible placeholder to maintain layout
            Space::with_width(80).into()
        };

        let next_btn: Element<'_, Message> = if page == WelcomePage::Finish {
            button(text("完成").size(14))
                .on_press(Message::Finish)
                .padding([8, 20])
                .into()
        } else {
            button(text("下一步").size(14))
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
        .padding(iced::Padding { top: 16.0, right: 0.0, bottom: 0.0, left: 0.0 })
        .into()
    }
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Get the user's choices after the wizard completes.
///
/// Returns (language, theme, autostart).
pub struct WelcomeChoices {
    pub language: String,
    pub theme: String,
    pub autostart: bool,
}

/// Run the welcome wizard. Blocks until the wizard is closed.
///
/// Returns `Ok(())` on success (user choices are applied via settings).
pub fn run_welcome_app() -> iced::Result {
    iced::application("EasySearch 欢迎", WelcomeApp::update, WelcomeApp::view)
        .theme(WelcomeApp::theme)
        .window_size(Size::new(700.0, 500.0))
        .resizable(false)
        .default_font(iced::Font {
            family: iced::font::Family::Name("Microsoft YaHei UI"),
            ..iced::Font::DEFAULT
        })
        .run_with(WelcomeApp::new)
}
