# About
aksldj

Rustymemo is a fast and intuitive CLI note-taking tool that lets you manage notes from anywhere in your terminal. Built with Rust and featuring a modern TUI interface, it's designed for developers who want quick access to their notes without leaving the command line.

## ✨ Features

- **📝 Quick Note Creation**: Create notes instantly from any directory
- **🔍 Fuzzy Search**: Find notes quickly with real-time fuzzy matching
- **⚡ Fast Navigation**: Vim-like keybindings for efficient navigation
- **🎨 Modern UI**: Clean terminal interface with helpful status bar
- **📁 Organized Storage**: All notes saved to `~/notes` directory
- **🔧 Editor Integration**: Opens notes in your preferred editor (cross-platform support)
- **🪟 Windows Compatible**: Works seamlessly on Windows, Linux, and macOS

### What you can do:

1. **Create** new notes that are saved to `~/notes`
2. **Search** through notes with fuzzy matching
3. **Edit** existing notes in your preferred editor
4. **Delete** notes with confirmation
5. **Navigate** efficiently with vim-like keybindings

# Installation

Currently the only way to install is with cargo

### Prerequisites

You'll need Rust 1.56+ (for Rust 2021 edition support) and cargo. Install using rustup:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Build and Install

```bash
git clone https://github.com/Simpaqt/Rustymemo.git
cd Rustymemo/
cargo build --release
mv target/release/Rustymemo ~/.local/bin/
```

**On Linux/macOS:**
Make sure `~/.local/bin` is in your PATH, or alternatively install to a directory that's already in your PATH:

```bash
# Alternative: install to /usr/local/bin (requires sudo)
sudo mv target/release/Rustymemo /usr/local/bin/
```

**On Windows:**
Add the executable to a directory in your PATH, or add the directory to your PATH:

```powershell
# Move to a directory in your PATH
move target\release\Rustymemo.exe C:\Windows\System32\
```

## 🚀 Usage

Simply run `Rustymemo` from anywhere in your terminal to launch the application.

### 🔧 Editor Configuration

Rustymemo automatically detects the best available text editor for your platform:

- **Linux/macOS**: Uses `nvim` by default
- **Windows**: Tries VS Code (`code`), Notepad++ (`notepad++`), then falls back to `notepad`
- **Custom Editor**: Set the `EDITOR` environment variable to use your preferred editor

To set a custom editor:

```bash
# Linux/macOS
export EDITOR=vim  # or nano, emacs, etc.

# Windows (PowerShell)
$env:EDITOR = "code"  # or "notepad++", "vim", etc.
```

Rustymemo features an intuitive interface with a dynamic status bar that shows context-sensitive help and keybinding hints for the current mode. The interface adapts based on what you're doing:

### Navigation
- **`j` / `k`** - Move up and down through notes (works in all modes)
- **`↑` / `↓`** - Alternative navigation (works in search mode)

### Actions
- **`i`** - Create a new note
- **`o`** - Open selected note in your default editor
- **`/`** - Enter search mode for fuzzy finding
- **`dd`** - Delete note (press 'd' twice for confirmation)
- **`q`** - Quit application

### Search Mode
- **Type** - Filter notes with fuzzy matching in real-time
- **`j` / `k`** - Navigate through filtered results
- **`↑` / `↓`** - Alternative navigation through filtered results
- **`Enter`** - Open selected note from search results
- **`Backspace`** - Remove characters from search query
- **`Esc`** - Exit search mode and return to normal view

### Create Mode
- **Type** - Enter note name
- **`Enter`** - Create the note (only if name is not empty and file doesn't exist)
- **`Backspace`** - Remove characters from note name
- **`Esc`** - Cancel creation and return to normal mode

### 💡 Smart Interface Features

- **Dynamic Status Bar**: Shows context-sensitive help and available keybindings for the current mode
- **Real-time Search**: Fuzzy matching updates results as you type
- **Visual Feedback**: Clear indicators for different modes (Normal, Create, Search)
- **Confirmation Prompts**: Safe deletion with double-key confirmation
- **Responsive Navigation**: Vim-like keybindings that work consistently across modes

The status bar at the bottom always shows available keybindings for the current mode, so you never have to memorize commands!

### Showcase


https://github.com/user-attachments/assets/75d06d42-76c3-4ce9-8540-6f5c35bbb86f

---

**Built with ❤️ in Rust** • Fast, reliable, and terminal-native
