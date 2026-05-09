//! ratatui TUI — interactive profile/session navigator.

pub mod model;
pub mod view;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use model::AppState;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

/// Entry point for the TUI. Blocks until the user quits.
pub async fn run() -> Result<()> {
    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let mut state = AppState::load();

    loop {
        terminal.draw(|f| view::render(f, &state))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                handle_key(&mut state, key.code, key.modifiers);
            }
        }

        if state.should_quit {
            break;
        }
    }

    Ok(())
}

fn handle_key(state: &mut AppState, code: KeyCode, modifiers: KeyModifiers) {
    match code {
        // Quit
        KeyCode::Char('q') | KeyCode::Char('Q') => state.should_quit = true,
        KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => state.should_quit = true,

        // Tab navigation
        KeyCode::Tab | KeyCode::Right => state.next_tab(),
        KeyCode::BackTab | KeyCode::Left => state.prev_tab(),

        // List navigation
        KeyCode::Down | KeyCode::Char('j') => state.move_down(),
        KeyCode::Up | KeyCode::Char('k') => state.move_up(),

        // Refresh
        KeyCode::Char('r') | KeyCode::Char('R') => state.refresh(),

        // Enter — activate selected profile:session
        KeyCode::Enter => {
            if let Some(profile) = state.selected_profile_name().map(String::from) {
                let sessions = state.selected_profile_sessions();
                let session = sessions
                    .get(state.selected_session)
                    .cloned()
                    .unwrap_or_else(|| "default".to_string());

                // Write pending switch (shell will pick it up via precmd)
                match cst_core::auto_switch::daemon::write_pending_switch(&profile, &session) {
                    Err(e) => {
                        state.status_message = format!("Error switching: {e}");
                    }
                    Ok(()) => {
                        // Update global config
                        if let Ok(mut cfg) = cst_core::config::GlobalConfig::load() {
                            cfg.current_profile = profile.clone();
                            cfg.current_session = session.clone();
                            if let Err(e) = cfg.save() {
                                tracing::warn!("failed to save active profile in TUI: {e}");
                            }
                        }

                        state.current_profile = profile.clone();
                        state.current_session = session.clone();
                        state.status_message = format!("Switched to {}:{}", profile, session);
                    }
                }

                // Refresh display
                state.refresh();
            }
        }

        _ => {}
    }
}
