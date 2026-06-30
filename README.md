# bmd

A TUI markdown viewer for the terminal, built with Rust.

## Features

- **Type-safe domain model**: Kamae-style Rust domain design with explicit state transitions.
- **Vim keybindings**: navigate with `j`/`k`, `Ctrl-d`/`Ctrl-u`, `g`/`G`, etc.
- **Rich markup**: headings, bold/italic/code, blockquotes, lists, syntax-highlighted code blocks.
- **Native mermaid rendering**: mermaid code blocks are rendered to PNG via the pure-Rust `merman` crate and displayed inline using terminal image protocols (Kitty / iTerm2 / Sixel), falling back to Unicode half-blocks when needed.
- **Responsive tables**: Markdown tables wrap columns based on terminal width.
- **In-document search**: forward (`/`) and backward (`?`) search with match highlighting.
- **Browser links**: press `n`/`N` to cycle links, `o` or `Enter` to open the selected link with macOS `open`.

## Requirements

- [devbox](https://www.jetify.com/devbox) (recommended; provides Rust 1.92, clang, sccache, prek)
- macOS (for the `open` browser launcher; the viewer itself is portable Rust)
- A terminal that supports one of the image protocols for the best mermaid experience (Ghostty, Kitty, iTerm2, WezTerm, etc.)

## Quick start

```bash
devbox run setup
devbox run build-release
./target/release/bmd sample.md
```

## Build

Use devbox for all builds so linker flags, `CARGO_HOME`, and sccache stay consistent:

```bash
devbox run build          # debug
devbox run build-release  # release
```

Without devbox you must set `RUSTFLAGS="-C linker=clang"` on macOS when the default `cc` is not Apple clang. Mixing devbox and plain `cargo` invalidates incremental artifacts because `RUSTFLAGS` differ.

## Development

This project uses [devbox](https://www.jetify.com/devbox) for a reproducible toolchain (Rust 1.92, clang, sccache, prek).

```bash
devbox shell
devbox run setup        # install toolchain and fetch dependencies
devbox run build        # debug build
devbox run build-release
devbox run test
devbox run run -- sample.md
devbox run clippy
devbox run fmt
devbox run prek         # run pre-commit hooks
devbox run cache-stats  # sccache hit rate and size
```

`devbox.json` sets project-local `RUSTUP_HOME`, `CARGO_HOME`, and `SCCACHE_DIR`, plus `RUSTFLAGS="-C linker=clang"` and `RUSTC_WRAPPER=sccache`. Build artifacts live in `target/`; rustc compilations are also cached in `.sccache/`.

Run builds from a normal terminal (not a sandboxed IDE shell) so `target/` is reused. Some editor sandboxes redirect `CARGO_TARGET_DIR` to a temp directory, which makes every build look like a cold start.

## Usage

```bash
# Open a file
bmd README.md

# Read from stdin
bmd < some-file.md

# Pipe
some-generator | bmd
```

Set `BMD_DEBUG=1` to log key events to stderr while debugging bindings.

## Keybindings

| Key | Action |
|-----|--------|
| `j` / `↓` | scroll down two lines |
| `k` / `↑` | scroll up two lines |
| `d` / `Ctrl-d` / `PageDown` | half page down |
| `u` / `Ctrl-u` / `PageUp` | half page up |
| `g` | jump to top |
| `G` | jump to bottom |
| `Tab` / `n` | next link (or next search match when a search is active) |
| `Shift-Tab` / `N` | previous link (or previous search match when a search is active) |
| `o` / `Enter` | open selected link in browser |
| `/` | start forward search |
| `?` | start backward search |
| `Enter` | confirm search query |
| `Esc` | cancel search input, clear active search, or quit |
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

See [`PLAN.md`](PLAN.md) for the original design notes (Japanese).

## License

MIT OR Apache-2.0
