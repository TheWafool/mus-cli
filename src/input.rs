//! Single-line text input with readline-style editing.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use unicode_width::UnicodeWidthChar;

#[derive(Debug, Clone, Default)]
pub struct TextInput {
    value: String,
    /// Cursor position in chars.
    cursor: usize,
}

impl TextInput {
    pub fn new(s: impl Into<String>) -> Self {
        let value = s.into();
        let cursor = value.chars().count();
        Self { value, cursor }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn set(&mut self, s: impl Into<String>) {
        self.value = s.into();
        self.cursor = self.value.chars().count();
    }

    pub fn clear(&mut self) {
        self.set("");
    }

    fn len(&self) -> usize {
        self.value.chars().count()
    }

    fn byte_idx(&self, ci: usize) -> usize {
        self.value
            .char_indices()
            .nth(ci)
            .map(|(b, _)| b)
            .unwrap_or(self.value.len())
    }

    pub fn insert(&mut self, c: char) {
        let b = self.byte_idx(self.cursor);
        self.value.insert(b, c);
        self.cursor += 1;
    }

    /// Insert pasted text; newlines and tabs become spaces.
    pub fn insert_str(&mut self, s: &str) {
        for c in s.chars() {
            match c {
                '\n' | '\r' | '\t' => {
                    if !self.value.ends_with(' ') && self.cursor > 0 {
                        self.insert(' ');
                    }
                }
                c if c.is_control() => {}
                c => self.insert(c),
            }
        }
    }

    fn remove_range(&mut self, from: usize, to: usize) {
        let (a, b) = (self.byte_idx(from), self.byte_idx(to));
        self.value.replace_range(a..b, "");
        self.cursor = from;
    }

    fn word_start_before(&self, pos: usize) -> usize {
        let chars: Vec<char> = self.value.chars().collect();
        let mut i = pos;
        while i > 0 && !chars[i - 1].is_alphanumeric() {
            i -= 1;
        }
        while i > 0 && chars[i - 1].is_alphanumeric() {
            i -= 1;
        }
        i
    }

    fn word_end_after(&self, pos: usize) -> usize {
        let chars: Vec<char> = self.value.chars().collect();
        let mut i = pos;
        while i < chars.len() && !chars[i].is_alphanumeric() {
            i += 1;
        }
        while i < chars.len() && chars[i].is_alphanumeric() {
            i += 1;
        }
        i
    }

    /// Handle an editing key. Returns true if the key was used.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            KeyCode::Char(c) if !ctrl && !alt => self.insert(c),
            KeyCode::Char('w') | KeyCode::Char('h') if ctrl => {
                let s = self.word_start_before(self.cursor);
                self.remove_range(s, self.cursor);
            }
            KeyCode::Backspace if ctrl || alt => {
                let s = self.word_start_before(self.cursor);
                self.remove_range(s, self.cursor);
            }
            KeyCode::Char('u') if ctrl => self.remove_range(0, self.cursor),
            KeyCode::Char('k') if ctrl => {
                let len = self.len();
                let c = self.cursor;
                self.remove_range(c, len);
                self.cursor = c;
            }
            KeyCode::Char('a') if ctrl => self.cursor = 0,
            KeyCode::Char('e') if ctrl => self.cursor = self.len(),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.len(),
            KeyCode::Left if ctrl || alt => self.cursor = self.word_start_before(self.cursor),
            KeyCode::Right if ctrl || alt => self.cursor = self.word_end_after(self.cursor),
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(self.len()),
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.remove_range(self.cursor - 1, self.cursor);
                }
            }
            KeyCode::Delete => {
                if self.cursor < self.len() {
                    let c = self.cursor;
                    self.remove_range(c, c + 1);
                }
            }
            _ => return false,
        }
        true
    }

    /// The visible slice for a box `width` cells wide, and the cursor column.
    pub fn view(&self, width: u16) -> (String, u16) {
        let width = width.max(1) as usize;
        let chars: Vec<char> = self.value.chars().collect();
        let w = |c: &char| c.width().unwrap_or(0);
        // Scroll so the cursor (plus one cell for it) fits on screen.
        let mut start = 0;
        let mut used: usize = chars[..self.cursor].iter().map(w).sum::<usize>() + 1;
        while used > width && start < self.cursor {
            used -= w(&chars[start]);
            start += 1;
        }
        let mut out = String::new();
        let mut cells = 0;
        let mut cursor_x = 0;
        for (i, c) in chars.iter().enumerate().skip(start) {
            if i == self.cursor {
                cursor_x = cells;
            }
            if cells + w(c) > width {
                break;
            }
            cells += w(c);
            out.push(*c);
        }
        if self.cursor >= chars.len() {
            cursor_x = cells;
        }
        (out, cursor_x as u16)
    }
}
