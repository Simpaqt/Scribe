use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};
use std::{env, io, path::Path, process::Command};

use crate::app::{App, AppMode, Pending};
use crate::notes::{create_new_note, delete_note, ensure_adoc_extension, strip_adoc_extension};
use crate::render::{export, render_preview};
use crate::templates::Template;

/// Get the appropriate text editor for the current platform.
fn get_editor() -> String {
    if let Ok(editor) = env::var("EDITOR") {
        return editor;
    }

    #[cfg(target_os = "windows")]
    {
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
        "nvim".to_string()
    }
}

fn open_in_editor<B: ratatui::backend::Backend + std::io::Write>(
    terminal: &mut Terminal<B>,
    file_path: &Path,
) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Command::new(get_editor()).arg(file_path).status()?;
    enable_raw_mode()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    terminal.clear()?;
    Ok(())
}

fn ensure_preview(app: &mut App) {
    let Some(name) = app.selected_note().cloned() else {
        app.preview_cache = None;
        return;
    };
    if let Some((cached_name, _)) = &app.preview_cache {
        if cached_name == &name {
            return;
        }
    }
    let path = Path::new(&app.directory).join(&name);
    let text = match render_preview(&path) {
        Ok(t) => t,
        Err(e) => format!("[preview unavailable]\n\n{e}\n\nInstall `asciidoctor` to enable previews."),
    };
    app.preview_cache = Some((name, text));
}

