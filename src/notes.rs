use std::{
    fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

/// AsciiDoc file extension recognised by Scribe.
pub const ADOC_EXT: &str = "adoc";

/// How to sort the document list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortMode {
    NameAsc,
    MtimeDesc,
    MtimeAsc,
    SizeDesc,
}

impl SortMode {
    pub fn label(self) -> &'static str {
        match self {
            SortMode::NameAsc => "name",
            SortMode::MtimeDesc => "modified ↓",
            SortMode::MtimeAsc => "modified ↑",
            SortMode::SizeDesc => "size ↓",
        }
    }

    pub fn next(self) -> SortMode {
        match self {
            SortMode::NameAsc => SortMode::MtimeDesc,
            SortMode::MtimeDesc => SortMode::MtimeAsc,
            SortMode::MtimeAsc => SortMode::SizeDesc,
            SortMode::SizeDesc => SortMode::NameAsc,
        }
    }
}

/// Single entry returned by [`list_notes`].
#[derive(Clone, Debug)]
pub struct NoteEntry {
    /// Path relative to the listing root (e.g. `projects/foo.adoc`).
    pub rel_path: String,
    /// File modification time, if known.
    pub mtime: Option<SystemTime>,
    /// File size in bytes.
    pub size: u64,
}

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
/// Creates parent directories as needed.
pub fn create_new_note(file_path: &Path, contents: &str) -> Result<(), io::Error> {
    use std::io::Write;

    if let Some(parent) = file_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(file_path)?;
    file.write_all(contents.as_bytes())
}

/// Recognised AsciiDoc filename?
fn is_adoc_filename(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".adoc") || lower.ends_with(".asciidoc")
}

/// List notes in `directory`. When `only_adoc` is true, only `.adoc`/`.asciidoc`
/// files are returned. When `recursive` is true, descends into subdirectories
/// (skipping hidden dirs and the scribe trash).
pub fn list_notes(
    directory: &str,
    only_adoc: bool,
    recursive: bool,
    sort: SortMode,
) -> Result<Vec<NoteEntry>, io::Error> {
    let root = Path::new(directory);
    let mut entries: Vec<NoteEntry> = Vec::new();
    walk(root, root, only_adoc, recursive, &mut entries)?;
    sort_entries(&mut entries, sort);
    Ok(entries)
}

fn walk(
    root: &Path,
    dir: &Path,
    only_adoc: bool,
    recursive: bool,
    out: &mut Vec<NoteEntry>,
) -> io::Result<()> {
    let read = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    for entry in read.flatten() {
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            // Skip well-known noise + scribe internal dirs.
            if name == "node_modules" || name == "target" || name == ".scribe-trash" {
                continue;
            }
            if recursive {
                walk(root, &path, only_adoc, recursive, out)?;
            }
            continue;
        }
        if !path.is_file() {
            continue;
        }
        if only_adoc && !is_adoc_filename(&name) {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let meta = path.metadata().ok();
        out.push(NoteEntry {
            rel_path: rel,
            mtime: meta.as_ref().and_then(|m| m.modified().ok()),
            size: meta.as_ref().map(|m| m.len()).unwrap_or(0),
        });
    }
    Ok(())
}

fn sort_entries(entries: &mut [NoteEntry], sort: SortMode) {
    match sort {
        SortMode::NameAsc => entries.sort_by(|a, b| a.rel_path.cmp(&b.rel_path)),
        SortMode::MtimeDesc => entries.sort_by(|a, b| b.mtime.cmp(&a.mtime)),
        SortMode::MtimeAsc => entries.sort_by(|a, b| a.mtime.cmp(&b.mtime)),
        SortMode::SizeDesc => entries.sort_by(|a, b| b.size.cmp(&a.size)),
    }
}

/// Rename / move a note. Returns the new absolute path.
pub fn rename_note(from: &Path, to: &Path) -> io::Result<()> {
    if to.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("destination exists: {}", to.display()),
        ));
    }
    if let Some(parent) = to.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::rename(from, to)
}

/// Trash directory under the notes root.
pub fn trash_dir(root: &str) -> PathBuf {
    Path::new(root).join(".scribe-trash")
}

/// Move `file_path` into the per-root trash directory. Returns the trashed path
/// so a subsequent undo can restore it.
pub fn soft_delete_note(root: &str, file_path: &Path) -> io::Result<PathBuf> {
    let trash = trash_dir(root);
    fs::create_dir_all(&trash)?;
    let rel = file_path
        .strip_prefix(root)
        .unwrap_or(file_path)
        .to_string_lossy()
        .replace('/', "__")
        .replace('\\', "__");
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S-%f");
    let trashed = trash.join(format!("{ts}__{rel}"));
    fs::rename(file_path, &trashed)?;
    Ok(trashed)
}

/// Restore a trashed file to `original`. Returns the restored path.
pub fn restore_trashed(trashed: &Path, original: &Path) -> io::Result<()> {
    if original.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("cannot restore — file exists: {}", original.display()),
        ));
    }
    if let Some(parent) = original.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::rename(trashed, original)
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

/// Read up to `max_bytes` of `path` as a UTF-8 string (lossy on invalid bytes).
/// Used to feed the full-text fuzzy search index without loading huge files.
pub fn read_head_lossy(path: &Path, max_bytes: usize) -> io::Result<String> {
    use std::io::Read;
    let mut f = fs::File::open(path)?;
    let mut buf = vec![0u8; max_bytes];
    let n = f.read(&mut buf)?;
    buf.truncate(n);
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Pull the first AsciiDoc title (`= Title`) from a file head, if present.
#[allow(dead_code)]
pub fn first_title(path: &Path) -> Option<String> {
    let head = read_head_lossy(path, 4096).ok()?;
    for line in head.lines().take(40) {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("= ") {
            return Some(rest.trim().to_string());
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

/// Format a `SystemTime` as `YYYY-MM-DD HH:MM` (UTC-naive via chrono).
pub fn fmt_mtime(t: SystemTime) -> String {
    let dt: chrono::DateTime<chrono::Local> = t.into();
    dt.format("%Y-%m-%d %H:%M").to_string()
}

/// Human-friendly byte size.
pub fn fmt_size(n: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    if n >= MB {
        format!("{:.1}M", n as f64 / MB as f64)
    } else if n >= KB {
        format!("{:.1}K", n as f64 / KB as f64)
    } else {
        format!("{n}B")
    }
}
