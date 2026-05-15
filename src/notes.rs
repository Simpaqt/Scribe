use std::{fs, io, path::Path};

/// AsciiDoc file extension recognised by Scribe.
pub const ADOC_EXT: &str = "adoc";

/// Ensure `name` ends with `.adoc`, appending it if missing.
pub fn ensure_adoc_extension(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".adoc") || lower.ends_with(".asciidoc") {
        name.to_string()
    } else {
        format!("{name}.{ADOC_EXT}")
    }
}

/// Strip a trailing `.adoc`/`.asciidoc` extension to recover a title.
pub fn strip_adoc_extension(name: &str) -> &str {
    if let Some(stem) = name.strip_suffix(".adoc") {
        stem
    } else if let Some(stem) = name.strip_suffix(".asciidoc") {
        stem
    } else {
        name
    }
}

/// Create a new note file at the specified path with the given contents.
pub fn create_new_note(file_path: &str, contents: &str) -> Result<(), io::Error> {
    if Path::new(file_path).exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("file already exists: {file_path}"),
        ));
    }
    fs::write(file_path, contents)
}

/// List notes in `directory`. When `only_adoc` is true, only `.adoc`/`.asciidoc`
/// files are returned.
pub fn list_notes(directory: &str, only_adoc: bool) -> Result<Vec<String>, io::Error> {
    let mut entries: Vec<String> = fs::read_dir(directory)?
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if !path.is_file() {
                return None;
            }
            let name = path.file_name()?.to_str()?.to_string();
            if only_adoc {
                let lower = name.to_ascii_lowercase();
                if !(lower.ends_with(".adoc") || lower.ends_with(".asciidoc")) {
                    return None;
                }
            }
            Some(name)
        })
        .collect();
    entries.sort();
    Ok(entries)
}

/// Delete a note file at the specified path.
pub fn delete_note(file_path: &str) -> Result<(), io::Error> {
    fs::remove_file(file_path)
}

/// Resolve the notes directory: explicit override > `$SCRIBE_DIR` > `~/notes`.
pub fn resolve_notes_dir(cli_override: Option<&str>) -> String {
    if let Some(d) = cli_override {
        return d.to_string();
    }
    if let Ok(d) = std::env::var("SCRIBE_DIR") {
        if !d.is_empty() {
            return d;
        }
    }
    dirs::home_dir()
        .and_then(|path| path.join("notes").to_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "notes".to_string())
}
