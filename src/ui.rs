use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::Backend,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};
use std::{env, io, path::Path, path::PathBuf, process::Command, time::Duration};

use crate::app::{App, AppMode, DirPickFocus, Focus, Pending};
use crate::notes::{
    create_new_note, ensure_adoc_extension, fmt_mtime, fmt_size, rename_note, restore_trashed,
    soft_delete_note, strip_adoc_extension,
};
use crate::render::{export, render_preview};
use crate::templates::user_templates_dir;

/// Get the appropriate text editor for the current platform.
fn get_editor() -> String {
    if let Ok(editor) = env::var("EDITOR") {
        return editor;
    }
    #[cfg(target_os = "windows")]
    {
        return "notepad".to_string();
    }
    #[cfg(not(target_os = "windows"))]
    {
        "nvim".to_string()
    }
}

fn open_in_editor<B: Backend + std::io::Write>(
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
    let Some(name) = app.selected_rel_path().map(|s| s.to_string()) else {
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
        Err(e) => format!(
            "[preview unavailable]\n\n{e}\n\nInstall `asciidoctor` to enable previews."
        ),
    };
    app.preview_cache = Some((name, text));
}

/// Format a single document row for the list view.
fn format_row(entry: &crate::notes::NoteEntry, width: u16) -> Line<'static> {
    let name = entry.rel_path.clone();
    let mtime = entry
        .mtime
        .map(fmt_mtime)
        .unwrap_or_else(|| "—".to_string());
    let size = fmt_size(entry.size);
    let meta = format!("  {mtime}  {size:>6}");

    // Truncate name if needed so the meta stays right-aligned.
    let total_w = width as usize;
    let meta_w = meta.chars().count();
    let max_name = total_w.saturating_sub(meta_w + 2);
    let name_disp = if name.chars().count() > max_name && max_name > 1 {
        let mut t: String = name.chars().take(max_name.saturating_sub(1)).collect();
        t.push('…');
        t
    } else {
        name
    };
    let pad = total_w
        .saturating_sub(name_disp.chars().count())
        .saturating_sub(meta_w);
    Line::from(vec![
        Span::styled(name_disp, Style::default().fg(Color::White)),
        Span::raw(" ".repeat(pad)),
        Span::styled(meta, Style::default().fg(Color::DarkGray)),
    ])
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

        terminal.draw(|f| draw_frame(f, &mut app))?;

        // Debounced event read so the UI stays responsive but doesn't busy-poll.
        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        let evt = event::read()?;
        let Event::Key(key) = evt else {
            continue;
        };
        if handle_key(&mut terminal, &mut app, key)? {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    Ok(())
}

fn draw_frame(f: &mut ratatui::Frame, app: &mut App) {
    let area = f.area();

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
        .split(area);

    // Bottom overlay split when in input-y modes.
    let bottom_overlay = matches!(
        app.mode,
        AppMode::Create | AppMode::Rename | AppMode::Search | AppMode::TemplatePick
    );
    let body = if bottom_overlay {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(7)].as_ref())
            .split(main_chunks[0])
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0)].as_ref())
            .split(main_chunks[0])
    };

    // Horizontal split if preview is open.
    let top = if app.preview_open && app.mode == AppMode::Normal {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(45), Constraint::Percentage(55)].as_ref())
            .split(body[0])
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)].as_ref())
            .split(body[0])
    };

    draw_list(f, app, top[0]);
    if app.preview_open && app.mode == AppMode::Normal && top.len() > 1 {
        draw_preview(f, app, top[1]);
    }

    if bottom_overlay {
        draw_bottom_overlay(f, app, body[1]);
    }

    draw_status_bar(f, app, main_chunks[1]);

    // Floating overlays.
    if app.mode == AppMode::Help {
        draw_help_overlay(f, area);
    } else if app.mode == AppMode::DirPick {
        draw_dir_picker(f, app, area);
    }
}

