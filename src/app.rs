//! The central application state and the main render/event loop.
//!
//! [`App`] is the single owner of all application state. Later milestones add
//! buffers, panes, and the file tree here; for now it tracks only whether the
//! editor should quit.

use std::io;

use ratatui::Frame;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Alignment;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::terminal::Tui;

/// Central application state — the single owner of everything NyxVim tracks.
#[derive(Debug, Default)]
pub struct App {
    should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        Self::default()
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

    /// Handle a key press. Global keys are handled here; later milestones route
    /// non-global keys to the focused component.
    fn on_key(&mut self, key: KeyEvent) {
        if is_quit(key) {
            self.should_quit = true;
        }
    }

    fn render(&self, frame: &mut Frame) {
        let block = Block::default().title(" NyxVim ").borders(Borders::ALL);
        let body = Paragraph::new("Welcome to NyxVim\n\nPress Ctrl+Q to quit.")
            .block(block)
            .alignment(Alignment::Center);
        frame.render_widget(body, frame.area());
    }
}

/// The global quit chord: Ctrl+Q.
fn is_quit(key: KeyEvent) -> bool {
    key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn ctrl_q_requests_quit() {
        let mut app = App::new();
        assert!(!app.should_quit);
        app.on_key(press(KeyCode::Char('q'), KeyModifiers::CONTROL));
        assert!(app.should_quit);
    }

    #[test]
    fn plain_q_does_not_quit() {
        let mut app = App::new();
        app.on_key(press(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(!app.should_quit);
    }
}
