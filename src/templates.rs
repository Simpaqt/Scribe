use chrono::Local;
use std::path::PathBuf;

/// A document template — either one of the built-ins compiled into the binary,
/// or a user-supplied `.adoc` file under `~/.config/scribe/templates/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Template {
    Builtin(BuiltinTemplate),
    User { name: String, body: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinTemplate {
    Blank,
    Readme,
    Journal,
}

impl BuiltinTemplate {
    pub const ALL: &'static [BuiltinTemplate] = &[
        BuiltinTemplate::Blank,
        BuiltinTemplate::Readme,
        BuiltinTemplate::Journal,
    ];

    pub fn name(self) -> &'static str {
        match self {
            BuiltinTemplate::Blank => "blank",
            BuiltinTemplate::Readme => "readme",
            BuiltinTemplate::Journal => "journal",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            BuiltinTemplate::Blank => "Minimal AsciiDoc header",
            BuiltinTemplate::Readme => "Technical README skeleton",
            BuiltinTemplate::Journal => "Date-stamped daily journal entry",
        }
    }

    fn raw(self) -> &'static str {
        match self {
            BuiltinTemplate::Blank => include_str!("../templates/blank.adoc"),
            BuiltinTemplate::Readme => include_str!("../templates/readme.adoc"),
            BuiltinTemplate::Journal => include_str!("../templates/journal.adoc"),
        }
    }
}

impl Template {
    pub fn name(&self) -> &str {
        match self {
            Template::Builtin(b) => b.name(),
            Template::User { name, .. } => name,
        }
    }

    pub fn description(&self) -> String {
        match self {
            Template::Builtin(b) => b.description().to_string(),
            Template::User { .. } => "User template".to_string(),
        }
    }

    fn raw(&self) -> &str {
        match self {
            Template::Builtin(b) => b.raw(),
            Template::User { body, .. } => body.as_str(),
        }
    }

    /// Render the template, substituting `{{title}}`, `{{author}}`, `{{date}}`.
    pub fn render(&self, title: &str) -> String {
        let date = Local::now().format("%Y-%m-%d").to_string();
        let author = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| String::new());

        self.raw()
            .replace("{{title}}", title)
            .replace("{{author}}", &author)
            .replace("{{date}}", &date)
    }

    pub fn from_name(name: &str) -> Option<Template> {
        let lower = name.to_ascii_lowercase();
        match lower.as_str() {
            "blank" | "" => return Some(Template::Builtin(BuiltinTemplate::Blank)),
            "readme" => return Some(Template::Builtin(BuiltinTemplate::Readme)),
            "journal" | "daily" => return Some(Template::Builtin(BuiltinTemplate::Journal)),
            _ => {}
        }
        // Fall back to user templates.
        load_all().into_iter().find(|t| t.name() == name)
    }
}

/// Directory in which user templates live: `$XDG_CONFIG_HOME/scribe/templates`
/// or `~/.config/scribe/templates`.
pub fn user_templates_dir() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("scribe").join("templates"));
        }
    }
    dirs::config_dir().map(|d| d.join("scribe").join("templates"))
}

/// Load user templates from disk. Each `*.adoc` file in [`user_templates_dir`]
/// becomes a template named after its file stem.
pub fn load_user_templates() -> Vec<Template> {
    let Some(dir) = user_templates_dir() else {
        return Vec::new();
    };
    let read = match std::fs::read_dir(&dir) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for entry in read.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let ext_ok = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| {
                let e = e.to_ascii_lowercase();
                e == "adoc" || e == "asciidoc"
            })
            .unwrap_or(false);
        if !ext_ok {
            continue;
        }
        if let Ok(body) = std::fs::read_to_string(&path) {
            out.push(Template::User { name, body });
        }
    }
    out.sort_by(|a, b| a.name().cmp(b.name()));
    out
}

/// All available templates: built-ins first, then user templates.
pub fn load_all() -> Vec<Template> {
    let mut v: Vec<Template> = BuiltinTemplate::ALL
        .iter()
        .copied()
        .map(Template::Builtin)
        .collect();
    v.extend(load_user_templates());
    v
}
