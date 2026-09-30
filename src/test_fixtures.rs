//! Sample documents shared by render and app tests.

use crate::parse::MarkupFormat;

/// `(name, format, source)` for every bundled sample plus the kitchen-sink fixture.
pub(crate) const SAMPLES: [(&str, MarkupFormat, &str); 5] = [
    (
        "kitchen-sink.md",
        MarkupFormat::Markdown,
        include_str!("../tests/fixtures/kitchen-sink.md"),
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
