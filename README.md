# bmd

A TUI markdown viewer for the terminal, built with Rust.

## Features

- **Type-safe domain model**: Kamae-style Rust domain design with explicit state transitions.
- **Vim keybindings**: navigate with `j`/`k`, `Ctrl-d`/`Ctrl-u`, `gg`/`G`, etc.
- **Rich markup**: headings, bold/italic/code, blockquotes, lists, syntax-highlighted code blocks.
- **Native mermaid rendering**: mermaid code blocks are rendered to PNG via the pure-Rust `merman` crate and displayed inline using terminal image protocols (Kitty / iTerm2 / Sixel), falling back to Unicode half-blocks when needed.
- **Responsive tables**: Markdown tables wrap columns based on terminal width.
- **Browser links**: press `n`/`N` to cycle links, `o` or `Enter` to open the selected link with macOS `open`.

## Requirements

- Rust 1.92+
- macOS (for the `open` browser launcher; the viewer itself is portable Rust)
- A terminal that supports one of the image protocols for the best mermaid experience

## Build

```bash
cargo build --release
```

On macOS, if your default `cc` is not Apple clang, use:

```bash
RUSTFLAGS="-C linker=clang" cargo build --release
```

## Usage

```bash
# Open a file
bmd README.md

# Read from stdin
bmd < some-file.md

# Pipe
some-generator | bmd
```

## Keybindings

| Key | Action |
|-----|--------|
| `j` / `↓` | scroll down one line |
| `k` / `↑` | scroll up one line |
| `d` / `Ctrl-d` | half page down |
| `u` / `Ctrl-u` | half page up |
| `g` `g` | jump to top |
| `G` | jump to bottom |
| `Tab` / `n` | next link (or next search match when a search is active) |
| `Shift-Tab` / `N` | previous link (or previous search match when a search is active) |
| `o` / `Enter` | open selected link in browser |
| `/` | start forward search |
| `?` | start backward search |
| `Enter` | confirm search query |
| `Esc` | cancel search input |
| `Backspace` | delete last search character |
| `q` / `Ctrl-c` | quit |

## Architecture

```text
src/
├── main.rs        # entry point and terminal setup
├── app.rs         # application loop and command handling
├── domain.rs      # domain model and typed state transitions
├── error.rs       # structured errors
├── parse.rs       # pulldown-cmark -> domain model
├── render.rs      # domain model -> ratatui widgets
├── keymap.rs      # vim keybinding mapping
└── browser.rs     # macOS open adapter
```

## License

MIT OR Apache-2.0
