//! An editor view over a [`Buffer`]: the cursor, the selection, the scroll
//! offset, and the modeless editing operations driven by the keyboard.

use std::io;

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Text};
use ratatui::widgets::Paragraph;

use crate::buffer::Buffer;

/// Cursor position within the buffer. `target_col` remembers the column the
/// user "wants" so vertical movement across short lines doesn't lose it.
#[derive(Debug, Default, Clone, Copy)]
pub struct Cursor {
    pub line: usize,
    pub col: usize,
    pub target_col: usize,
}

#[derive(Debug)]
pub struct EditorView {
    pub buffer: Buffer,
    pub cursor: Cursor,
    /// Selection anchor (line, col). The selection spans from here to the
    /// cursor; `None` means no active selection.
    anchor: Option<(usize, usize)>,
    scroll_row: usize,
    scroll_col: usize,
}

impl EditorView {
    pub fn new(buffer: Buffer) -> Self {
        Self {
            buffer,
            cursor: Cursor::default(),
            anchor: None,
            scroll_row: 0,
            scroll_col: 0,
        }
    }

    // --- queries -----------------------------------------------------------

    pub fn has_selection(&self) -> bool {
        self.anchor.is_some()
    }

    fn last_line(&self) -> usize {
        self.buffer.line_count() - 1
    }

    fn line_len(&self, line: usize) -> usize {
        self.buffer.line_len_chars(line)
    }

    /// The selection as an ordered (start, end) pair, if any.
    fn ordered_selection(&self) -> Option<((usize, usize), (usize, usize))> {
        let anchor = self.anchor?;
        let cursor = (self.cursor.line, self.cursor.col);
        Some(if anchor <= cursor {
            (anchor, cursor)
        } else {
            (cursor, anchor)
        })
    }

    // --- movement ----------------------------------------------------------

    /// Manage the selection anchor for a movement: extend keeps/sets the
    /// anchor, a plain move collapses any selection.
    fn pre_move(&mut self, extend: bool) {
        if extend {
            if self.anchor.is_none() {
                self.anchor = Some((self.cursor.line, self.cursor.col));
            }
        } else {
            self.anchor = None;
        }
    }

