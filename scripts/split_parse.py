#!/usr/bin/env python3
"""Split src/parse.rs into parse/ submodules."""

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"
PARSE = SRC / "parse"
PARSE.mkdir(exist_ok=True)

lines = (SRC / "parse.rs").read_text().splitlines(keepends=True)


def extract(start: int, end: int) -> str:
    return "".join(lines[start - 1 : end])


files = {
    "html.rs": extract(11, 129),
    "inline.rs": extract(173, 348),
    "block.rs": extract(138, 171) + extract(350, 812),
    "tests.rs": extract(816, len(lines)),
}

headers = {
    "html.rs": "//! Inline HTML token parsing.\n\n",
    "inline.rs": (
        "//! Inline content accumulation.\n\n"
        "use crate::domain::{Inline, Link, LinkId, LinkUrl};\n"
        "use crate::error::AppError;\n\n"
    ),
    "block.rs": (
        "//! Block-level markdown event handling.\n\n"
        "use pulldown_cmark::{Alignment as CmarkAlignment, Event, Parser, Tag, TagEnd};\n\n"
        "use crate::domain::{\n"
        "    Alignment, Block, CodeBlock, Document, Heading, HeadingLevel, Inline, List, ListItem,\n"
        "    MermaidDiagram, Table,\n"
        "};\n"
        "use crate::error::AppError;\n\n"
        "use super::html::{InlineHtmlKind, InlineHtmlToken};\n"
        "use super::inline::InlineParser;\n\n"
    ),
    "tests.rs": (
        "use super::parse;\n"
        "use crate::domain::{Block, Document, Inline};\n\n"
    ),
}

pub_crate = {
    "html.rs": ["InlineHtmlToken", "InlineHtmlKind", "extract_href", "parse_tag"],
    "inline.rs": ["InlineParser", "InlineFrame"],
}

# After visibility pass, inline.rs gets all impl methods pub(crate); block.rs gets
# ParserState API surface pub(crate). Applied via post-process below.


def vis(body: str, fname: str) -> str:
    out = []
    for line in body.splitlines(keepends=True):
        s = line.lstrip()
        if s.startswith("pub ") or s.startswith("#["):
            out.append(line)
            continue
        if s.startswith("enum ") or s.startswith("struct ") or s.startswith("fn "):
            name = s.split("(")[0].split("{")[0].split(" ")[1]
            if name in pub_crate.get(fname, []):
                if s.startswith("enum "):
                    line = line.replace("enum ", "pub(crate) enum ", 1)
                elif s.startswith("struct "):
                    line = line.replace("struct ", "pub(crate) struct ", 1)
                elif s.startswith("fn "):
                    line = line.replace("fn ", "pub(crate) fn ", 1)
        out.append(line)
    return "".join(out)


for name, body in files.items():
    (PARSE / name).write_text(headers[name] + vis(body, name))

mod_rs = (
    "//! Markdown parser adapter: pulldown-cmark events -> domain model.\n\n"
    "mod block;\n"
    "mod html;\n"
    "mod inline;\n\n"
    "#[cfg(test)]\n"
    "mod tests;\n\n"
    "use pulldown_cmark::{Options, Parser};\n\n"
    "use crate::domain::Document;\n"
    "use crate::error::AppError;\n\n"
    "use block::ParserState;\n\n"
    + extract(130, 136)
)
(PARSE / "mod.rs").write_text(mod_rs)
(SRC / "parse.rs").unlink()
print("split parse.rs")
