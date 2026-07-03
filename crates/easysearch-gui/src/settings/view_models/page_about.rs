// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! About page view model — version info, license text.
//!
//! # Requirements
//! - Req 3.5: Each Setting_Page has independent View + ViewModel

/// State for the about page.
#[derive(Debug, Clone)]
pub struct AboutViewModel {
    /// Application version string.
    pub version: String,
    /// License text.
    pub license: String,
}

/// Messages for the about page (currently none needed).
#[derive(Debug, Clone)]
pub enum AboutMessage {
    /// Placeholder — no interactive actions yet.
    _Noop,
}

impl AboutViewModel {
    /// Create a new AboutViewModel with version and license info.
    pub fn new() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            license: "MIT License\n\nCopyright (c) 2025-2026 LIJIALU".to_string(),
        }
    }

    /// Handle an incoming message.
    pub fn update(&mut self, msg: AboutMessage) {
        match msg {
            AboutMessage::_Noop => {}
        }
    }
}
