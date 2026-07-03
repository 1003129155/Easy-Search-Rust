// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Hotkey settings page view model — hotkey display and recording.
//!
//! # Requirements
//! - Req 3.5: Each Setting_Page has independent View + ViewModel
//! - Req 4.4: hotkey binding setting

/// State for the hotkey settings page.
#[derive(Debug, Clone)]
pub struct HotkeyViewModel {
    /// Current hotkey binding display string.
    pub current_hotkey: String,
    /// Whether we are currently recording a new hotkey.
    pub is_recording: bool,
}

/// Messages for the hotkey settings page.
#[derive(Debug, Clone)]
pub enum HotkeyMessage {
    /// User clicked the "record new hotkey" button.
    StartRecording,
    /// User cancelled recording.
    CancelRecording,
    /// A new hotkey was recorded.
    HotkeyRecorded(String),
}

impl HotkeyViewModel {
    /// Create a new HotkeyViewModel with default hotkey.
    pub fn new() -> Self {
        Self {
            current_hotkey: "Alt+Space".to_string(),
            is_recording: false,
        }
    }

    /// Handle an incoming message.
    pub fn update(&mut self, msg: HotkeyMessage) {
        match msg {
            HotkeyMessage::StartRecording => {
                self.is_recording = true;
            }
            HotkeyMessage::CancelRecording => {
                self.is_recording = false;
            }
            HotkeyMessage::HotkeyRecorded(hotkey) => {
                self.current_hotkey = hotkey;
                self.is_recording = false;
            }
        }
    }
}
