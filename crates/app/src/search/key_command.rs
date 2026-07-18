// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Key decoding and command dispatch.
//!
//! Separates the "what key was pressed" concern from the "what happens next"
//! concern.  `decode_key_command` translates raw virtual-key codes + modifiers
//! into a `KeyCommand`; `execute_key_command` performs the immediate state
//! mutations and returns a `DeferredAction` for any Win32 calls that must
//! happen outside the `AppState` borrow.

#[cfg(windows)]
use windows::Win32::Foundation::WPARAM;
#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_BACK, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_LEFT,
    VK_MENU, VK_RETURN, VK_RIGHT, VK_SHIFT, VK_UP,
};

#[cfg(windows)]
use super::app_state::{AppState, ViewMode};

/// High-level actions that may result from a keyboard event.
///
/// Some of these are "immediate" (state-only), others are "deferred" (require
/// Win32 calls that must happen outside the AppState borrow).
#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum KeyCommand {
    /// Move selection up (wrap to bottom).
    SelectUp,
    /// Move selection down (wrap to top).
    SelectDown,
    /// Execute the currently selected item.
    Execute,
    /// Select and execute a result by its zero-based Alt+number index.
    ExecuteIndex(usize),
    /// Open the containing folder (Ctrl+Enter).
    OpenFolder,
    /// Open context actions for the selected item (Shift+Enter, Ctrl+O, Right).
    OpenContext,
    /// Close context actions and return to results (Escape, Left).
    CloseContext,
    /// Show the native Windows context menu (Alt+Enter).
    ShowNativeContextMenu,
    /// Hide the search window.
    Hide,
    /// Backspace: delete character before cursor.
    Backspace,
    /// Delete: delete character after cursor.
    Delete,
    /// Move cursor to the start of the input.
    Home,
    /// Move cursor to the end of the input.
    End,
    /// Move cursor left one character.
    CursorLeft,
    /// Move cursor right one character.
    CursorRight,
    /// Select all text (Ctrl+A).
    SelectAll,
    /// Paste from clipboard (Ctrl+V).
    Paste,
    /// Copy selection to clipboard (Ctrl+C).
    Copy,
    /// Cut selection to clipboard (Ctrl+X).
    Cut,
    /// Autocomplete the input with the selected item's title (Tab).
    Autocomplete,
    /// Fill search box with the selected plugin's keyword (Enter on hint).
    FillHint,
    /// No meaningful action.
    None,
}

/// Deferred actions that require Win32 calls (must be done AFTER releasing borrow).
#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DeferredAction {
    Hide,
    Execute,
    OpenFolder,
    OpenContext,
    CloseContext,
    ShowNativeContextMenu,
    None,
}

/// Decode a raw WM_KEYDOWN event into a [`KeyCommand`].
///
/// This function reads the virtual-key code and modifier state, but does NOT
/// access application state.  The caller supplies `view_mode`, `is_hint`,
/// and `input_focused` so that the decoder can make context-sensitive
/// decisions without holding a borrow.
#[cfg(windows)]
pub(super) fn decode_key_command(
    wparam: WPARAM,
    view_mode: ViewMode,
    is_hint: bool,
    input_empty: bool,
    input_focused: bool,
) -> KeyCommand {
    let vk = wparam.0 as u16;
    let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;
    let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
    let alt = unsafe { GetKeyState(VK_MENU.0 as i32) } < 0;

    match vk {
        v if v == VK_ESCAPE.0 => {
            if view_mode == ViewMode::ContextActions {
                KeyCommand::CloseContext
            } else {
                KeyCommand::Hide
            }
        }
        v if alt
            && !ctrl
            && !shift
            && (0x31..=0x39).contains(&v)
            && view_mode == ViewMode::Results =>
        {
            KeyCommand::ExecuteIndex((v - 0x31) as usize)
        }
        v if v == VK_RETURN.0 && is_hint && input_empty && view_mode == ViewMode::Results => {
            // Enter on a home-screen hint item: fill keyword into input
            KeyCommand::FillHint
        }
        v if v == VK_RETURN.0 => {
            if alt {
                KeyCommand::ShowNativeContextMenu
            } else if shift {
                KeyCommand::OpenContext
            } else if ctrl {
                if view_mode == ViewMode::Results {
                    KeyCommand::OpenFolder
                } else {
                    KeyCommand::Execute
                }
            } else {
                KeyCommand::Execute
            }
        }
        v if ctrl && v == 0x4F => KeyCommand::OpenContext,
        v if v == VK_UP.0 => {
            if input_focused {
                // When input is focused, Up does nothing.
                KeyCommand::None
            } else {
                KeyCommand::SelectUp
            }
        }
        v if v == VK_DOWN.0 => KeyCommand::SelectDown,
        v if v == VK_BACK.0 => KeyCommand::Backspace,
        v if v == VK_DELETE.0 => KeyCommand::Delete,
        v if v == VK_HOME.0 => KeyCommand::Home,
        v if v == VK_END.0 => KeyCommand::End,
        v if v == VK_LEFT.0 => {
            if view_mode == ViewMode::ContextActions {
                KeyCommand::CloseContext
            } else {
                KeyCommand::CursorLeft
            }
        }
        v if v == VK_RIGHT.0 => {
            if input_focused {
                // When input is focused, Right moves cursor.
                KeyCommand::CursorRight
            } else if view_mode == ViewMode::Results {
                KeyCommand::OpenContext
            } else {
                KeyCommand::CursorRight
            }
        }
        v if ctrl && v == 0x41 => KeyCommand::SelectAll,
        v if ctrl && v == 0x56 => KeyCommand::Paste,
        v if ctrl && v == 0x43 => KeyCommand::Copy,
        v if ctrl && v == 0x58 => KeyCommand::Cut,
        v if v == 0x09 => KeyCommand::Autocomplete,
        _ => KeyCommand::None,
    }
}

