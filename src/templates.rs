use chrono::Local;

/// Built-in document templates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Template {
    Blank,
    Readme,
    Journal,
}

impl Template {
    pub const ALL: &'static [Template] = &[Template::Blank, Template::Readme, Template::Journal];

    pub fn name(self) -> &'static str {
        match self {
            Template::Blank => "blank",
            Template::Readme => "readme",
            Template::Journal => "journal",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Template::Blank => "Minimal AsciiDoc header",
            Template::Readme => "Technical README skeleton",
            Template::Journal => "Date-stamped daily journal entry",
        }
    }

    pub fn from_name(name: &str) -> Option<Template> {
        match name.to_ascii_lowercase().as_str() {
            "blank" | "" => Some(Template::Blank),
            "readme" => Some(Template::Readme),
            "journal" | "daily" => Some(Template::Journal),
            _ => None,
        }
    }

    fn raw(self) -> &'static str {
        match self {
            Template::Blank => include_str!("../templates/blank.adoc"),
            Template::Readme => include_str!("../templates/readme.adoc"),
            Template::Journal => include_str!("../templates/journal.adoc"),
        }
    }

    /// Render the template, substituting `{{title}}`, `{{author}}`, `{{date}}`.
    pub fn render(self, title: &str) -> String {
        let date = Local::now().format("%Y-%m-%d").to_string();
        let author = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| String::new());

        self.raw()
            .replace("{{title}}", title)
            .replace("{{author}}", &author)
            .replace("{{date}}", &date)
    }
}