fn draw_list(f: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let inner_w = area.width.saturating_sub(2); // account for borders
    let items: Vec<ListItem> = if app.filtered_indices.is_empty() {
        let msg = if !app.search_query.is_empty() {
            format!(
                "No notes match '{}'. Ctrl-N to create a new note with this title.",
                app.search_query
            )
        } else if app.notes.is_empty() {
            "No documents yet. Press 'i' to create one, or 't' to pick a template.".to_string()
        } else {
            "(filtered list is empty)".to_string()
        };
        vec![ListItem::new(msg).style(Style::default().fg(Color::Gray))]
    } else {
        app.filtered_indices
            .iter()
            .filter_map(|&i| app.notes.get(i))
            .map(|note| ListItem::new(format_row(note, inner_w)))
            .collect()
    };

    let target_hint = if app.target_cwd { "CWD" } else { "notes" };
    let recursive_hint = if app.recursive { " R" } else { "" };
    let title = match app.mode {
        AppMode::Normal => format!(
            "Scribe — {} doc(s) | dir: {} | target: {} | sort: {}{}",
            app.list_len(),
            app.directory,
            target_hint,
            app.sort.label(),
            recursive_hint,
        ),
        AppMode::Create => format!(
            "New [{}] in {} ({} existing)",
            app.chosen_template.name(),
            app.target_label(),
            app.notes.len()
        ),
        AppMode::Rename => "Renaming…".to_string(),
        AppMode::TemplatePick => format!("Choose a template (target: {})", app.target_label()),
        AppMode::Search => {
            if app.search_query.is_empty() {
                format!("Search — {} document(s)", app.notes.len())
            } else {
                format!(
                    "Search: '{}' — {} match(es)",
                    app.search_query,
                    app.list_len()
                )
            }
        }
        AppMode::Help => "Help".to_string(),
        AppMode::DirPick => "Directory picker".to_string(),
    };

    let border_color = if app.focus == Focus::List {
        Color::Blue
    } else {
        Color::DarkGray
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .style(Style::default().fg(border_color)),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    f.render_stateful_widget(list, area, &mut app.state);
}

fn draw_preview(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let preview_text = app
        .preview_cache
        .as_ref()
        .map(|(_, t)| t.as_str())
        .unwrap_or("(no selection)");
    let border = if app.focus == Focus::Preview {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let title = if app.focus == Focus::Preview {
        "Preview (Tab to return — j/k scroll, g/G top/bottom)"
    } else {
        "Preview (Tab to focus)"
    };
    let preview = Paragraph::new(preview_text)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .style(Style::default().fg(border)),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.preview_scroll, 0))
        .style(Style::default().fg(Color::White));
    f.render_widget(preview, area);
}

fn draw_bottom_overlay(f: &mut ratatui::Frame, app: &mut App, area: Rect) {
    match app.mode {
        AppMode::Create => {
            let block = Block::default()
                .title(format!(
                    "New title  [{} template, {}] — Enter to create, Tab toggles target, Esc cancels",
                    app.chosen_template.name(),
                    app.target_label(),
                ))
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Green));
            let input = Paragraph::new(app.input.as_str())
                .block(block)
                .style(Style::default().fg(Color::White));
            f.render_widget(input, area);
        }
        AppMode::Rename => {
            let from = app
                .rename_from
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "?".to_string());
            let block = Block::default()
                .title(format!(
                    "Rename {from} — Enter to confirm, Esc to cancel"
                ))
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Magenta));
            let input = Paragraph::new(app.input.as_str())
                .block(block)
                .style(Style::default().fg(Color::White));
            f.render_widget(input, area);
        }
        AppMode::Search => {
            let block = Block::default()
                .title("Search — Enter open, Ctrl-N create, n/N persist, Esc clear")
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Magenta));
            let input = Paragraph::new(app.search_query.as_str())
                .block(block)
                .style(Style::default().fg(Color::White));
            f.render_widget(input, area);
        }
        AppMode::TemplatePick => {
            let items: Vec<ListItem> = app
                .templates
                .iter()
                .map(|t| {
                    ListItem::new(format!("{:<14}  {}", t.name(), t.description()))
                        .style(Style::default().fg(Color::White))
                })
                .collect();
            let dir_hint = user_templates_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "(no config dir)".to_string());
            let title = format!(
                "Template (target: {}) — Enter confirm, Esc cancel — user dir: {}",
                app.target_label(),
                dir_hint
            );
            let list = List::new(items)
                .block(
                    Block::default()
                        .title(title)
                        .borders(Borders::ALL)
                        .style(Style::default().fg(Color::Cyan)),
                )
                .highlight_style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("> ");
            f.render_stateful_widget(list, area, &mut app.template_state);
        }
        _ => {}
    }
}