/// Execute a [`KeyCommand`] against the application state.
///
/// Returns a [`DeferredAction`] that the caller must process *after* the
/// borrow is released.
#[cfg(windows)]
pub(super) fn execute_key_command(app: &mut AppState, cmd: KeyCommand) -> DeferredAction {
    let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;

    match cmd {
        KeyCommand::SelectUp => {
            if app.items.is_empty() {
                // no-op
            } else if app.selected_index > 0 {
                app.selected_index -= 1;
            } else {
                // At the first result, pressing Up returns focus to input box
                // instead of wrapping to the last result.
                app.input_focused = true;
            }
            match app.view_mode {
                ViewMode::Results => {
                    app.result_selected_index = app.selected_index;
                }
                ViewMode::ContextActions => {
                    app.context_selected_index = app.selected_index;
                }
            }
            DeferredAction::None
        }
        KeyCommand::SelectDown => {
            if app.input_focused {
                // Transfer focus from input box to result list
                app.input_focused = false;
                app.selected_index = 0;
                match app.view_mode {
                    ViewMode::Results => {
                        app.result_selected_index = 0;
                    }
                    ViewMode::ContextActions => {
                        app.context_selected_index = 0;
                    }
                }
            } else if !app.items.is_empty() {
                if app.selected_index < app.items.len() - 1 {
                    app.selected_index += 1;
                } else {
                    app.selected_index = 0;
                }
                match app.view_mode {
                    ViewMode::Results => {
                        app.result_selected_index = app.selected_index;
                    }
                    ViewMode::ContextActions => {
                        app.context_selected_index = app.selected_index;
                    }
                }
            }
            DeferredAction::None
        }
        KeyCommand::Execute => DeferredAction::Execute,
        KeyCommand::ExecuteIndex(index) => {
            if index >= app.items.len() {
                return DeferredAction::None;
            }
            app.selected_index = index;
            app.result_selected_index = index;
            app.input_focused = false;
            DeferredAction::Execute
        }
        KeyCommand::OpenFolder => DeferredAction::OpenFolder,
        KeyCommand::OpenContext => DeferredAction::OpenContext,
        KeyCommand::CloseContext => DeferredAction::CloseContext,
        KeyCommand::ShowNativeContextMenu => DeferredAction::ShowNativeContextMenu,
        KeyCommand::Hide => DeferredAction::Hide,
        KeyCommand::Backspace => {
            app.input.backspace();
            super::search_flow::on_input_changed(app);
            DeferredAction::None
        }
        KeyCommand::Delete => {
            app.input.delete();
            super::search_flow::on_input_changed(app);
            DeferredAction::None
        }
        KeyCommand::Home => {
            app.input.move_home(shift);
            app.cursor_moved_at = current_time_millis();
            DeferredAction::None
        }
        KeyCommand::End => {
            app.input.move_end(shift);
            app.cursor_moved_at = current_time_millis();
            DeferredAction::None
        }
        KeyCommand::CursorLeft => {
            app.input.move_left(shift);
            app.cursor_moved_at = current_time_millis();
            DeferredAction::None
        }
        KeyCommand::CursorRight => {
            app.input.move_right(shift);
            app.cursor_moved_at = current_time_millis();
            DeferredAction::None
        }
        KeyCommand::SelectAll => {
            app.input.select_all();
            DeferredAction::None
        }
        KeyCommand::Paste => {
            if let Some(text) = super::clipboard::get_text(app.hwnd) {
                if !text.is_empty() {
                    app.input.insert_str(&text);
                    super::search_flow::on_input_changed(app);
                }
            }
            DeferredAction::None
        }
        KeyCommand::Copy => {
            if app.input.has_selection() {
                let selected = app.input.selected_text().to_string();
                super::clipboard::set_text(app.hwnd, &selected);
            }
            DeferredAction::None
        }
        KeyCommand::Cut => {
            if app.input.has_selection() {
                let selected = app.input.selected_text().to_string();
                super::clipboard::set_text(app.hwnd, &selected);
                app.input.backspace();
                super::search_flow::on_input_changed(app);
            }
            DeferredAction::None
        }
        KeyCommand::Autocomplete => {
            if !app.items.is_empty() {
                let idx = app.selected_index.min(app.items.len() - 1);
                let title = app.items[idx].title.clone();
                if !title.is_empty() {
                    app.input.set_text(&title);
                    super::search_flow::on_input_changed(app);
                }
            }
            DeferredAction::None
        }
        KeyCommand::FillHint => {
            if !app.items.is_empty() {
                let idx = app.selected_index.min(app.items.len() - 1);
                let keyword = app.items[idx].title.trim().to_string();
                if !keyword.is_empty() {
                    app.input.set_text(&format!("{keyword} "));
                    app.input.move_end(false);
                    super::search_flow::on_input_changed(app);
                }
            }
            DeferredAction::None
        }
        KeyCommand::None => DeferredAction::None,
    }
}

/// Get current time in milliseconds since UNIX epoch.
#[cfg(windows)]
fn current_time_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
