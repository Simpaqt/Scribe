# Scribe

Scribe is a terminal-native tool for writing, organising, previewing and
exporting [AsciiDoc](https://asciidoc.org/) notes and documentation. It started
life as **Rustymemo**, a quick TUI for plain text notes, and grew into a
focused AsciiDoc workflow that still gets out of the way and lets you edit in
your favourite `$EDITOR`.

## Features

- Fast TUI for browsing AsciiDoc documents in a directory of your choice
- Document creation with built-in templates (blank, Technical README, daily journal)
- Fuzzy search across documents
- Toggleable in-pane preview rendered by `asciidoctor`
- One-key export to HTML (`asciidoctor`) or PDF (`asciidoctor-pdf`)
- Headless CLI flags for scripting (`--new`, `--list`, `--export`)
- Editor-agnostic — opens documents in `$EDITOR` (defaults to `nvim`)
- Configurable storage directory via `--dir` or `$SCRIBE_DIR`

## Installation

### Prerequisites

- Rust 1.70+ and `cargo` ([rustup](https://rustup.rs/))
- `asciidoctor` (Ruby gem) — required for preview and HTML export
- `asciidoctor-pdf` — optional, required only for PDF export
- A terminal editor referenced by `$EDITOR` (defaults to `nvim`)

```bash
# Install the AsciiDoc toolchain (one-time)
gem install asciidoctor asciidoctor-pdf
```

### Build & install

```bash
git clone https://github.com/Simpaqt/Rustymemo.git
cd Rustymemo
cargo build --release
install -m 0755 target/release/scribe ~/.local/bin/scribe
```

Make sure `~/.local/bin` (or wherever you placed the binary) is in your `PATH`.

## Usage

```bash
scribe                       # launch the TUI on the default directory
scribe --here                # launch the TUI on the current working directory
scribe -H                    # short form of --here
scribe --dir ~/docs          # use a custom directory
SCRIBE_DIR=~/docs scribe     # same via env var
scribe --list                # list documents and exit
scribe --new "Project Plan" --template readme           # create in ~/notes
scribe -H --new "README" --template readme              # create in CWD
scribe --export ~/docs/plan.adoc --format pdf -o ~/plan.pdf
```

### Working in the current directory

There are two ways to target your current shell directory instead of the
configured notes directory:

- **`-H` / `--here`** on the CLI applies to every command (TUI, `--new`,
  `--list`, `--export`).
- **Inside the TUI**:
  - **`I`** (capital) creates a document directly in the process's working
    directory using the currently selected template.
  - **`T`** (capital) opens the template picker and lands the resulting
    document in the current working directory.
  - Lowercase `i` / `t` still target the configured notes directory.
  - The create-mode and template-picker title bars tell you which target will
    be used.

### Directory & format conventions

- New documents always end with `.adoc` (added automatically if you forget).
- The list view shows only `.adoc`/`.asciidoc` files by default. Pass `--all`
  to include everything in the directory.
- Default directory: `~/notes` (kept for backward compatibility with
  Rustymemo). Override with `--dir` or `$SCRIBE_DIR`.

### Templates

Selectable via `--template <name>` on the CLI, or in the TUI by pressing `t`:

| Name      | Description                              |
| --------- | ---------------------------------------- |
| `blank`   | Minimal AsciiDoc header                  |
| `readme`  | Technical README skeleton                |
| `journal` | Date-stamped daily journal entry         |

Templates substitute `{{title}}`, `{{author}}` (from `$USER`/`$USERNAME`) and
`{{date}}` (today, `YYYY-MM-DD`).

### Keybindings

#### Normal mode

| Key      | Action                                      |
| -------- | ------------------------------------------- |
| `j`/`k`  | Move down / up (arrow keys also work)       |
| `o`      | Open selected document in `$EDITOR`         |
| `i`      | Create a new document in the configured directory |
| `I`      | Create a new document in the current working directory |
| `t`      | Pick a template, then create in the configured directory |
| `T`      | Pick a template, then create in the current working directory |
| `/`      | Enter fuzzy search                          |
| `p`      | Toggle preview pane                         |
| `e` `h`  | Export selected document to HTML            |
| `e` `p`  | Export selected document to PDF             |
| `d` `d`  | Delete selected document (press `d` twice)  |
| `q`      | Quit                                        |

#### Create mode

Type the title (extension optional) and press `Enter`. `Esc` cancels.

#### Template picker

`j`/`k` to choose, `Enter` to confirm, `Esc` to cancel.

#### Search mode

Type to filter. `j`/`k` or arrow keys navigate, `Enter` opens, `Esc` exits.

## Editor configuration

Scribe respects `$EDITOR` everywhere. Fallbacks:

- Linux/macOS: `nvim`
- Windows: tries `code`, `notepad++`, then `notepad`

```bash
export EDITOR=hx     # or vim, nano, code, etc.
```

## Notes on the rename

Scribe is the successor to **Rustymemo**. The repository name still reflects
the original project, but the package and binary are now `scribe`. Existing
notes in `~/notes` continue to work; only `.adoc`/`.asciidoc` files appear by
default, so add `--all` if you still want to see your old plain-text notes.

---

Built with Rust, `ratatui`, `crossterm` and `clap`.