fn draw_status_bar(f: &mut ratatui::Frame, app: &App, area: Rect) {
    let (default_text, status_color) = match app.mode {
        AppMode::Normal => match app.pending {
            Pending::Delete => (
                "Press 'd' again to confirm deletion (soft-delete, `u` to undo)".to_string(),
                Color::Red,
            ),
            Pending::Export => (
                "Export: 'h' = HTML, 'p' = PDF, any other key cancels".to_string(),
                Color::Yellow,
            ),
            Pending::None => (
                "j/k nav | o open | i new | r rename | t tpl | / search | Tab target | R recursive | s sort | c chdir | p preview | e export | dd delete | u undo | ? help | q quit".to_string(),
                Color::Cyan,
            ),
        },
        AppMode::Create => (
            "Type title (without .adoc) — Enter to create, Tab toggle target, Esc cancel".to_string(),
            Color::Green,
        ),
        AppMode::Rename => (
            "Type new path/name — Enter to confirm, Esc cancel".to_string(),
            Color::Magenta,
        ),
        AppMode::TemplatePick => (
            "j/k choose | Enter confirm | Tab toggle target | Esc cancel".to_string(),
            Color::Cyan,
        ),
        AppMode::Search => (
            "Type to filter | j/k navigate | Enter open | Ctrl-N create from query | Esc clear".to_string(),
            Color::Magenta,
        ),
        AppMode::Help => ("? or Esc to close help".to_string(), Color::Cyan),
        AppMode::DirPick => (
            match app.dir_pick_focus {
                DirPickFocus::Recents => "Recents: j/k navigate | Enter switch | Tab → input | Esc cancel".to_string(),
                DirPickFocus::NewPath => "New path: type a directory | Enter switch | Tab → list | Esc cancel".to_string(),
            },
            Color::Cyan,
        ),
    };

    let (text, color) = match &app.status_message {
        Some(s) => (
            s.text.clone(),
            if s.is_error { Color::Red } else { status_color },
        ),
        None => (default_text, status_color),
    };

    let status_bar = Paragraph::new(text)
        .block(
            Block::default()
                .title("Scribe")
                .borders(Borders::ALL)
                .style(Style::default().fg(color)),
        )
        .style(Style::default().fg(Color::White));

    f.render_widget(status_bar, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn draw_help_overlay(f: &mut ratatui::Frame, area: Rect) {
    let area = centered_rect(70, 80, area);
    f.render_widget(Clear, area);
    let body = r#"
Scribe keybindings
==================

NAVIGATION
  j / Down      Next document
  k / Up        Previous document
  g / G         Top / bottom
  Tab           Toggle target (notes dir <-> CWD)  -- in Normal: also toggles list/preview focus when preview is open
  /             Open fuzzy search (filename + body)
  n / N         Re-enter last search (kept across modes)
  s             Cycle sort: name -> mtime↓ -> mtime↑ -> size↓
  R             Toggle recursive listing of subdirectories
  c             Open directory picker (recent dirs + free entry)
  H             Jump to current working directory

DOCUMENT ACTIONS
  o             Open in $EDITOR
  i             New document (uses current template, target indicator)
  t             Choose template, then create
  r             Rename / move selected document
  d d           Soft-delete (moves to .scribe-trash)
  u             Undo last delete
  e h / e p     Export to HTML / PDF
  p             Toggle preview pane (when focused: j/k scroll, g/G ends)

GLOBAL
  ?             This help
  q             Quit

Search mode
  Type to filter (filename + first 4KB body fuzzy match)
  Enter           Open highlighted match
  Ctrl-N          Create new note titled after the current query
  Esc             Clear query and exit search

Templates
  Built-ins: blank, readme, journal
  User templates: drop *.adoc files in $XDG_CONFIG_HOME/scribe/templates
                   (or ~/.config/scribe/templates)
  Substitutions: {{title}}, {{author}}, {{date}}
"#;
    let p = Paragraph::new(body.trim_start())
        .block(
            Block::default()
                .title("Help — ? or Esc to close")
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(Color::White));
    f.render_widget(p, area);
}

fn draw_dir_picker(f: &mut ratatui::Frame, app: &mut App, area: Rect) {
    let area = centered_rect(70, 60, area);
    f.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)].as_ref())
        .split(area);

    let recents_focused = app.dir_pick_focus == DirPickFocus::Recents;
    let newpath_focused = app.dir_pick_focus == DirPickFocus::NewPath;

    let recents_border = if recents_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };
    let newpath_border = if newpath_focused {
        Color::Green
    } else {
        Color::DarkGray
    };

    let recents_title = if recents_focused {
        "Recent directories — j/k navigate, Enter switch, Tab → input, Esc cancel"
    } else {
        "Recent directories (Tab to focus)"
    };
    let newpath_title = if newpath_focused {
        "New directory — type a path, Enter switch, Tab → list, Esc cancel"
    } else {
        "New directory (Tab to focus)"
    };

    let items: Vec<ListItem> = app
        .recent_dirs
        .iter()
        .map(|d| ListItem::new(d.clone()).style(Style::default().fg(Color::White)))
        .collect();
    let list = List::new(items)
        .block(
            Block::default()
                .title(recents_title)
                .borders(Borders::ALL)
                .style(Style::default().fg(recents_border)),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");
    f.render_stateful_widget(list, chunks[0], &mut app.recent_state);

    let input = Paragraph::new(app.input.as_str())
        .block(
            Block::default()
                .title(newpath_title)
                .borders(Borders::ALL)
                .style(Style::default().fg(newpath_border)),
        )
        .style(Style::default().fg(Color::White));
    f.render_widget(input, chunks[1]);
}

