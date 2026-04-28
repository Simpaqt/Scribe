use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use std::{env, io, path::Path, process::Command};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::app::{App, AppMode};
use crate::notes::{create_new_note, delete_note};

/// Get the appropriate text editor for the current platform
fn get_editor() -> String {
    // First check EDITOR environment variable
    if let Ok(editor) = env::var("EDITOR") {
        return editor;
    }
    
    // Platform-specific defaults
    #[cfg(target_os = "windows")]
    {
        // Try common Windows editors
        if Command::new("code").arg("--version").output().is_ok() {
            return "code".to_string();
        }
        if Command::new("notepad++").arg("--version").output().is_ok() {
            return "notepad++".to_string();
        }
        "notepad".to_string()
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        // Unix-like systems (Linux, macOS)
        "nvim".to_string()
    }
}

pub fn run_tui(directory: &str) -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(directory);

    loop {
        terminal.draw(|f| {
            // Create main layout with status bar at bottom
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
                .split(f.area());

            let chunks = if app.mode == AppMode::Create || app.mode == AppMode::Search {
                Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(80), Constraint::Percentage(20)].as_ref())
                    .split(main_chunks[0])
            } else {
                Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(100)].as_ref())
                    .split(main_chunks[0])
            };

            let current_notes = app.get_current_notes().clone();
            let items: Vec<ListItem> = if current_notes.is_empty() {
                let empty_message = match app.mode {
                    AppMode::Search => "No notes match your search. Try a different query.",
                    _ => "No notes found. Press 'i' to create your first note!",
                };
                vec![ListItem::new(empty_message).style(Style::default().fg(Color::Gray))]
            } else {
                current_notes
                    .iter()
                    .map(|note| ListItem::new(note.as_str()).style(Style::default().fg(Color::White)))
                    .collect()
            };

            let title = match app.mode {
                AppMode::Normal => format!("📝 Rustymemo - {} notes", current_notes.len()),
                AppMode::Create => format!("📝 Rustymemo - Creating new note ({} total)", app.notes.len()),
                AppMode::Search => {
                    if app.search_query.is_empty() {
                        format!("🔍 Search - {} notes", app.notes.len())
                    } else {
                        format!("🔍 Search: '{}' - {} matches", app.search_query, current_notes.len())
                    }
                }
            };

            let list = List::new(items)
                .block(
                    Block::default()
                        .title(title)
                        .borders(Borders::ALL)
                        .style(Style::default().fg(Color::Blue)),
                )
                .highlight_style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("▶ ");

            f.render_stateful_widget(list, chunks[0], &mut app.state);

            if app.mode == AppMode::Create {
                let input_block = Block::default()
                    .title("✏️  New Note Name (Enter to create, Esc to cancel)")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Green));
                let input = Paragraph::new(app.input.as_str())
                    .block(input_block)
                    .style(Style::default().fg(Color::White));
                f.render_widget(input, chunks[1]);
            } else if app.mode == AppMode::Search {
                let search_block = Block::default()
                    .title("🔍 Search Notes (Enter to open, Esc to exit)")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Magenta));
                let search_input = Paragraph::new(app.search_query.as_str())
                    .block(search_block)
                    .style(Style::default().fg(Color::White));
                f.render_widget(search_input, chunks[1]);
            }

            // Render status bar with keybinding hints
            let (status_text, status_color) = match app.mode {
                AppMode::Normal => {
                    if app.pending_delete {
                        ("⚠️  Press 'd' again to confirm deletion, or any other key to cancel", Color::Red)
                    } else {
                        ("💡 j/k: navigate | o: open | i: new note | /: search | dd: delete | q: quit", Color::Cyan)
                    }
                }
                AppMode::Create => ("✏️  Type note name and press Enter to create, Esc to cancel", Color::Green),
                AppMode::Search => ("🔍 Type to search | j/k: navigate | Enter: open | Esc: exit search", Color::Magenta),
            };

            let status_bar = Paragraph::new(status_text)
                .block(
                    Block::default()
                        .title("💡 Help & Keybindings")
                        .borders(Borders::ALL)
                        .style(Style::default().fg(status_color)),
                )
                .style(Style::default().fg(Color::White));

            f.render_widget(status_bar, main_chunks[1]);
        })?;

        if let Event::Key(key) = event::read()? {
            match app.mode {
                AppMode::Normal => match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('i') => {
                        app.mode = AppMode::Create;
                        app.input.clear();
                        app.pending_delete = false;
                    }
                    KeyCode::Char('/') => {
                        app.mode = AppMode::Search;
                        app.search_query.clear();
                        app.update_filtered_notes();
                        app.pending_delete = false;
                    }
                    KeyCode::Char('o') => {
                        if let Some(selected) = app.state.selected() {
                            let current_notes = app.get_current_notes();
                            if let Some(note) = current_notes.get(selected) {
                                let file_path = Path::new(&app.directory).join(note);
                                disable_raw_mode()?;
                                execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
                                Command::new(get_editor()).arg(&file_path).status()?;
                                enable_raw_mode()?;
                                execute!(terminal.backend_mut(), EnterAlternateScreen)?;
                                terminal.clear()?;
                                app.pending_delete = false;
                            }
                        }
                    }
                    KeyCode::Char('d') => {
                        if app.pending_delete {
                            if let Some(selected) = app.state.selected() {
                                let current_notes = app.get_current_notes();
                                if let Some(note) = current_notes.get(selected) {
                                    let file_path = Path::new(&app.directory).join(note);
                                    if delete_note(file_path.to_str().unwrap()).is_ok() {
                                        app.refresh_notes();
                                    }
                                }
                            }
                            app.pending_delete = false;
                        } else {
                            app.pending_delete = true;
                        }
                    }
                    KeyCode::Char('j') => {
                        let selected = app.state.selected().unwrap_or(0);
                        let current_notes = app.get_current_notes();
                        if selected < current_notes.len() - 1 {
                            app.state.select(Some(selected + 1));
                        }
                        app.pending_delete = false;
                    }
                    KeyCode::Char('k') => {
                        let selected = app.state.selected().unwrap_or(0);
                        if selected > 0 {
                            app.state.select(Some(selected - 1));
                        }
                        app.pending_delete = false;
                    }
                    _ => {
                        app.pending_delete = false;
                    }
                },
                AppMode::Create => match key.code {
                    KeyCode::Enter if !app.input.is_empty() => {
                        let file_path = Path::new(&app.directory).join(&app.input);
                        if !file_path.exists()
                            && create_new_note(file_path.to_str().unwrap()).is_ok() {
                                app.refresh_notes();
                            }
                        app.input.clear();
                        app.mode = AppMode::Normal;
                    }
                    KeyCode::Enter => {}
                    KeyCode::Esc => {
                        app.input.clear();
                        app.mode = AppMode::Normal;
                    }
                    KeyCode::Char(c) => {
                        app.input.push(c);
                    }
                    KeyCode::Backspace => {
                        app.input.pop();
                    }
                    _ => {}
                },
                AppMode::Search => match key.code {
                    KeyCode::Esc => {
                        app.mode = AppMode::Normal;
                        app.search_query.clear();
                        app.update_filtered_notes();
                    }
                    KeyCode::Enter => {
                        if let Some(selected) = app.state.selected() {
                            if let Some(note) = app.filtered_notes.get(selected) {
                                let file_path = Path::new(&app.directory).join(note);
                                disable_raw_mode()?;
                                execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
                                Command::new(get_editor()).arg(&file_path).status()?;
                                enable_raw_mode()?;
                                execute!(terminal.backend_mut(), EnterAlternateScreen)?;
                                terminal.clear()?;
                                app.mode = AppMode::Normal;
                                app.search_query.clear();
                                app.update_filtered_notes();
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        app.search_query.pop();
                        app.update_filtered_notes();
                    }
                    KeyCode::Up => {
                        let selected = app.state.selected().unwrap_or(0);
                        if selected > 0 {
                            app.state.select(Some(selected - 1));
                        }
                    }
                    KeyCode::Down => {
                        let selected = app.state.selected().unwrap_or(0);
                        if selected < app.filtered_notes.len() - 1 {
                            app.state.select(Some(selected + 1));
                        }
                    }
                    KeyCode::Char('k') => {
                        let selected = app.state.selected().unwrap_or(0);
                        if selected > 0 {
                            app.state.select(Some(selected - 1));
                        }
                    }
                    KeyCode::Char('j') => {
                        let selected = app.state.selected().unwrap_or(0);
                        if selected < app.filtered_notes.len() - 1 {
                            app.state.select(Some(selected + 1));
                        }
                    }
                    KeyCode::Char(c) => {
                        app.search_query.push(c);
                        app.update_filtered_notes();
                    }
                    _ => {}
                },
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}
