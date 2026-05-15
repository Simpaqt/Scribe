use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
#[command(
    name = "scribe",
    version,
    about = "A TUI for managing, previewing and exporting AsciiDoc notes & docs",
    long_about = None,
)]
pub struct Cli {
    /// Directory where .adoc files live (overrides $SCRIBE_DIR, default ~/notes)
    #[arg(short, long, value_name = "PATH", env = "SCRIBE_DIR")]
    pub dir: Option<String>,

    /// Use the current working directory (shorthand for --dir .)
    #[arg(short = 'H', long, conflicts_with = "dir")]
    pub here: bool,

    /// Show all files in the directory, not just .adoc files
    #[arg(long)]
    pub all: bool,

    /// Create a new document with the given title (no TUI).
    #[arg(long, value_name = "TITLE")]
    pub new: Option<String>,

    /// Template to use with --new (blank, readme, journal)
    #[arg(short, long, value_name = "NAME", default_value = "blank")]
    pub template: String,

    /// Export the given document and exit
    #[arg(long, value_name = "FILE")]
    pub export: Option<String>,

    /// Output format for --export
    #[arg(long, value_enum, default_value_t = ExportFormat::Html)]
    pub format: ExportFormat,

    /// Optional output path for --export (defaults next to source)
    #[arg(short, long, value_name = "PATH")]
    pub output: Option<String>,

    /// List documents and exit
    #[arg(long)]
    pub list: bool,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum ExportFormat {
    Html,
    Pdf,
}

impl ExportFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            ExportFormat::Html => "html",
            ExportFormat::Pdf => "pdf",
        }
    }
}