/// Returns `true` to signal the main loop should exit.
fn handle_key<B: Backend + std::io::Write>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    key: KeyEvent,
) -> io::Result<bool> {
    // Status messages dismiss on any keypress that ISN'T just `?` opening help
    // — we clear after handling to avoid wiping freshly-set messages.
    let prev_status = app.status_message.clone();

    let result = match app.mode {
        AppMode::Normal => handle_normal(terminal, app, key)?,
        AppMode::Create => handle_create(app, key)?,
        AppMode::Rename => handle_rename(app, key)?,
        AppMode::TemplatePick => handle_template_pick(app, key)?,
        AppMode::Search => handle_search(terminal, app, key)?,
        AppMode::Help => {
            match key.code {
                KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q') => {
                    app.mode = AppMode::Normal;
                }
                _ => {}
            }
            false
        }
        AppMode::DirPick => handle_dir_pick(app, key)?,
    };

    // Auto-dismiss only if the status didn't change this tick.
    if let (Some(prev), Some(cur)) = (&prev_status, &app.status_message) {
        if prev.text == cur.text && !matches!(key.code, KeyCode::Char('?')) {
            // keep sticky — only clear on explicit Esc in Normal mode
            if app.mode == AppMode::Normal && matches!(key.code, KeyCode::Esc) {
                app.clear_status();
            }
        }
    }

    Ok(result)
}

