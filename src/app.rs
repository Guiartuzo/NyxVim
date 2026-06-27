//! The central application state and the main render/event loop.
//!
//! [`App`] is the single owner of all application state: the central buffer
//! store, the list of panes (each referencing a buffer by id), and which pane
//! is focused. The file tree is added in a later milestone.

use std::io;

use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout};

use crate::buffer::Buffer;
use crate::pane::EditorPane;
use crate::terminal::Tui;

/// Central application state — the single owner of everything NyxVim tracks.
#[derive(Debug)]
pub struct App {
    should_quit: bool,
    /// Central buffer store; panes reference entries by index (`buffer_id`).
    buffers: Vec<Buffer>,
    /// Side-by-side editor panes (vertical splits).
    panes: Vec<EditorPane>,
    /// Index into `panes` of the pane receiving input.
    focused: usize,
}

impl App {
    /// Start with a single pane viewing `buffer`.
    pub fn new(buffer: Buffer) -> Self {
        Self {
            should_quit: false,
            buffers: vec![buffer],
            panes: vec![EditorPane::new(0)],
            focused: 0,
        }
    }

    /// Run the main loop until a quit is requested.
    ///
    /// Each iteration draws the current state, then blocks waiting for the next
    /// input event. Blocking on input means an idle editor consumes no CPU and
    /// never redraws on its own.
    pub fn run(&mut self, terminal: &mut Tui) -> io::Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| self.render(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn render(&mut self, frame: &mut Frame) {
        // Divide the width evenly across panes (vertical splits).
        let constraints = vec![Constraint::Fill(1); self.panes.len()];
        let regions = Layout::horizontal(constraints).split(frame.area());
        for (i, pane) in self.panes.iter_mut().enumerate() {
            let buffer = &self.buffers[pane.buffer_id];
            pane.render(frame, regions[i], buffer, i == self.focused);
        }
    }

    /// Block for the next event and update state accordingly.
    fn handle_events(&mut self) -> io::Result<()> {
        if let Event::Key(key) = event::read()? {
            // Only react to presses, not key-release/repeat events some
            // terminals emit, so a single keystroke does one thing.
            if key.kind == KeyEventKind::Press {
                self.on_key(key);
            }
        }
        Ok(())
    }

    /// Handle a key press. Global chords (quit, save, pane management) are
    /// handled here; the rest are dispatched to the focused pane.
    fn on_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);

        match key.code {
            KeyCode::Char('q') if ctrl => self.should_quit = true,
            KeyCode::Char('s') if ctrl => self.save_focused(),
            KeyCode::Char('\\') if ctrl => self.split_vertical(),
            KeyCode::Char('w') if ctrl => self.close_focused_pane(),
            KeyCode::Left if alt => self.focus_prev(),
            KeyCode::Right if alt => self.focus_next(),
            _ => self.dispatch_to_focused(key),
        }
    }

    /// Route an editing/movement key to the focused pane and its buffer.
    fn dispatch_to_focused(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let extend = key.modifiers.contains(KeyModifiers::SHIFT);

        let pane = &mut self.panes[self.focused];
        let buffer = &mut self.buffers[pane.buffer_id];

        match key.code {
            KeyCode::Left => pane.move_left(buffer, extend),
            KeyCode::Right => pane.move_right(buffer, extend),
            KeyCode::Up => pane.move_up(buffer, extend),
            KeyCode::Down => pane.move_down(buffer, extend),
            KeyCode::Enter => pane.insert_newline(buffer),
            KeyCode::Backspace => pane.backspace(buffer),
            KeyCode::Delete => pane.delete_forward(buffer),
            KeyCode::Tab => {
                // Insert spaces so one character always equals one column,
                // keeping cursor placement correct.
                for _ in 0..4 {
                    pane.insert_char(buffer, ' ');
                }
            }
            // Printable input: any char that isn't part of a Ctrl/Alt chord.
            KeyCode::Char(c) if !ctrl && !alt => pane.insert_char(buffer, c),
            _ => {}
        }
    }

    // --- pane management ---------------------------------------------------

    fn save_focused(&mut self) {
        let buffer_id = self.panes[self.focused].buffer_id;
        let _ = self.buffers[buffer_id].save();
    }

    /// Split the focused pane, placing a new pane viewing the same buffer
    /// beside it and moving focus to the new pane.
    fn split_vertical(&mut self) {
        let buffer_id = self.panes[self.focused].buffer_id;
        self.panes.insert(self.focused + 1, EditorPane::new(buffer_id));
        self.focused += 1;
    }

    /// Close the focused pane, unless it is the last one.
    fn close_focused_pane(&mut self) {
        if self.panes.len() <= 1 {
            return;
        }
        self.panes.remove(self.focused);
        if self.focused >= self.panes.len() {
            self.focused = self.panes.len() - 1;
        }
    }

    fn focus_next(&mut self) {
        self.focused = (self.focused + 1) % self.panes.len();
    }

    fn focus_prev(&mut self) {
        self.focused = (self.focused + self.panes.len() - 1) % self.panes.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> App {
        App::new(Buffer::from_str("hello"))
    }

    fn press(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn ctrl_q_requests_quit() {
        let mut app = test_app();
        assert!(!app.should_quit);
        app.on_key(press(KeyCode::Char('q'), KeyModifiers::CONTROL));
        assert!(app.should_quit);
    }

    #[test]
    fn plain_q_inserts_into_focused_pane() {
        let mut app = test_app();
        app.on_key(press(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(!app.should_quit);
        assert_eq!(app.buffers[0].line_text(0), "qhello");
    }

    #[test]
    fn split_adds_pane_and_focuses_it() {
        let mut app = test_app();
        app.split_vertical();
        assert_eq!(app.panes.len(), 2);
        assert_eq!(app.focused, 1);
        // both panes view the same buffer
        assert_eq!(app.panes[0].buffer_id, app.panes[1].buffer_id);
    }

    #[test]
    fn focus_cycles_through_panes() {
        let mut app = test_app();
        app.split_vertical(); // focused = 1
        app.focus_next(); // wraps to 0
        assert_eq!(app.focused, 0);
        app.focus_prev(); // wraps to 1
        assert_eq!(app.focused, 1);
    }

    #[test]
    fn close_pane_keeps_at_least_one() {
        let mut app = test_app();
        app.split_vertical();
        app.close_focused_pane();
        assert_eq!(app.panes.len(), 1);
        assert_eq!(app.focused, 0);
        // closing the last pane is a no-op
        app.close_focused_pane();
        assert_eq!(app.panes.len(), 1);
    }
}