pub fn run_tui(directory: &str, show_all: bool) -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(directory, show_all);

    loop {
        if app.preview_open && app.mode == AppMode::Normal {
            ensure_preview(&mut app);
        }

        terminal.draw(|f| {
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
                .split(f.area());

            // Vertical split for input/template-picker overlay at the bottom.
            let body = if matches!(app.mode, AppMode::Create | AppMode::Search | AppMode::TemplatePick) {
                Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(75), Constraint::Percentage(25)].as_ref())
                    .split(main_chunks[0])
            } else {
                Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Percentage(100)].as_ref())
                    .split(main_chunks[0])
            };

            // Horizontal split if preview is open.
            let top = if app.preview_open && app.mode == AppMode::Normal {
                Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(40), Constraint::Percentage(60)].as_ref())
                    .split(body[0])
            } else {
                Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(100)].as_ref())
                    .split(body[0])
            };

            let current_notes = app.get_current_notes().clone();
            let items: Vec<ListItem> = if current_notes.is_empty() {
                let empty_message = match app.mode {
                    AppMode::Search => "No notes match your search.",
                    _ => "No documents yet. Press 'i' to create one, or 't' to pick a template.",
                };
                vec![ListItem::new(empty_message).style(Style::default().fg(Color::Gray))]
            } else {
                current_notes
                    .iter()
                    .map(|note| ListItem::new(note.as_str()).style(Style::default().fg(Color::White)))
                    .collect()
            };

            let title = match app.mode {
                AppMode::Normal => format!("Scribe — {} document(s)", current_notes.len()),
                AppMode::Create => format!(
                    "Scribe — New [{}] ({} existing)",
                    app.chosen_template.name(),
                    app.notes.len()
                ),
                AppMode::TemplatePick => "Scribe — Choose a template".to_string(),
                AppMode::Search => {
                    if app.search_query.is_empty() {
                        format!("Search — {} document(s)", app.notes.len())
                    } else {
                        format!(
                            "Search: '{}' — {} match(es)",
                            app.search_query,
                            current_notes.len()
                        )
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
                .highlight_symbol("> ");

            f.render_stateful_widget(list, top[0], &mut app.state);

            // Preview pane.
            if app.preview_open && app.mode == AppMode::Normal && top.len() > 1 {
                let preview_text = app
                    .preview_cache
                    .as_ref()
                    .map(|(_, t)| t.as_str())
                    .unwrap_or("(no selection)");
                let preview = Paragraph::new(preview_text)
                    .block(
                        Block::default()
                            .title("Preview (asciidoctor)")
                            .borders(Borders::ALL)
                            .style(Style::default().fg(Color::DarkGray)),
                    )
                    .wrap(Wrap { trim: false })
                    .style(Style::default().fg(Color::White));
                f.render_widget(preview, top[1]);
            }

            // Bottom overlays.
            if app.mode == AppMode::Create {
                let target_label = if app.create_in_cwd {
                    "in CWD".to_string()
                } else {
                    format!("in {}", app.directory)
                };
                let input_block = Block::default()
                    .title(format!(
                        "New document title ({} template, {}) — Enter to create, Esc to cancel",
                        app.chosen_template.name(),
                        target_label,
                    ))
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Green));
                let input = Paragraph::new(app.input.as_str())
                    .block(input_block)
                    .style(Style::default().fg(Color::White));
                f.render_widget(input, body[1]);
            } else if app.mode == AppMode::Search {
                let search_block = Block::default()
                    .title("Search (Enter to open, Esc to exit)")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Magenta));
                let search_input = Paragraph::new(app.search_query.as_str())
                    .block(search_block)
                    .style(Style::default().fg(Color::White));
                f.render_widget(search_input, body[1]);
            } else if app.mode == AppMode::TemplatePick {
                let items: Vec<ListItem> = Template::ALL
                    .iter()
                    .map(|t| {
                        ListItem::new(format!("{:<10}  {}", t.name(), t.description()))
                            .style(Style::default().fg(Color::White))
                    })
                    .collect();
                let picker_title = if app.create_in_cwd {
                    "Template (target: CWD) — Enter to confirm, Esc to cancel"
                } else {
                    "Template — Enter to confirm, Esc to cancel"
                };
                let list = List::new(items)
                    .block(
                        Block::default()
                            .title(picker_title)
                            .borders(Borders::ALL)
                            .style(Style::default().fg(Color::Cyan)),
                    )
                    .highlight_style(
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol("> ");
                f.render_stateful_widget(list, body[1], &mut app.template_state);
            }

            // Status bar.
            let (default_text, status_color) = match app.mode {
                AppMode::Normal => match app.pending {
                    Pending::Delete => (
                        "Press 'd' again to confirm deletion, or any other key to cancel".to_string(),
                        Color::Red,
                    ),
                    Pending::Export => (
                        "Export: 'h' = HTML, 'p' = PDF, any other key cancels".to_string(),
                        Color::Yellow,
                    ),
                    Pending::None => (
                        "j/k nav | o open | i new | I new-in-cwd | t tpl | T tpl-in-cwd | / search | p preview | e export | dd delete | q quit"
                            .to_string(),
                        Color::Cyan,
                    ),
                },
                AppMode::Create => (
                    "Type title (without .adoc) — Enter to create, Esc to cancel".to_string(),
                    Color::Green,
                ),
                AppMode::TemplatePick => (
                    "j/k choose | Enter confirm | Esc cancel".to_string(),
                    Color::Cyan,
                ),
                AppMode::Search => (
                    "Type to filter | j/k navigate | Enter open | Esc exit".to_string(),
                    Color::Magenta,
                ),
            };

            let status_text = app
                .status_message
                .clone()
                .unwrap_or(default_text);

            let status_bar = Paragraph::new(status_text)
                .block(
                    Block::default()
                        .title("Scribe")
                        .borders(Borders::ALL)
                        .style(Style::default().fg(status_color)),
                )
                .style(Style::default().fg(Color::White));

            f.render_widget(status_bar, main_chunks[1]);
        })?;

        if let Event::Key(key) = event::read()? {
            // Any keypress dismisses a transient status message (except when
            // we're about to set a new one below).
            let had_status = app.status_message.is_some();
            if had_status {
                app.clear_status();
            }

            match app.mode {
                AppMode::Normal => match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('i') => {
                        app.mode = AppMode::Create;
                        app.input.clear();
                        app.create_in_cwd = false;
                        app.pending = Pending::None;
                    }
                    KeyCode::Char('I') => {
                        app.mode = AppMode::Create;
                        app.input.clear();
                        app.create_in_cwd = true;
                        app.pending = Pending::None;
                    }
                    KeyCode::Char('t') => {
                        app.mode = AppMode::TemplatePick;
                        app.template_state.select(Some(0));
                        app.create_in_cwd = false;
                        app.pending = Pending::None;
                    }
                    KeyCode::Char('T') => {
                        app.mode = AppMode::TemplatePick;
                        app.template_state.select(Some(0));
                        app.create_in_cwd = true;
                        app.pending = Pending::None;
                    }
                    KeyCode::Char('/') => {
                        app.mode = AppMode::Search;
                        app.search_query.clear();
                        app.update_filtered_notes();
                        app.pending = Pending::None;
                    }
                    KeyCode::Char('p') if app.pending == Pending::Export => {
                        if let Some(note) = app.selected_note().cloned() {
                            let file_path = Path::new(&app.directory).join(&note);
                            match export(&file_path, "pdf", None) {
                                Ok(out) => app.set_status(format!("Exported PDF -> {}", out.display())),
                                Err(e) => app.set_status(format!("Export failed: {e}")),
                            }
                        }
                        app.pending = Pending::None;
                    }
                    KeyCode::Char('p') => {
                        app.preview_open = !app.preview_open;
                        app.pending = Pending::None;
                    }
                    KeyCode::Char('o') => {
                        if let Some(note) = app.selected_note().cloned() {
                            let file_path = Path::new(&app.directory).join(&note);
                            open_in_editor(&mut terminal, &file_path)?;
                            app.preview_cache = None;
                        }
                        app.pending = Pending::None;
                    }
                    KeyCode::Char('e') => {
                        if app.pending == Pending::Export {
                            app.pending = Pending::None;
                        } else {
                            app.pending = Pending::Export;
                        }
                    }
                    KeyCode::Char('h') if app.pending == Pending::Export => {
                        if let Some(note) = app.selected_note().cloned() {
                            let file_path = Path::new(&app.directory).join(&note);
                            match export(&file_path, "html", None) {
                                Ok(out) => app.set_status(format!("Exported HTML -> {}", out.display())),
                                Err(e) => app.set_status(format!("Export failed: {e}")),
                            }
                        }
                        app.pending = Pending::None;
                    }
                    KeyCode::Char('d') => {
                        if app.pending == Pending::Delete {
                            if let Some(note) = app.selected_note().cloned() {
                                let file_path = Path::new(&app.directory).join(&note);
                                match delete_note(file_path.to_str().unwrap()) {
                                    Ok(_) => {
                                        app.refresh_notes();
                                        app.set_status(format!("Deleted {note}"));
                                    }
                                    Err(e) => app.set_status(format!("Delete failed: {e}")),
                                }
                            }
                            app.pending = Pending::None;
                        } else {
                            app.pending = Pending::Delete;
                        }
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        let selected = app.state.selected().unwrap_or(0);
                        let count = app.get_current_notes().len();
                        if count > 0 && selected + 1 < count {
                            app.state.select(Some(selected + 1));
                            app.preview_cache = None;
                        }
                        app.pending = Pending::None;
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        let selected = app.state.selected().unwrap_or(0);
                        if selected > 0 {
                            app.state.select(Some(selected - 1));
                            app.preview_cache = None;
                        }
                        app.pending = Pending::None;
                    }
                    _ => {
                        app.pending = Pending::None;
                    }
                },
                AppMode::Create => match key.code {
                    KeyCode::Enter if !app.input.is_empty() => {
                        let filename = ensure_adoc_extension(&app.input);
                        let title = strip_adoc_extension(&app.input).to_string();
                        let target_dir = if app.create_in_cwd {
                            std::env::current_dir().ok()
                        } else {
                            Some(std::path::PathBuf::from(&app.directory))
                        };
                        let file_path = match target_dir {
                            Some(d) => d.join(&filename),
                            None => Path::new(&app.directory).join(&filename),
                        };
                        let contents = app.chosen_template.render(&title);
                        match create_new_note(file_path.to_str().unwrap(), &contents) {
                            Ok(_) => {
                                if !app.create_in_cwd {
                                    app.refresh_notes();
                                }
                                app.set_status(format!("Created {}", file_path.display()));
                            }
                            Err(e) => app.set_status(format!("Create failed: {e}")),
                        }
                        app.input.clear();
                        app.create_in_cwd = false;
                        app.mode = AppMode::Normal;
                    }
                    KeyCode::Enter => {}
                    KeyCode::Esc => {
                        app.input.clear();
                        app.create_in_cwd = false;
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
                AppMode::TemplatePick => match key.code {
                    KeyCode::Esc => {
                        app.create_in_cwd = false;
                        app.mode = AppMode::Normal;
                    }
                    KeyCode::Enter => {
                        let idx = app.template_state.selected().unwrap_or(0);
                        if let Some(t) = Template::ALL.get(idx) {
                            app.chosen_template = *t;
                        }
                        app.input.clear();
                        app.mode = AppMode::Create;
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        let selected = app.template_state.selected().unwrap_or(0);
                        if selected + 1 < Template::ALL.len() {
                            app.template_state.select(Some(selected + 1));
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        let selected = app.template_state.selected().unwrap_or(0);
                        if selected > 0 {
                            app.template_state.select(Some(selected - 1));
                        }
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
                        if let Some(note) = app.selected_note().cloned() {
                            let file_path = Path::new(&app.directory).join(&note);
                            open_in_editor(&mut terminal, &file_path)?;
                            app.mode = AppMode::Normal;
                            app.search_query.clear();
                            app.update_filtered_notes();
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
                        if selected + 1 < app.filtered_notes.len() {
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