fn handle_normal<B: Backend + std::io::Write>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    key: KeyEvent,
) -> io::Result<bool> {
    // Resolve `e` chord branches first
    if app.pending == Pending::Export {
        match key.code {
            KeyCode::Char('h') => {
                if let Some(path) = app.selected_path() {
                    match export(&path, "html", None) {
                        Ok(out) => app.set_status(format!("Exported HTML -> {}", out.display())),
                        Err(e) => app.set_error(format!("Export failed: {e}")),
                    }
                }
                app.pending = Pending::None;
                return Ok(false);
            }
            KeyCode::Char('p') => {
                if let Some(path) = app.selected_path() {
                    match export(&path, "pdf", None) {
                        Ok(out) => app.set_status(format!("Exported PDF -> {}", out.display())),
                        Err(e) => app.set_error(format!("Export failed: {e}")),
                    }
                }
                app.pending = Pending::None;
                return Ok(false);
            }
            _ => {
                app.pending = Pending::None;
                // fall through to general handling
            }
        }
    }

    if app.pending == Pending::Delete {
        if key.code == KeyCode::Char('d') {
            if let Some(note) = app.selected_note().cloned() {
                let path = Path::new(&app.directory).join(&note.rel_path);
                match soft_delete_note(&app.directory, &path) {
                    Ok(trashed) => {
                        app.last_trash = Some(crate::app::TrashRecord {
                            trashed_path: trashed,
                            original_path: path,
                            display_name: note.rel_path.clone(),
                        });
                        app.refresh_notes();
                        app.set_status(format!(
                            "Deleted {} — press 'u' to undo",
                            note.rel_path
                        ));
                    }
                    Err(e) => app.set_error(format!("Delete failed: {e}")),
                }
            }
            app.pending = Pending::None;
            return Ok(false);
        } else {
            app.pending = Pending::None;
            // fall through
        }
    }

    match key.code {
        KeyCode::Char('q') => return Ok(true),
        KeyCode::Char('?') => app.mode = AppMode::Help,
        KeyCode::Char('i') => {
            app.mode = AppMode::Create;
            app.input.clear();
        }
        KeyCode::Char('I') => {
            // Legacy alias: capital-I forces CWD on this single action.
            app.mode = AppMode::Create;
            app.input.clear();
            app.target_cwd = true;
        }
        KeyCode::Char('t') => {
            app.mode = AppMode::TemplatePick;
            app.template_state.select(Some(0));
        }
        KeyCode::Char('T') => {
            app.mode = AppMode::TemplatePick;
            app.template_state.select(Some(0));
            app.target_cwd = true;
        }
        KeyCode::Char('r') => {
            if let Some(path) = app.selected_path() {
                app.rename_from = Some(path.clone());
                app.input = app
                    .selected_rel_path()
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                app.mode = AppMode::Rename;
            } else {
                app.set_error("Nothing to rename");
            }
        }
        KeyCode::Char('/') => {
            app.mode = AppMode::Search;
            // keep existing query so `/` followed by typing extends; clear only on Esc.
            app.recompute_filter(true);
        }
        KeyCode::Char('n') => {
            // re-enter search keeping current query
            if !app.search_query.is_empty() {
                app.mode = AppMode::Search;
            } else {
                app.set_status("No previous search");
            }
        }
        KeyCode::Char('N') => {
            if !app.search_query.is_empty() {
                app.search_query.clear();
                app.recompute_filter(false);
                app.set_status("Search cleared");
            }
        }
        KeyCode::Char('s') => {
            app.cycle_sort();
            app.set_status(format!("Sort: {}", app.sort.label()));
        }
        KeyCode::Char('R') => {
            app.toggle_recursive();
            app.set_status(if app.recursive {
                "Recursive listing ON"
            } else {
                "Recursive listing OFF"
            });
        }
        KeyCode::Char('c') => {
            app.mode = AppMode::DirPick;
            app.input.clear();
            app.recent_state.select(Some(0));
            app.dir_pick_focus = DirPickFocus::Recents;
        }
        KeyCode::Char('H') => {
            if let Ok(cwd) = std::env::current_dir() {
                if let Some(s) = cwd.to_str() {
                    let s = s.to_string();
                    app.change_directory(&s);
                    app.set_status(format!("Switched to {s}"));
                }
            }
        }
        KeyCode::Tab => {
            if app.preview_open {
                app.focus = match app.focus {
                    Focus::List => Focus::Preview,
                    Focus::Preview => Focus::List,
                };
            } else {
                app.toggle_target();
                app.set_status(format!("Target: {}", app.target_label()));
            }
        }
        KeyCode::BackTab => {
            // Shift-Tab always toggles target (even with preview focus active).
            app.toggle_target();
            app.set_status(format!("Target: {}", app.target_label()));
        }
        KeyCode::Char('p') => {
            app.preview_open = !app.preview_open;
            if !app.preview_open {
                app.focus = Focus::List;
            }
        }
        KeyCode::Char('o') => {
            if let Some(path) = app.selected_path() {
                open_in_editor(terminal, &path)?;
                app.preview_cache = None;
            }
        }
        KeyCode::Char('e') => {
            app.pending = Pending::Export;
        }
        KeyCode::Char('d') => {
            app.pending = Pending::Delete;
        }
        KeyCode::Char('u') => {
            if let Some(rec) = app.last_trash.clone() {
                match restore_trashed(&rec.trashed_path, &rec.original_path) {
                    Ok(_) => {
                        app.last_trash = None;
                        app.refresh_notes();
                        app.set_status(format!("Restored {}", rec.display_name));
                    }
                    Err(e) => app.set_error(format!("Undo failed: {e}")),
                }
            } else {
                app.set_status("Nothing to undo");
            }
        }
        KeyCode::Char('j') | KeyCode::Down => {
            if app.focus == Focus::Preview {
                app.preview_scroll = app.preview_scroll.saturating_add(1);
            } else {
                app.move_selection(1);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if app.focus == Focus::Preview {
                app.preview_scroll = app.preview_scroll.saturating_sub(1);
            } else {
                app.move_selection(-1);
            }
        }
        KeyCode::Char('g') => {
            if app.focus == Focus::Preview {
                app.preview_scroll = 0;
            } else if app.list_len() > 0 {
                app.state.select(Some(0));
                app.preview_cache = None;
            }
        }
        KeyCode::Char('G') => {
            if app.focus == Focus::Preview {
                app.preview_scroll = u16::MAX / 2;
            } else if app.list_len() > 0 {
                app.state.select(Some(app.list_len() - 1));
                app.preview_cache = None;
            }
        }
        KeyCode::PageDown => {
            if app.focus == Focus::Preview {
                app.preview_scroll = app.preview_scroll.saturating_add(10);
            } else {
                app.move_selection(10);
            }
        }
        KeyCode::PageUp => {
            if app.focus == Focus::Preview {
                app.preview_scroll = app.preview_scroll.saturating_sub(10);
            } else {
                app.move_selection(-10);
            }
        }
        KeyCode::Esc => {
            app.clear_status();
        }
        _ => {}
    }
    Ok(false)
}

fn create_note_with_template(
    app: &mut App,
    target_dir: &Path,
    title_raw: &str,
) -> io::Result<PathBuf> {
    let filename = ensure_adoc_extension(title_raw);
    let bare_title = strip_adoc_extension(title_raw).to_string();
    let file_path = target_dir.join(&filename);
    let contents = app.chosen_template.render(&bare_title);
    create_new_note(&file_path, &contents)?;
    Ok(file_path)
}

fn handle_create(app: &mut App, key: KeyEvent) -> io::Result<bool> {
    match key.code {
        KeyCode::Enter if !app.input.is_empty() => {
            let target = app.target_dir();
            let input = app.input.clone();
            match create_note_with_template(app, &target, &input) {
                Ok(path) => {
                    let target_is_root = target == Path::new(&app.directory);
                    if target_is_root {
                        app.refresh_notes();
                    }
                    app.set_status(format!("Created {}", path.display()));
                }
                Err(e) => app.set_error(format!("Create failed: {e}")),
            }
            app.input.clear();
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {}
        KeyCode::Esc => {
            app.input.clear();
            app.mode = AppMode::Normal;
        }
        KeyCode::Tab => {
            app.toggle_target();
        }
        KeyCode::Char(c) => app.input.push(c),
        KeyCode::Backspace => {
            app.input.pop();
        }
        _ => {}
    }
    Ok(false)
}

fn handle_rename(app: &mut App, key: KeyEvent) -> io::Result<bool> {
    match key.code {
        KeyCode::Enter if !app.input.is_empty() => {
            if let Some(from) = app.rename_from.clone() {
                let new_rel = ensure_adoc_extension(&app.input);
                let to = Path::new(&app.directory).join(&new_rel);
                match rename_note(&from, &to) {
                    Ok(_) => {
                        app.refresh_notes();
                        app.set_status(format!("Renamed to {new_rel}"));
                    }
                    Err(e) => app.set_error(format!("Rename failed: {e}")),
                }
            }
            app.input.clear();
            app.rename_from = None;
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {}
        KeyCode::Esc => {
            app.input.clear();
            app.rename_from = None;
            app.mode = AppMode::Normal;
        }
        KeyCode::Char(c) => app.input.push(c),
        KeyCode::Backspace => {
            app.input.pop();
        }
        _ => {}
    }
    Ok(false)
}

fn handle_template_pick(app: &mut App, key: KeyEvent) -> io::Result<bool> {
    match key.code {
        KeyCode::Esc => {
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {
            let idx = app.template_state.selected().unwrap_or(0);
            if let Some(t) = app.templates.get(idx).cloned() {
                app.chosen_template = t;
            }
            app.input.clear();
            app.mode = AppMode::Create;
        }
        KeyCode::Tab => {
            app.toggle_target();
        }
        KeyCode::Char('j') | KeyCode::Down => {
            let selected = app.template_state.selected().unwrap_or(0);
            if selected + 1 < app.templates.len() {
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
    }
    Ok(false)
}

fn handle_search<B: Backend + std::io::Write>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    key: KeyEvent,
) -> io::Result<bool> {
    // Ctrl-N: create a new note titled after the current query.
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('n') {
        if !app.search_query.is_empty() {
            let target = app.target_dir();
            let q = app.search_query.clone();
            match create_note_with_template(app, &target, &q) {
                Ok(path) => {
                    app.search_query.clear();
                    app.mode = AppMode::Normal;
                    app.refresh_notes();
                    app.set_status(format!("Created {}", path.display()));
                    // open it
                    open_in_editor(terminal, &path)?;
                    app.preview_cache = None;
                }
                Err(e) => app.set_error(format!("Create failed: {e}")),
            }
        }
        return Ok(false);
    }

    match key.code {
        KeyCode::Esc => {
            app.search_query.clear();
            app.recompute_filter(false);
            app.mode = AppMode::Normal;
        }
        KeyCode::Enter => {
            if let Some(path) = app.selected_path() {
                open_in_editor(terminal, &path)?;
                app.mode = AppMode::Normal;
                app.preview_cache = None;
            }
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            app.recompute_filter(true);
        }
        KeyCode::Up => app.move_selection(-1),
        KeyCode::Down => app.move_selection(1),
        KeyCode::Char(c) => {
            app.search_query.push(c);
            app.recompute_filter(true);
        }
        _ => {}
    }
    Ok(false)
}

fn handle_dir_pick(app: &mut App, key: KeyEvent) -> io::Result<bool> {
    // Tab / Shift-Tab always toggles focus between the two sub-panes.
    if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
        app.dir_pick_focus = match app.dir_pick_focus {
            DirPickFocus::Recents => DirPickFocus::NewPath,
            DirPickFocus::NewPath => DirPickFocus::Recents,
        };
        return Ok(false);
    }

    // Esc closes from either pane.
    if key.code == KeyCode::Esc {
        app.input.clear();
        app.mode = AppMode::Normal;
        return Ok(false);
    }

    match app.dir_pick_focus {
        DirPickFocus::Recents => match key.code {
            KeyCode::Enter => {
                let idx = app.recent_state.selected().unwrap_or(0);
                let target = app.recent_dirs.get(idx).cloned().unwrap_or_default();
                if target.is_empty() {
                    app.set_error("No directory selected");
                } else if !Path::new(&target).is_dir() {
                    app.set_error(format!("Not a directory: {target}"));
                } else {
                    app.input.clear();
                    app.mode = AppMode::Normal;
                    app.change_directory(&target);
                    app.set_status(format!("Switched to {target}"));
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let sel = app.recent_state.selected().unwrap_or(0);
                if sel + 1 < app.recent_dirs.len() {
                    app.recent_state.select(Some(sel + 1));
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let sel = app.recent_state.selected().unwrap_or(0);
                if sel > 0 {
                    app.recent_state.select(Some(sel - 1));
                }
            }
            KeyCode::Char('g') => {
                if !app.recent_dirs.is_empty() {
                    app.recent_state.select(Some(0));
                }
            }
            KeyCode::Char('G') => {
                if !app.recent_dirs.is_empty() {
                    app.recent_state.select(Some(app.recent_dirs.len() - 1));
                }
            }
            _ => {}
        },
        DirPickFocus::NewPath => match key.code {
            KeyCode::Enter => {
                if app.input.is_empty() {
                    app.set_error("Type a path or Tab back to the list");
                } else {
                    let target = expand_path(&app.input);
                    if !Path::new(&target).is_dir() {
                        app.set_error(format!("Not a directory: {target}"));
                    } else {
                        app.input.clear();
                        app.mode = AppMode::Normal;
                        app.change_directory(&target);
                        app.set_status(format!("Switched to {target}"));
                    }
                }
            }
            KeyCode::Char(c) => app.input.push(c),
            KeyCode::Backspace => {
                app.input.pop();
            }
            _ => {}
        },
    }
    Ok(false)
}

fn expand_path(raw: &str) -> String {
    if let Some(stripped) = raw.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped).to_string_lossy().into_owned();
        }
    }
    if raw == "~" {
        if let Some(home) = dirs::home_dir() {
            return home.to_string_lossy().into_owned();
        }
    }
    raw.to_string()
}
