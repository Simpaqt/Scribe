use crate::notes::{NoteEntry, SortMode, list_notes, read_head_lossy};
use crate::templates::{Template, load_all};
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use ratatui::widgets::ListState;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(PartialEq, Clone, Copy)]
pub enum AppMode {
    Normal,
    Create,
    Rename,
    TemplatePick,
    Search,
    Help,
    DirPick,
}

/// Pending multi-key chord (e.g. `dd`, `eh`, `ep`).
#[derive(PartialEq, Clone, Copy)]
pub enum Pending {
    None,
    Delete,
    Export,
}

/// Which pane has focus when both list + preview are visible.
#[derive(PartialEq, Clone, Copy)]
pub enum Focus {
    List,
    Preview,
}

/// Which sub-pane has focus inside the directory-picker overlay.
#[derive(PartialEq, Clone, Copy)]
pub enum DirPickFocus {
    Recents,
    NewPath,
}

/// Bounded ring of recent status events so the user can scroll back with `?`.
#[derive(Clone, Debug)]
pub struct StatusEntry {
    pub text: String,
    pub is_error: bool,
}

/// Most recent soft-deleted note, kept so `u` can undo.
#[derive(Clone, Debug)]
pub struct TrashRecord {
    pub trashed_path: PathBuf,
    pub original_path: PathBuf,
    pub display_name: String,
}

pub struct App {
    /// Current root directory whose contents are listed.
    pub directory: String,
    /// Saved when we entered the TUI; used as the "configured notes dir"
    /// target regardless of how the user navigates.
    #[allow(dead_code)]
    pub notes_root: String,
    /// Recent directories — populated by the dir picker.
    pub recent_dirs: Vec<String>,
    pub recent_state: ListState,

    pub notes: Vec<NoteEntry>,
    pub filtered_indices: Vec<usize>,
    pub state: ListState,

    pub input: String,
    pub mode: AppMode,
    pub pending: Pending,
    pub search_query: String,
    pub show_all: bool,
    pub recursive: bool,
    pub sort: SortMode,

    pub preview_open: bool,
    pub preview_cache: Option<(String, String)>,
    pub preview_scroll: u16,
    pub focus: Focus,

    /// History of status messages (most recent first).
    pub status_log: Vec<StatusEntry>,
    /// Currently displayed status — sticky until next state change or dismissal.
    pub status_message: Option<StatusEntry>,

    pub template_state: ListState,
    pub templates: Vec<Template>,
    pub chosen_template: Template,

    /// Last soft-deleted note (for `u` to restore).
    pub last_trash: Option<TrashRecord>,

    /// Rename source path (when in Rename mode).
    pub rename_from: Option<PathBuf>,

    /// Which sub-pane of the directory picker has focus.
    pub dir_pick_focus: DirPickFocus,

    /// Cached file heads keyed by relative path; rebuilt on refresh.
    body_index: HashMap<String, String>,

    matcher: SkimMatcherV2,
}

impl App {
    pub fn new(directory: &str, show_all: bool) -> App {
        let templates = load_all();
        let chosen_template = templates
            .first()
            .cloned()
            .unwrap_or_else(|| Template::from_name("blank").expect("blank template"));
        let mut template_state = ListState::default();
        template_state.select(Some(0));

        let recent_dirs = seed_recent_dirs(directory);

        let mut app = App {
            directory: directory.to_string(),
            notes_root: directory.to_string(),
            recent_dirs,
            recent_state: {
                let mut s = ListState::default();
                s.select(Some(0));
                s
            },

            notes: Vec::new(),
            filtered_indices: Vec::new(),
            state: ListState::default(),

            input: String::new(),
            mode: AppMode::Normal,
            pending: Pending::None,
            search_query: String::new(),
            show_all,
            recursive: false,
            sort: SortMode::NameAsc,

            preview_open: false,
            preview_cache: None,
            preview_scroll: 0,
            focus: Focus::List,

            status_log: Vec::new(),
            status_message: None,

            template_state,
            templates,
            chosen_template,

            last_trash: None,
            rename_from: None,
            dir_pick_focus: DirPickFocus::Recents,
            body_index: HashMap::new(),
            matcher: SkimMatcherV2::default(),
        };
        app.refresh_notes();
        app
    }

