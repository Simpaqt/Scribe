use crate::notes::list_notes;
use crate::templates::Template;
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use ratatui::widgets::ListState;

#[derive(PartialEq)]
pub enum AppMode {
    Normal,
    Create,
    TemplatePick,
    Search,
}

/// Pending multi-key chord (e.g. `dd`, `eh`, `ep`).
#[derive(PartialEq, Clone, Copy)]
pub enum Pending {
    None,
    Delete,
    Export,
}

pub struct App {
    pub notes: Vec<String>,
    pub filtered_notes: Vec<String>,
    pub state: ListState,
    pub directory: String,
    pub input: String,
    pub mode: AppMode,
    pub pending: Pending,
    pub search_query: String,
    pub show_all: bool,
    pub preview_open: bool,
    pub preview_cache: Option<(String, String)>,
    pub status_message: Option<String>,
    pub template_state: ListState,
    pub chosen_template: Template,
    /// When true, the next `Create` action writes to the process CWD instead
    /// of `self.directory`.
    pub create_in_cwd: bool,
    matcher: SkimMatcherV2,
}

impl App {
    pub fn new(directory: &str, show_all: bool) -> App {
        let mut state = ListState::default();
        let notes = list_notes(directory, !show_all).unwrap_or_default();
        let filtered_notes = notes.clone();
        if !notes.is_empty() {
            state.select(Some(0));
        }
        let mut template_state = ListState::default();
        template_state.select(Some(0));
        App {
            notes,
            filtered_notes,
            state,
            directory: directory.to_string(),
            input: String::new(),
            mode: AppMode::Normal,
            pending: Pending::None,
            search_query: String::new(),
            show_all,
            preview_open: false,
            preview_cache: None,
            status_message: None,
            template_state,
            chosen_template: Template::Blank,
            create_in_cwd: false,
            matcher: SkimMatcherV2::default(),
        }
    }

    pub fn refresh_notes(&mut self) {
        self.notes = list_notes(&self.directory, !self.show_all).unwrap_or_default();
        self.update_filtered_notes();
        let notes_to_check = if self.mode == AppMode::Search {
            &self.filtered_notes
        } else {
            &self.notes
        };

        if !notes_to_check.is_empty() {
            if let Some(selected) = self.state.selected() {
                if selected >= notes_to_check.len() {
                    self.state.select(Some(notes_to_check.len() - 1));
                }
            } else {
                self.state.select(Some(0));
            }
        } else {
            self.state.select(None);
        }
        // Selection may have changed file; invalidate preview cache.
        self.preview_cache = None;
    }

    pub fn update_filtered_notes(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_notes = self.notes.clone();
        } else {
            self.filtered_notes = self
                .notes
                .iter()
                .filter_map(|note| {
                    self.matcher
                        .fuzzy_match(note, &self.search_query)
                        .map(|_| note.clone())
                })
                .collect();
        }

        if !self.filtered_notes.is_empty() {
            self.state.select(Some(0));
        } else {
            self.state.select(None);
        }
        self.preview_cache = None;
    }

    pub fn get_current_notes(&self) -> &Vec<String> {
        if self.mode == AppMode::Search {
            &self.filtered_notes
        } else {
            &self.notes
        }
    }

    pub fn selected_note(&self) -> Option<&String> {
        let idx = self.state.selected()?;
        self.get_current_notes().get(idx)
    }

    pub fn set_status<S: Into<String>>(&mut self, msg: S) {
        self.status_message = Some(msg.into());
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }
}
