mod app;
mod cli;
mod notes;
mod render;
mod templates;
mod ui;

use clap::Parser;
use std::path::{Path, PathBuf};
use std::{fs, io};

use cli::Cli;
use notes::{
    SortMode, create_new_note, ensure_adoc_extension, fmt_mtime, fmt_size, list_notes,
    resolve_notes_dir, strip_adoc_extension,
};
use render::export;
use templates::Template;
use ui::run_tui;

fn main() -> io::Result<()> {
    let cli = Cli::parse();
    let directory = if cli.here {
        std::env::current_dir()?
            .to_str()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "current directory is not valid UTF-8"))?
            .to_string()
    } else {
        resolve_notes_dir(cli.dir.as_deref())
    };
    fs::create_dir_all(&directory)?;

    let sort = parse_sort(&cli.sort);

    if cli.list {
        return cmd_list(&directory, cli.all, cli.recursive, sort);
    }
    if let Some(title) = cli.new.as_deref() {
        return cmd_new(&directory, title, &cli.template);
    }
    if let Some(file) = cli.export.as_deref() {
        return cmd_export(&directory, file, cli.format.as_str(), cli.output.as_deref());
    }

    run_tui(&directory, cli.all)?;
    Ok(())
}

fn parse_sort(s: &str) -> SortMode {
    match s.to_ascii_lowercase().as_str() {
        "mtime" | "modified" | "mtime-desc" => SortMode::MtimeDesc,
        "mtime-asc" | "oldest" => SortMode::MtimeAsc,
        "size" => SortMode::SizeDesc,
        _ => SortMode::NameAsc,
    }
}

fn cmd_list(directory: &str, all: bool, recursive: bool, sort: SortMode) -> io::Result<()> {
    let entries = list_notes(directory, !all, recursive, sort)?;
    if entries.is_empty() {
        println!("(no documents in {directory})");
    } else {
        for e in entries {
            let mtime = e.mtime.map(fmt_mtime).unwrap_or_else(|| "—".to_string());
            println!("{:>7}  {}  {}", fmt_size(e.size), mtime, e.rel_path);
        }
    }
    Ok(())
}

fn cmd_new(directory: &str, title: &str, template_name: &str) -> io::Result<()> {
    let tpl = Template::from_name(template_name).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown template: {template_name}"),
        )
    })?;
    let filename = ensure_adoc_extension(title);
    let bare_title = strip_adoc_extension(title).to_string();
    let path = Path::new(directory).join(&filename);
    let contents = tpl.render(&bare_title);
    create_new_note(&path, &contents)?;
    println!("Created {}", path.display());
    Ok(())
}

fn cmd_export(directory: &str, file: &str, format: &str, output: Option<&str>) -> io::Result<()> {
    let candidate = PathBuf::from(file);
    let path = if candidate.exists() {
        candidate
    } else {
        Path::new(directory).join(ensure_adoc_extension(file))
    };
    if !path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("document not found: {}", path.display()),
        ));
    }
    let out = output.map(PathBuf::from);
    let written = export(&path, format, out.as_deref())?;
    println!("Exported {} -> {}", path.display(), written.display());
    Ok(())
}