    /// Re-read the directory listing and rebuild the search index.
    pub fn refresh_notes(&mut self) {
        let previous = self.selected_rel_path().map(str::to_owned);
        self.notes =
            list_notes(&self.directory, !self.show_all, self.recursive, self.sort).unwrap_or_default();
        self.body_index.clear();
        for note in &self.notes {
            let path = Path::new(&self.directory).join(&note.rel_path);
            if let Ok(head) = read_head_lossy(&path, 4096) {
                self.body_index.insert(note.rel_path.clone(), head);
            }
        }
        self.recompute_filter(false);
        if let Some(previous) = previous {
            if let Some(pos) = self
                .filtered_indices
                .iter()
                .position(|&i| self.notes[i].rel_path == previous)
            {
                self.state.select(Some(pos));
            }

        self.preview_cache = None;
    }

    /// Recompute `filtered_indices` from `search_query`. If `preserve_selection`
    /// is true, try to keep the currently selected note highlighted.
    pub fn recompute_filter(&mut self, preserve_selection: bool) {
        let previous = self.selected_rel_path().map(|s| s.to_string());
        if self.search_query.is_empty() {
            self.filtered_indices = (0..self.notes.len()).collect();
        } else {
            let q = &self.search_query;
            let mut scored: Vec<(i64, usize)> = self
                .notes
                .iter()
                .enumerate()
                .filter_map(|(i, note)| {
                    let name_score = self.matcher.fuzzy_match(&note.rel_path, q);
                    let body_score = self
                        .body_index
                        .get(&note.rel_path)
                        .and_then(|b| self.matcher.fuzzy_match(b, q));
                    match (name_score, body_score) {
                        (Some(a), Some(b)) => Some((a.max(b) + a / 2, i)),
                        (Some(a), None) => Some((a + 50, i)),
                        (None, Some(b)) => Some((b, i)),
                        (None, None) => None,
                    }
                })
                .collect();
            scored.sort_by(|a, b| b.0.cmp(&a.0));
            self.filtered_indices = scored.into_iter().map(|(_, i)| i).collect();
        }

        if self.filtered_indices.is_empty() {
            self.state.select(None);
        } else if preserve_selection {
            let new_idx = previous
                .and_then(|p| {
                    self.filtered_indices
                        .iter()
                        .position(|&i| self.notes[i].rel_path == p)
                })
                .unwrap_or(0);
            self.state.select(Some(new_idx));
        } else {
            self.state.select(Some(0));
        }
        self.preview_cache = None;
    }

    pub fn selected_note(&self) -> Option<&NoteEntry> {
        let idx = self.state.selected()?;
        let real = *self.filtered_indices.get(idx)?;
        self.notes.get(real)
    }

    pub fn selected_rel_path(&self) -> Option<&str> {
        self.selected_note().map(|n| n.rel_path.as_str())
    }

    pub fn selected_path(&self) -> Option<PathBuf> {
        self.selected_note()
            .map(|n| Path::new(&self.directory).join(&n.rel_path))
    }

    pub fn list_len(&self) -> usize {
        self.filtered_indices.len()
    }

    pub fn move_selection(&mut self, delta: i32) {
        let len = self.list_len();
        if len == 0 {
            return;
        }
        let cur = self.state.selected().unwrap_or(0) as i32;
        let next = (cur + delta).clamp(0, len as i32 - 1);
        if next != cur {
            self.state.select(Some(next as usize));
            self.preview_cache = None;
            self.preview_scroll = 0;
        }
    }

    pub fn set_status<S: Into<String>>(&mut self, msg: S) {
        self.push_status(msg, false);
    }

    pub fn set_error<S: Into<String>>(&mut self, msg: S) {
        self.push_status(msg, true);
    }

    fn push_status<S: Into<String>>(&mut self, msg: S, is_error: bool) {
        let entry = StatusEntry {
            text: msg.into(),
            is_error,
        };
        self.status_log.insert(0, entry.clone());
        if self.status_log.len() > 50 {
            self.status_log.truncate(50);
        }
        self.status_message = Some(entry);
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

    pub fn cycle_sort(&mut self) {
        self.sort = self.sort.next();
        let selected_path = self.selected_rel_path().map(|s| s.to_string());
        self.notes =
            list_notes(&self.directory, !self.show_all, self.recursive, self.sort).unwrap_or_default();
        self.recompute_filter(false);
        // restore selection if possible
        if let Some(p) = selected_path {
            if let Some(pos) = self
                .filtered_indices
                .iter()
                .position(|&i| self.notes[i].rel_path == p)
            {
                self.state.select(Some(pos));
            }
        }
    }

    pub fn toggle_recursive(&mut self) {
        self.recursive = !self.recursive;
        self.refresh_notes();
    }

    pub fn change_directory(&mut self, new_dir: &str) {
        if !self.recent_dirs.iter().any(|d| d == new_dir) {
            self.recent_dirs.insert(0, new_dir.to_string());
            if self.recent_dirs.len() > 16 {
                self.recent_dirs.truncate(16);
            }
        }
        self.directory = new_dir.to_string();
        self.search_query.clear();
        self.refresh_notes();
    }
}

/// Build an initial list of useful directories to show in the chdir picker:
/// the current root, the process CWD, the home dir, and the configured
/// notes dir (`$SCRIBE_DIR` or `~/notes`). Deduplicated, order-preserving.
fn seed_recent_dirs(current: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |p: Option<String>| {
        if let Some(s) = p {
            if !s.is_empty() && !out.iter().any(|x| x == &s) {
                out.push(s);
            }
        }
    };

    push(Some(current.to_string()));
    push(
        std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(|s| s.to_string())),
    );
    push(crate::notes::resolve_notes_dir(None).into());
    push(
        dirs::home_dir().and_then(|p| p.to_str().map(|s| s.to_string())),
    );
    // Parent of the current dir, if any.
    push(
        Path::new(current)
            .parent()
            .and_then(|p| p.to_str().map(|s| s.to_string())),
    );

    out
}
