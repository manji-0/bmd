#!/usr/bin/env python3
"""Split src/domain.rs into domain/ submodules aligned with dagayn communities."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"
DOMAIN = SRC / "domain"
DOMAIN.mkdir(exist_ok=True)

lines = (SRC / "domain.rs").read_text().splitlines(keepends=True)


def extract(start: int, end: int) -> str:
    return "".join(lines[start - 1 : end])


files = {
    "markdown.rs": extract(12, 389),
    "link.rs": extract(390, 437),
    "view.rs": extract(438, 915),
    "tests.rs": extract(918, len(lines)),
}

headers = {
    "markdown.rs": (
        "//! Markdown document and block model.\n\n"
        "use unicode_width::UnicodeWidthStr;\n\n"
        "use super::link::{DocumentError, Link, LinkId, LinkUrl};\n\n"
    ),
    "link.rs": (
        "//! Link value objects and validation errors.\n\n"
        "use std::fmt;\n\n"
    ),
    "view.rs": (
        "//! View, scroll, and search state with typed transitions.\n\n"
        "use super::link::LinkId;\n"
        "use super::markdown::Document;\n\n"
    ),
    "tests.rs": (
        "use super::link::{LinkUrl, LinkUrlError};\n"
        "use super::markdown::{\n"
        "    Alignment, Block, CodeBlock, Document, DocumentError, Inline, List, ListItem, Table,\n"
        "};\n"
        "use super::view::{\n"
        "    SearchDirection, SearchMatch, SearchQuery, SearchQueryError, TerminalSize,\n"
        "    TerminalSizeError, ViewState,\n"
        "};\n\n"
    ),
}

for name, body in files.items():
    (DOMAIN / name).write_text(headers[name] + body)

(DOMAIN / "mod.rs").write_text(
    """//! Domain model for the TUI markdown viewer.
//!
//! Invalid states and invalid transitions are modelled out of the type system where practical:
//! - `LinkUrl` cannot be empty.
//! - `TerminalSize` cannot have zero dimensions.
//! - `ViewState` transitions consume `self`, so the old state cannot be reused.

mod link;
mod markdown;
mod view;

#[cfg(test)]
mod tests;

pub use link::{DocumentError, Link, LinkId, LinkUrl, LinkUrlError};
pub use markdown::{
    Alignment, Block, CodeBlock, Document, Heading, HeadingLevel, Inline, List, ListItem,
    MermaidDiagram, Table,
};
pub use view::{
    Scroll, SearchDirection, SearchMatch, SearchQuery, SearchQueryError, SearchState,
    TerminalSize, TerminalSizeError, ViewState,
};
"""
)

(SRC / "domain.rs").unlink()
print("split domain.rs")
