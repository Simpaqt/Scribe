use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Returns true if the named command exists on PATH.
pub fn command_available(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Render an AsciiDoc file to plain text suitable for a TUI preview pane.
///
/// Strategy: ask `asciidoctor` for HTML on stdout, then strip tags.
pub fn render_preview(path: &Path) -> io::Result<String> {
    let output = Command::new("asciidoctor")
        .args(["-s", "-a", "showtitle", "-o", "-", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            let contents = std::fs::read(path)?;
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(&contents)?;
            }
            child.wait_with_output()
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::other(format!(
            "asciidoctor failed: {}",
            stderr.trim()
        )));
    }

    let html = String::from_utf8_lossy(&output.stdout).into_owned();
    Ok(strip_html(&html))
}

/// Very small HTML → text converter for preview purposes. Not a real parser:
/// drops tags, decodes a handful of entities, normalises whitespace.
fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut current_tag = String::new();

    for ch in html.chars() {
        match ch {
            '<' => {
                in_tag = true;
                current_tag.clear();
            }
            '>' => {
                in_tag = false;
                let stripped = current_tag.trim_start_matches('/').to_ascii_lowercase();
                let tag_name: String = stripped
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric())
                    .collect();
                match tag_name.as_str() {
                    "br" | "p" | "div" | "li" | "tr" | "h1" | "h2" | "h3" | "h4" | "h5"
                    | "h6" | "pre" | "hr" | "ul" | "ol" => {
                        if !out.ends_with('\n') {
                            out.push('\n');
                        }
                    }
                    _ => {}
                }
            }
            c if in_tag => {
                current_tag.push(c);
            }
            c => out.push(c),
        }
    }

    let decoded = out
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");

    // Collapse 3+ consecutive newlines down to 2.
    let mut collapsed = String::with_capacity(decoded.len());
    let mut newline_run = 0;
    for ch in decoded.chars() {
        if ch == '\n' {
            newline_run += 1;
            if newline_run <= 2 {
                collapsed.push('\n');
            }
        } else {
            newline_run = 0;
            collapsed.push(ch);
        }
    }
    collapsed.trim_start_matches('\n').to_string()
}

/// Export an AsciiDoc file to HTML or PDF via asciidoctor / asciidoctor-pdf.
pub fn export(path: &Path, format: &str, output: Option<&Path>) -> io::Result<PathBuf> {
    let (bin, ext) = match format {
        "html" => ("asciidoctor", "html"),
        "pdf" => ("asciidoctor-pdf", "pdf"),
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown export format: {other}"),
            ));
        }
    };

    if !command_available(bin) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("`{bin}` not found in PATH; install it to use this feature"),
        ));
    }

    let out_path = match output {
        Some(p) => p.to_path_buf(),
        None => path.with_extension(ext),
    };

    let status = Command::new(bin)
        .arg("-o")
        .arg(&out_path)
        .arg(path)
        .status()?;

    if !status.success() {
        return Err(io::Error::other(format!(
            "{bin} exited with status {status}"
        )));
    }

    Ok(out_path)
}
