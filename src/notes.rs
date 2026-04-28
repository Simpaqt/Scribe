use std::{fs, io};

/// Create a new note file at the specified path
pub fn create_new_note(file_path: &str) -> Result<(), io::Error> {
    fs::File::create(file_path)?;
    Ok(())
}

/// List all notes in the specified directory
pub fn list_notes(directory: &str) -> Result<Vec<String>, io::Error> {
    let entries = fs::read_dir(directory)?
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.is_file() {
                path.file_name()?.to_str().map(|s| s.to_string())
            } else {
                None
            }
        })
        .collect::<Vec<String>>();
    Ok(entries)
}

/// Delete a note file at the specified path
pub fn delete_note(file_path: &str) -> Result<(), io::Error> {
    fs::remove_file(file_path)?;
    Ok(())
}

/// Get the default notes directory path (~/notes)
pub fn get_notes_dir() -> String {
    dirs::home_dir()
        .and_then(|path| path.join("notes").to_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "notes".to_string())
}