    pub fn move_left(&mut self, extend: bool) {
        self.pre_move(extend);
        if self.cursor.col > 0 {
            self.cursor.col -= 1;
        } else if self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.cursor.col = self.line_len(self.cursor.line);
        }
        self.cursor.target_col = self.cursor.col;
    }

    pub fn move_right(&mut self, extend: bool) {
        self.pre_move(extend);
        if self.cursor.col < self.line_len(self.cursor.line) {
            self.cursor.col += 1;
        } else if self.cursor.line < self.last_line() {
            self.cursor.line += 1;
            self.cursor.col = 0;
        }
        self.cursor.target_col = self.cursor.col;
    }

    pub fn move_up(&mut self, extend: bool) {
        self.pre_move(extend);
        if self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.cursor.col = self.cursor.target_col.min(self.line_len(self.cursor.line));
        }
    }

    pub fn move_down(&mut self, extend: bool) {
        self.pre_move(extend);
        if self.cursor.line < self.last_line() {
            self.cursor.line += 1;
            self.cursor.col = self.cursor.target_col.min(self.line_len(self.cursor.line));
        }
    }

    // --- editing -----------------------------------------------------------

    /// Delete the active selection, if any, moving the cursor to its start.
    /// Returns whether a selection was deleted.
    fn delete_selection(&mut self) -> bool {
        let Some((start, end)) = self.ordered_selection() else {
            return false;
        };
        let s = self.buffer.char_idx(start.0, start.1);
        let e = self.buffer.char_idx(end.0, end.1);
        self.buffer.remove(s, e);
        self.cursor.line = start.0;
        self.cursor.col = start.1;
        self.cursor.target_col = start.1;
        self.anchor = None;
        true
    }

    pub fn insert_char(&mut self, ch: char) {
        self.delete_selection();
        let idx = self.buffer.char_idx(self.cursor.line, self.cursor.col);
        let mut encoded = [0u8; 4];
        self.buffer.insert(idx, ch.encode_utf8(&mut encoded));
        self.cursor.col += 1;
        self.cursor.target_col = self.cursor.col;
    }

    pub fn insert_newline(&mut self) {
        self.delete_selection();
        let idx = self.buffer.char_idx(self.cursor.line, self.cursor.col);
        self.buffer.insert(idx, "\n");
        self.cursor.line += 1;
        self.cursor.col = 0;
        self.cursor.target_col = 0;
    }

    /// Delete the character before the cursor, joining lines at a line start.
    pub fn backspace(&mut self) {
        if self.delete_selection() {
            return;
        }
        let idx = self.buffer.char_idx(self.cursor.line, self.cursor.col);
        if idx == 0 {
            return;
        }
        self.buffer.remove(idx - 1, idx);
        let (line, col) = self.buffer.line_col(idx - 1);
        self.cursor.line = line;
        self.cursor.col = col;
        self.cursor.target_col = col;
    }

    /// Delete the character at the cursor, joining lines at a line end.
    pub fn delete_forward(&mut self) {
        if self.delete_selection() {
            return;
        }
        let idx = self.buffer.char_idx(self.cursor.line, self.cursor.col);
        if idx < self.buffer.len_chars() {
            self.buffer.remove(idx, idx + 1);
        }
    }

    pub fn save(&mut self) -> io::Result<bool> {
        self.buffer.save()
    }

    // --- rendering ---------------------------------------------------------

    /// Render the editor into `area`: the buffer's visible region plus a
    /// one-row status bar at the bottom.
    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let [content, status] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(area);

        self.render_content(frame, content);
        self.render_status(frame, status);
    }

    fn render_content(&mut self, frame: &mut Frame, area: Rect) {
        let height = area.height as usize;
        let width = area.width as usize;

        self.scroll_row = scroll_to_show(self.scroll_row, self.cursor.line, height);
        self.scroll_col = scroll_to_show(self.scroll_col, self.cursor.col, width);

        let mut lines: Vec<Line> = Vec::with_capacity(height);
        for row in 0..height {
            let line_idx = self.scroll_row + row;
            if line_idx >= self.buffer.line_count() {
                break;
            }
            let text = self.buffer.line_text(line_idx);
            let visible: String = text.chars().skip(self.scroll_col).collect();
            lines.push(Line::raw(visible));
        }
        frame.render_widget(Paragraph::new(Text::from(lines)), area);

        let cx = area.x + (self.cursor.col.saturating_sub(self.scroll_col)) as u16;
        let cy = area.y + (self.cursor.line.saturating_sub(self.scroll_row)) as u16;
        frame.set_cursor_position((cx, cy));
    }

    fn render_status(&self, frame: &mut Frame, area: Rect) {
        let name = self
            .buffer
            .path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "[No Name]".to_string());
        let dirty = if self.buffer.is_dirty() { " [+]" } else { "" };
        let text = format!(
            " {name}{dirty}    Ln {}, Col {} ",
            self.cursor.line + 1,
            self.cursor.col + 1
        );
        let bar = Paragraph::new(text).style(Style::new().bg(Color::DarkGray).fg(Color::White));
        frame.render_widget(bar, area);
    }
}

