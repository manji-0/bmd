# bmd

A terminal TUI for reading Markdown. Vim-style keybindings, rich markup rendering, native Mermaid diagrams, in-document search, and interactive task lists.

## Features

### Markdown rendering

Documents parsed with [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark) are drawn with [ratatui](https://github.com/ratatui/ratatui). Text wraps to the terminal width; only blocks in the visible scroll region are rendered.

- **Headings** — H1–H6 with per-level styles and `#` prefix markers
- **Paragraphs** — bold, italic, inline code, hard breaks
- **Code blocks** — syntax highlighting via [syntect](https://github.com/trishume/syntect) with a language label
- **Block quotes** — nested block quotes supported
- **Lists** — ordered and unordered, including nested lists
- **Task lists** — GitHub-style `- [ ]` / `- [x]` checklists; click a checkbox to toggle (session-only, not saved to disk)
- **Tables** — column widths adapt to terminal width; cells wrap internally
- **Horizontal rules** — `---` and similar rule lines

### Vim-style navigation

Scroll position is tracked in logical lines; the on-screen position is animated with exponential smoothing. Holding `j` / `k` follows the OS key repeat with rate limiting.

| Action | Keys |
|--------|------|
| Scroll down / up 2 lines | `j` `↓` / `k` `↑` |
| Half page down / up | `d` `PageDown` / `u` `PageUp` |
| Jump to top / bottom | `g` / `G` |
| Quit | `q` `Ctrl-c` |

### In-document search

Press `/` for forward search or `?` for backward search. A prompt appears at the bottom of the screen; press `Enter` to confirm and return to normal mode. Matches are highlighted in yellow; the current match is emphasized in magenta.

- Case-insensitive substring matching
- Searches plain text across paragraphs, code blocks, lists, block quotes, and tables
- After confirming, `n` / `N` / `p` (or `Tab` / `Shift-Tab`) move between matches and scroll to the matching line
- `Esc` while search is active clears the search (does not quit)

### Links and preview

Cycle through links in the document with `n` / `N` / `p`. Only links currently visible on screen are included; pressing `n` wraps within that set without scrolling.

| Type | Example | `o` / `Enter` |
|------|---------|---------------|
| Web | `[text](https://…)` | Opens in the browser via macOS `open` / Linux `xdg-open` |
| Anchor | `[text](#section)` | Jumps to the matching heading; prior scroll positions are stacked |
| Document | `[text](./other.md)` | Opens the linked file in the same view; file stack supports nested navigation |
| Image | `![alt](path.png)` | Floating in-terminal preview |
| Mermaid | Link from a mermaid code block | Floating preview of the rendered diagram |

Close the preview overlay with `Esc` or `o`. The `o`/`O` pairing is consistent: `o` opens links and previews; `O` closes an open preview or steps back one navigation level (anchor jump or previous file). `Esc` resets the anchor stack to your pre-jump scroll position, or returns to the first opened file when only the document stack is active. Anchor navigation takes priority over document navigation when both apply. Each stack keeps the live current section or file outside the stack; following a link fixes the prior position/document once at jump time (scrolling and other navigation never update stored priors). Both stacks count the current item as layer 1 and support up to 64 layers. Further link jumps beyond that limit show a status-bar message and leave the current view unchanged. Web links are blue; image and Mermaid links are magenta. The selected link is shown inverted.

### Task lists

Markdown task lists (`- [ ]` / `- [x]`) render with checkbox markers. Left-click a marker to toggle checked state for the current session; changes are not written back to the file.

Marker appearance is chosen automatically:

| `BMD_CHECKLIST_STYLE` | Markers |
|-----------------------|---------|
| `unicode` (default when auto-detection is inconclusive) | `☐` / `☑` |
| `emoji` | `⬜` / `✅` |
| `auto` or unset | Emoji when the terminal is identifiable (Kitty, Ghostty, iTerm2, WezTerm, Apple Terminal, VS Code); otherwise Unicode |

### Mermaid and images

Mermaid fenced code blocks are rasterized with the pure-Rust [merman](https://crates.io/crates/merman) crate and displayed inline using the terminal graphics protocol.

- Queries terminal capabilities at startup (Kitty, iTerm2, Sixel, etc.); falls back to Unicode half-blocks when unsupported
- Pauses image drawing while scrolling; resumes 100 ms after scrolling stops
- Markdown images with relative paths resolve against the input file's directory

### Other

- **Type-safe domain model** — Kamae-style state transitions (`ViewState` methods consume `self`)
- **Document render cache** — full document buffered until width or highlight state changes; scrolling only blits the viewport
- **stdin / file input** — path argument, `-`, or pipe; file paths reload automatically on save (scroll position preserved)
- **Debug** — `BMD_DEBUG=1` logs key events and commands to stderr

## Requirements

- [devbox](https://www.jetify.com/devbox) (recommended; provides Rust 1.92, clang, sccache, prek)
- macOS for opening web links via `open`; Linux uses `xdg-open`
- Kitty, Ghostty, iTerm2, WezTerm, or similar for inline Mermaid and image rendering

## Quick start

```bash
# Install from crates.io
cargo install bmd

# Or build from source (devbox)
devbox run setup
devbox run build-release
./target/release/bmd sample.md
```

## Usage

```bash
# Open a file
bmd README.md

# Read from stdin
bmd < some-file.md

# Pipe
some-generator | bmd

# Force Unicode checklist markers
BMD_CHECKLIST_STYLE=unicode bmd notes.md
```

## Keybindings

### Normal mode

| Key | Action |
|-----|--------|
| `j` / `↓` | Scroll down 2 lines |
| `k` / `↑` | Scroll up 2 lines |
| `d` / `PageDown` | Half page down |
| `u` / `PageUp` | Half page up |
| `g` / `G` | Jump to top / bottom |
| `[` / `]` | Previous / next heading |
| `Tab` / `n` | Next visible link (or next search match when search is active) |
| `Shift-Tab` / `N` / `p` | Previous visible link (or previous search match) |
| `o` / `Enter` | Open selected link / preview (`#anchor` jumps in-document) |
| `O` | Close an open preview, or step back one navigation level |
| `/` / `?` | Start forward / backward search |
| `h` / `H` | Show help overlay / close help overlay |
| `x` | Toggle task-list item on top visible line |
| Mouse wheel | Scroll up / down |
| `q` / `Ctrl-c` | Quit (`Esc` clears search when active; else resets anchor or document stack) |
| Left click on checkbox | Toggle task-list item (normal mode) |

### Search input mode

| Key | Action |
|-----|--------|
| Character | Append to query |
| `Backspace` | Delete one character |
| `Enter` | Confirm search |
| `Esc` | Cancel input |

### Preview mode

| Key | Action |
|-----|--------|
| `Esc` / `o` | Close preview |
| `+` / `=` / `-` | Zoom in / out |
| `0` | Reset zoom to fit |
| Ctrl+trackpad pinch | Zoom in / out |
| `q` / `Ctrl-c` | Quit |

## Configuration

Optional settings live in `~/.config/bmd/config.toml` (or `$XDG_CONFIG_HOME/bmd/config.toml`). Missing files use built-in defaults.

### Theme

Pick a built-in preset, then override individual roles on top of that preset:

```toml
[theme]
preset = "nord"   # see table below
```

| Preset | Description |
|--------|-------------|
| `dark` | High-contrast classic terminal palette |
| `cursor-midnight` | Application default (`DEFAULT_PRESET`; used when `preset` is omitted) |
| `light` | Dark text for light terminal backgrounds |
| `solarized-dark` | Ethan Schoonover Solarized (dark) |
| `solarized-light` | Ethan Schoonover Solarized (light) |
| `nord` | Nord frost / aurora palette |
| `gruvbox-dark` | Warm Gruvbox dark |
| `dracula` | Dracula purple-pink accents |
| `tokyo-night` | Tokyo Night editor colors |
| `hackerman-omarchy` | Omarchy Hackerman neon cyan/green on `#0B0C16` |

Each `[theme.<role>]` section overrides only the fields you set on the chosen preset; omitted fields keep the preset value. Set a boolean modifier to `false` to turn it off.

```toml
[theme]
preset = "nord"

[theme.link]
fg = "cyan"        # overrides preset link foreground only

[theme.h1]
underlined = false # removes h1 underline from the preset
```

Supported fields per role: `fg`, `bg`, `bold`, `italic`, `underlined`, `dim`, `reversed`, `crossed_out`. Colors may be named (`white`, `blue`, `darkgray`, …) or hex (`#ff8800`). Roles match theme keys (`text`, `h1`, `link`, `code_block`, …).

### Keymap

Bindings are grouped by mode. Each command accepts one key string or an array of aliases. Modifier prefixes: `C-` (Ctrl), `S-` (Shift), `A-` (Alt). Named keys such as `down`, `enter`, and `pagedown` are supported.

```toml
[keymap.normal]
scroll_down = ["j", "down"]
prev_link = ["N", "backtab", "p"]
prev_heading = "["
next_heading = "]"
toggle_help = "h"
close_help = "H"

[keymap.preview]
preview_zoom_in = ["+", "="]
preview_zoom_out = "-"
preview_zoom_reset = "0"
```

Available commands:

| Mode | Commands |
|------|----------|
| `normal` | `scroll_down`, `scroll_up`, `half_page_down`, `half_page_up`, `jump_to_top`, `jump_to_bottom`, `next_link`, `prev_link`, `next_heading`, `prev_heading`, `open_link`, `nav_back`, `start_search_forward`, `start_search_backward`, `toggle_help`, `close_help`, `toggle_checklist`, `quit` |
| `preview` | `close_preview`, `preview_zoom_in`, `preview_zoom_out`, `preview_zoom_reset`, `quit` |
| `search` | `search_confirm`, `search_cancel`, `search_backspace` |

## Build

Building through devbox sets linker flags, `CARGO_HOME`, and sccache configuration.

```bash
devbox run build          # debug
devbox run build-release  # release
devbox run build-linux-x86_64  # static Linux x86_64 (musl, from macOS)
devbox run package        # release binaries (dist/*.tar.gz) + crates.io crate
```

Without devbox, macOS may require `RUSTFLAGS="-C linker=clang"` when the default `cc` is not Apple clang. Mixing devbox and plain `cargo` invalidates incremental artifacts due to differing `RUSTFLAGS`.

## Development

```bash
devbox shell
devbox run setup        # toolchain and dependencies
devbox run build
devbox run build-release
devbox run package
devbox run test
devbox run run -- sample.md
devbox run clippy
devbox run fmt
devbox run prek         # pre-commit hooks
devbox run cache-stats  # sccache hit rate
```

`devbox.json` configures project-local `RUSTUP_HOME`, `CARGO_HOME`, `SCCACHE_DIR`, `RUSTFLAGS="-C linker=clang"`, and `RUSTC_WRAPPER=sccache`. Artifacts go to `target/` and `dist/`; compile cache to `.sccache/`.

Sandboxed IDE shells may point `CARGO_TARGET_DIR` at a temporary directory, which looks like a clean build every time — prefer building from a normal terminal.

## Architecture

```text
src/
├── main.rs           # entry point and terminal setup
├── app/              # application loop, input, drawing, navigation
├── domain/           # domain model and typed state transitions
├── parse/            # pulldown-cmark → domain model
├── render/           # domain model → ratatui widgets
├── config.rs         # ~/.config/bmd/config.toml loader
├── keymap.rs         # per-mode keybindings (configurable)
├── browser.rs        # macOS open adapter
└── error.rs
```

Design notes are in [`PLAN.md`](PLAN.md) (Japanese).

## License

Apache-2.0
