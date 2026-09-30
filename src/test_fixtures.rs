//! Sample documents shared by render and app tests.

use crate::parse::MarkupFormat;

/// `(name, format, source)` for every bundled sample plus one kitchen-sink fixture per format.
pub(crate) const SAMPLES: [(&str, MarkupFormat, &str); 7] = [
    (
        "kitchen-sink.md",
        MarkupFormat::Markdown,
        include_str!("../tests/fixtures/kitchen-sink.md"),
    ),
    (
        "kitchen-sink.adoc",
        MarkupFormat::AsciiDoc,
        include_str!("../tests/fixtures/kitchen-sink.adoc"),
    ),
    (
        "kitchen-sink.rst",
        MarkupFormat::Rest,
        include_str!("../tests/fixtures/kitchen-sink.rst"),
    ),
    (
        "sample.md",
        MarkupFormat::Markdown,
        include_str!("../sample.md"),
    ),
    (
        "sample-gfm.md",
        MarkupFormat::Markdown,
        include_str!("../sample-gfm.md"),
    ),
    (
        "sample.rst",
        MarkupFormat::Rest,
        include_str!("../sample.rst"),
    ),
    (
        "sample.adoc",
        MarkupFormat::AsciiDoc,
        include_str!("../sample.adoc"),
    ),
];