/// Given the current scroll offset, the cursor index, and the viewport size on
/// one axis, return the scroll offset that keeps the cursor visible while
/// moving as little as possible.
fn scroll_to_show(scroll: usize, cursor: usize, viewport: usize) -> usize {
    if viewport == 0 {
        return scroll;
    }
    if cursor < scroll {
        cursor
    } else if cursor >= scroll + viewport {
        cursor + 1 - viewport
    } else {
        scroll
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(text: &str) -> EditorView {
        EditorView::new(Buffer::from_str(text))
    }

    #[test]
    fn cursor_inside_viewport_does_not_scroll() {
        assert_eq!(scroll_to_show(10, 12, 20), 10);
    }

    #[test]
    fn cursor_above_viewport_scrolls_up_to_cursor() {
        assert_eq!(scroll_to_show(10, 4, 20), 4);
    }

    #[test]
    fn cursor_below_viewport_scrolls_just_enough() {
        assert_eq!(scroll_to_show(10, 35, 20), 16);
    }

    #[test]
    fn zero_viewport_is_a_noop() {
        assert_eq!(scroll_to_show(7, 100, 0), 7);
    }

    #[test]
    fn vertical_move_preserves_target_column() {
        let mut v = view("abcde\nxy\nlongerline");
        for _ in 0..4 {
            v.move_right(false);
        }
        assert_eq!((v.cursor.line, v.cursor.col), (0, 4));
        v.move_down(false); // "xy" len 2 -> clamp to 2, target stays 4
        assert_eq!((v.cursor.line, v.cursor.col), (1, 2));
        assert_eq!(v.cursor.target_col, 4);
        v.move_down(false); // "longerline" -> col back to 4
        assert_eq!((v.cursor.line, v.cursor.col), (2, 4));
    }

    #[test]
    fn move_left_wraps_to_previous_line_end() {
        let mut v = view("ab\ncd");
        v.move_down(false);
        assert_eq!((v.cursor.line, v.cursor.col), (1, 0));
        v.move_left(false);
        assert_eq!((v.cursor.line, v.cursor.col), (0, 2));
    }

    #[test]
    fn insert_chars_and_newline() {
        let mut v = view("");
        v.insert_char('h');
        v.insert_char('i');
        assert_eq!(v.buffer.line_text(0), "hi");
        assert_eq!(v.cursor.col, 2);
        v.insert_newline();
        assert_eq!((v.cursor.line, v.cursor.col), (1, 0));
        v.insert_char('x');
        assert_eq!(v.buffer.line_text(1), "x");
        assert!(v.buffer.is_dirty());
    }

    #[test]
    fn enter_splits_line() {
        let mut v = view("abcd");
        v.move_right(false);
        v.move_right(false);
        v.insert_newline();
        assert_eq!(v.buffer.line_text(0), "ab");
        assert_eq!(v.buffer.line_text(1), "cd");
        assert_eq!((v.cursor.line, v.cursor.col), (1, 0));
    }

    #[test]
    fn backspace_at_line_start_joins_lines() {
        let mut v = view("ab\ncd");
        v.move_down(false);
        v.backspace();
        assert_eq!(v.buffer.line_text(0), "abcd");
        assert_eq!((v.cursor.line, v.cursor.col), (0, 2));
    }

    #[test]
    fn delete_forward_at_line_end_joins_lines() {
        let mut v = view("ab\ncd");
        v.move_right(false);
        v.move_right(false);
        v.delete_forward();
        assert_eq!(v.buffer.line_text(0), "abcd");
        assert_eq!((v.cursor.line, v.cursor.col), (0, 2));
    }

    #[test]
    fn shift_arrow_selects_and_typing_replaces() {
        let mut v = view("hello");
        v.move_right(true);
        v.move_right(true);
        v.move_right(true); // select "hel"
        assert!(v.has_selection());
        v.insert_char('H');
        assert_eq!(v.buffer.line_text(0), "Hlo");
        assert_eq!(v.cursor.col, 1);
        assert!(!v.has_selection());
    }

    #[test]
    fn plain_move_collapses_selection() {
        let mut v = view("hello");
        v.move_right(true);
        assert!(v.has_selection());
        v.move_right(false);
        assert!(!v.has_selection());
    }

    #[test]
    fn backspace_removes_selection() {
        let mut v = view("hello");
        v.move_right(true);
        v.move_right(true); // select "he"
        v.backspace();
        assert_eq!(v.buffer.line_text(0), "llo");
        assert!(!v.has_selection());
        assert_eq!(v.cursor.col, 0);
    }

    #[test]
    fn save_writes_file_and_clears_dirty() {
        use std::io::Read;
        let path =
            std::env::temp_dir().join(format!("nyxvim_save_test_{}.txt", std::process::id()));
        std::fs::write(&path, "old").unwrap();

        let mut v = EditorView::new(Buffer::from_path(&path).unwrap());
        for _ in 0..3 {
            v.move_right(false);
        }
        v.insert_char('!');
        assert!(v.buffer.is_dirty());

        assert!(v.save().unwrap());
        assert!(!v.buffer.is_dirty());

        let mut written = String::new();
        std::fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut written)
            .unwrap();
        assert_eq!(written, "old!");
        std::fs::remove_file(&path).ok();
    }
}
