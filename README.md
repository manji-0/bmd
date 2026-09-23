# bmd

A terminal TUI for reading Markdown. Vim-style keybindings, rich markup rendering, native Mermaid diagrams, in-document search, sticky outline, scroll marks, and yank.

## Features

### Markdown rendering

Documents parsed with [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark) are drawn with [ratatui](https://github.com/ratatui/ratatui). Text wraps to the terminal width; only blocks in the visible scroll region are rendered.

- **Headings** — H1–H6 with per-level styles and `#` prefix markers
- **Paragraphs** — bold, italic, inline code, hard breaks
- **Code blocks** — syntax highlighting via [syntect](https://github.com/trishume/syntect) with a language label
- **Block quotes** — nested block quotes supported
- **Lists** — ordered and unordered, including nested lists and GFM task markers (`- [ ]` / `- [x]`)
- **Tables** — column widths adapt to terminal width; cells wrap internally
- **Horizontal rules** — `---` and similar rule lines

### Vim-style navigation

Scroll position is tracked in logical lines; the on-screen position is animated with exponential smoothing. Holding `j` / `k` follows the OS key repeat with rate limiting.

| Action | Keys |
|--------|------|
| Scroll down / up 2 lines | `j` `↓` / `k` `↑` |
| Half page down / up | `d` `PageDown` / `u` `PageUp` |
| Jump to top / bottom | `g` / `G` |
| Previous / next heading | `[` / `]` |
| Toggle outline sidebar | `t` |
| Set / jump scroll mark | `ma` / `'a` (`a`–`z`) |
| Quit | `q` `Ctrl-c` |

### Outline

Press `t` to pin a heading outline on the left. The sidebar stays visible while you scroll the document (`j` / `k` and other keys keep working on the document). Selection tracks the heading under the current scroll position; click an outline entry to jump. Press `t` again to close it.

### Yank

Press `y` to copy an active text selection. With no selection, `y` waits for a second key: `l` copies the selected link URL, `h` copies the current heading as `#slug`, `c` copies the nearest visible code block, and `y` retries selection copy.

### In-document search

Press `/` for forward search or `?` for backward search. A prompt appears at the bottom of the screen; as you type, matching lines are highlighted and the prompt shows a live match count. Press `Enter` to confirm, jump to the nearest match, and return to normal mode. Matches are highlighted in yellow; the current match is emphasized in magenta.

- Case-insensitive substring matching
- Searches plain text across paragraphs, code blocks, lists, block quotes, and tables
- After confirming, `n` / `N` (or `Tab` / `Shift-Tab`) move between matches and scroll to the matching line
- `Esc` while search is active clears the search (does not quit)

### Links and preview

Cycle through links in the document with `n` / `N` (or `Tab` / `Shift-Tab`). Selection walks the whole document in order, scrolls to each link, and wraps from last to first (and vice versa). Footnote markers are not in that cycle — click a `[^ref]` marker to open its preview.

| Type | Example | `o` / `Enter` |
|------|---------|---------------|
| Web | `[text](https://…)` | Opens in the browser via macOS `open` / Linux `xdg-open` |
| Anchor | `[text](#section)` | Jumps to the matching heading; prior scroll positions are stacked |
| Document | `[text](./other.md)` | Opens the linked file in the same view; file stack supports nested navigation |
| Image | `![alt](path.png)` | Floating in-terminal preview |
| Mermaid | Link from a mermaid code block | Floating preview of the rendered diagram |

Close the preview overlay with `Esc`, `o`, or `O`. **One back model:** `Esc` and `O` always dismiss one layer — close an open preview, or step back one navigation jump (in-document `#anchor` first, then the previous file). The status bar shows where you went (`back → README.md` or `back → previous position`). Press again to continue stepping; there is no separate “reset both stacks” key by default (bind `nav_reset` in config if you want a full drain). Each stack keeps the live current section or file outside the stack; following a link fixes the prior position/document once at jump time (scrolling and other navigation never update stored priors). Both stacks count the current item as layer 1 and support up to 64 layers. Further link jumps beyond that limit show a status-bar message and leave the current view unchanged. Web links are blue; image and Mermaid links are magenta. The selected link is shown inverted.

### Mermaid and images

Mermaid fenced code blocks are rasterized with the pure-Rust [merman](https://crates.io/crates/merman) crate and displayed inline using the terminal graphics protocol.

- Queries terminal capabilities at startup (Kitty, iTerm2, Sixel, etc.); falls back to Unicode half-blocks when unsupported
- Pauses image drawing while scrolling; resumes 100 ms after scrolling stops
- Markdown images with relative paths resolve against the input file's directory

### Other

- **Task markers** — GFM `- [ ]` / `- [x]` render as checkboxes; left-click toggles for the session only (not saved). Marker style via `BMD_CHECKLIST_STYLE` (`unicode` / `emoji` / `auto`)
- **Type-safe domain model** — Kamae-style state transitions (`ViewState` methods consume `self`)
- **Document render cache** — full document buffered until width or highlight state changes; scrolling only blits the viewport
- **stdin / file input** — path argument, `-`, or pipe; file paths reload automatically on save (scroll position preserved)
- **Debug** — `BMD_DEBUG=1` logs key events and commands to stderr

## Requirements

- [devbox](https://www.jetify.com/devbox) (recommended; provides Rust 1.98, clang, sccache, prek)
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
| `t` | Toggle outline sidebar (tracks scroll; click entry to jump) |
| `m` then `a`–`z` | Set scroll mark |
| `'` then `a`–`z` | Jump to scroll mark |
| `Tab` / `n` | Next link in the document, scrolling to it (or next search match when search is active) |
| `Shift-Tab` / `N` | Previous link in the document, scrolling to it (or previous search match) |
| `o` / `Enter` | Open selected link / preview (`#anchor` jumps in-document) |
| `O` / `Esc` | One step back: close preview or previous jump/file (status: `back → …`) |
| `/` / `?` | Start forward / backward search |
| `h` | Show help overlay (`Esc` closes) |
| `y` | Copy text selection, or start yank (`yl` link, `yh` heading, `yc` code, `yy` selection) |
| Mouse wheel | Scroll up / down |
| `q` / `Ctrl-c` | Quit (`Esc` clears search when active; otherwise same one-step back as `O`) |
| Left click on link | Open link / preview |
| Left click on footnote marker | Open footnote preview |
| Left click on checkbox | Toggle task marker for this session (not saved) |
| Left click on outline | Jump to heading |
| Drag | Select text (highlight only; press `y` to copy) |

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
| `Esc` / `o` / `O` | Close preview |
| `+` / `=` / `-` | Zoom in / out |
| `0` | Reset zoom to fit |
| Ctrl+trackpad pinch | Zoom in / out |
| Left click outside preview | Close preview |
| `q` / `Ctrl-c` | Quit |

## Configuration

Optional settings live in `~/.config/bmd/config.toml` (or `$XDG_CONFIG_HOME/bmd/config.toml`). Missing files use built-in defaults.

### Theme

Pick one of the three built-in presets, then override individual roles for a custom palette:

```toml
[theme]
preset = "dark"   # or light / cursor-midnight (default when omitted)
```

| Preset | Description |
|--------|-------------|
| `cursor-midnight` | Application default (`DEFAULT_PRESET`; used when `preset` is omitted) |
| `dark` | High-contrast classic terminal palette |
| `light` | Dark text for light terminal backgrounds |

Further skins are not shipped as presets — use `[theme.<role>]` overrides on top of a base. Each override section replaces only the fields you set; omitted fields keep the preset value. Set a boolean modifier to `false` to turn it off.

```toml
[theme]
preset = "dark"

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
prev_link = ["N", "backtab"]
prev_heading = "["
next_heading = "]"
toggle_help = "h"
toggle_outline = "t"
yank_prefix = "y"

[keymap.preview]
preview_zoom_in = ["+", "="]
preview_zoom_out = "-"
preview_zoom_reset = "0"
```

Available commands:

| Mode | Commands |
|------|----------|
| `normal` | `scroll_down`, `scroll_up`, `half_page_down`, `half_page_up`, `jump_to_top`, `jump_to_bottom`, `next_link`, `prev_link`, `next_heading`, `prev_heading`, `open_link`, `nav_back`, `nav_reset`, `start_search_forward`, `start_search_backward`, `toggle_help`, `toggle_outline`, `yank_prefix`, `copy_selection`, `quit` (optional: `close_help`, `toggle_checklist`, unbound by default) |
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
