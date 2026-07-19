// Copyright (c) 2025-2026 LIJIALU. MIT License.

//! Text input state management.
//! Handles cursor position, selection, and text editing operations.

/// Text input buffer with cursor/selection tracking.
#[derive(Debug, Clone)]
pub struct InputState {
    /// Current text content (UTF-16 for Win32 compatibility).
    text: String,
    /// Cursor position (byte offset in `text`).
    cursor: usize,
    /// Selection anchor (byte offset, or same as cursor if no selection).
    selection_anchor: usize,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
            selection_anchor: 0,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn has_selection(&self) -> bool {
        self.cursor != self.selection_anchor
    }

    pub fn selection_range(&self) -> (usize, usize) {
        let start = self.cursor.min(self.selection_anchor);
        let end = self.cursor.max(self.selection_anchor);
        (start, end)
    }

    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, ch: char) {
        self.delete_selection();
        self.text.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
        self.selection_anchor = self.cursor;
    }

    /// Insert a string at the cursor position.
    pub fn insert_str(&mut self, s: &str) {
        self.delete_selection();
        self.text.insert_str(self.cursor, s);
        self.cursor += s.len();
        self.selection_anchor = self.cursor;
    }

    /// Delete character before cursor (Backspace).
    pub fn backspace(&mut self) {
        if self.has_selection() {
            self.delete_selection();
            return;
        }
        if self.cursor == 0 {
            return;
        }
        // Find previous char boundary
        let prev = self.text[..self.cursor]
            .char_indices()
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.text.drain(prev..self.cursor);
        self.cursor = prev;
        self.selection_anchor = self.cursor;
    }

    /// Delete character at cursor (Delete key).
    pub fn delete(&mut self) {
        if self.has_selection() {
            self.delete_selection();
            return;
        }
        if self.cursor >= self.text.len() {
            return;
        }
        let next = self.cursor
            + self.text[self.cursor..]
                .chars()
                .next()
                .map_or(0, |c| c.len_utf8());
        self.text.drain(self.cursor..next);
    }

    /// Move cursor left.
    pub fn move_left(&mut self, select: bool) {
        if self.cursor > 0 {
            let prev = self.text[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.cursor = prev;
        }
        if !select {
            self.selection_anchor = self.cursor;
        }
    }

    /// Move cursor right.
    pub fn move_right(&mut self, select: bool) {
        if self.cursor < self.text.len() {
            let next = self.cursor
                + self.text[self.cursor..]
                    .chars()
                    .next()
                    .map_or(0, |c| c.len_utf8());
            self.cursor = next;
        }
        if !select {
            self.selection_anchor = self.cursor;
        }
    }

    /// Move cursor to the beginning.
    pub fn move_home(&mut self, select: bool) {
        self.cursor = 0;
        if !select {
            self.selection_anchor = self.cursor;
        }
    }

    /// Move cursor to the end.
    pub fn move_end(&mut self, select: bool) {
        self.cursor = self.text.len();
        if !select {
            self.selection_anchor = self.cursor;
        }
    }

    /// Select all text.
    pub fn select_all(&mut self) {
        self.selection_anchor = 0;
        self.cursor = self.text.len();
    }

    /// Clear all text.
    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
        self.selection_anchor = 0;
    }

    /// Set text content (replaces everything).
    pub fn set_text(&mut self, text: &str) {
        self.text = text.to_string();
        self.cursor = self.text.len();
        self.selection_anchor = self.cursor;
    }

    /// Delete the current selection (if any).
    fn delete_selection(&mut self) {
        if !self.has_selection() {
            return;
        }
        let (start, end) = self.selection_range();
        self.text.drain(start..end);
        self.cursor = start;
        self.selection_anchor = start;
    }

    /// Get selected text.
    pub fn selected_text(&self) -> &str {
        if !self.has_selection() {
            return "";
        }
        let (start, end) = self.selection_range();
        &self.text[start..end]
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}
