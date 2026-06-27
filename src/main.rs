mod app;
mod terminal;

use app::App;

fn main() -> std::io::Result<()> {
    let mut tui = terminal::init()?;
    let result = App::new().run(&mut tui);
    // Always restore the terminal, even if the run loop returned an error.
    terminal::restore()?;
    result
}
