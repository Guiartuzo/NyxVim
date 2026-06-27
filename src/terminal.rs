//! Terminal lifecycle: raw mode, the alternate screen, and panic-safe teardown.
//!
//! NyxVim takes over the terminal on startup and must always hand it back in a
//! usable state — on a normal quit, on an error, and even on a panic.

use std::io::{self, Stdout};

use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

/// The concrete terminal type the rest of the app draws to.
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Enter raw mode and the alternate screen, install the panic hook, and return
/// a ready-to-draw terminal. The caller is responsible for calling [`restore`].
pub fn init() -> io::Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    install_panic_hook();
    Terminal::new(CrosstermBackend::new(stdout))
}

/// Restore the terminal to its pre-launch state: cooked mode, main screen.
pub fn restore() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}

/// Wrap the existing panic hook so the terminal is restored before the panic
/// message is printed — otherwise the message would render into raw mode and be
/// unreadable, and the user's shell would be left broken.
fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore();
        original(info);
    }));
}
