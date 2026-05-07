mod app;
mod ui;

pub use app::App;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, time::Duration};

use crate::config::Config;

pub async fn run(config: Config) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(config);
    app.start_connect();

    let result = event_loop(&mut terminal, &mut app).await;

    // Always restore the terminal, even on error
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn event_loop<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        // Pull background events (non-blocking)
        app.process_events();

        // Draw frame
        terminal.draw(|f| ui::render(f, app))?;

        // Poll keyboard with a short timeout so the UI refreshes during downloads
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                // Only handle key-press events (not release/repeat on some platforms)
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key).await;
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
