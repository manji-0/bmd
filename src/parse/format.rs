//! Markup format detection.

use std::fmt;
use std::path::Path;

/// Supported markup language for document parsing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MarkupFormat {
    Markdown,
    Rest,
    AsciiDoc,
}

impl fmt::Display for MarkupFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Markdown => write!(f, "markdown"),
            Self::Rest => write!(f, "reST"),
            Self::AsciiDoc => write!(f, "asciidoc"),
        }
    }
}

impl MarkupFormat {
    /// Infer format from a file path extension.
    pub fn from_path(path: &Path) -> Option<Self> {
        match path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("md") | Some("markdown") | Some("mdown") => Some(Self::Markdown),
            Some("rst") | Some("rest") => Some(Self::Rest),
            Some("adoc") | Some("asciidoc") | Some("asc") => Some(Self::AsciiDoc),
            _ => None,
        }
    }

    /// Detect format from path and, when inconclusive, from content heuristics.
    pub fn detect(path: Option<&Path>, content: &str) -> Self {
        if let Some(path) = path
            && let Some(format) = Self::from_path(path)
        {
            return format;
        }
        Self::sniff_content(content)
    }

    fn sniff_content(content: &str) -> Self {
        let lines: Vec<&str> = content
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .take(4)
            .collect();
        let Some(first) = lines.first().copied() else {
            return Self::Markdown;
        };
        if is_asciidoc_heading(first) {
            return Self::AsciiDoc;
        }
        if first.starts_with(".. ") {
            return Self::Rest;
        }
        if is_rest_field_line(first) {
            return Self::Rest;
        }
        if is_rest_adornment_line(first) {
            return Self::Rest;
        }
        if lines
            .get(1)
            .is_some_and(|line| is_rest_adornment_line(line))
        {
            return Self::Rest;
        }
        Self::Markdown
    }
}

fn is_asciidoc_heading(line: &str) -> bool {
    let line = line.trim();
    if !line.starts_with('=') {
        return false;
    }
    let rest = line.trim_start_matches('=');
    rest.starts_with(' ') && !rest.trim().is_empty()
}

fn is_rest_adornment_line(line: &str) -> bool {
    let line = line.trim();
    let Some(ch) = line.chars().next() else {
        return false;
    };
    line.len() >= 3 && line.chars().all(|c| c == ch) && ch.is_ascii_punctuation()
}

fn is_rest_field_line(line: &str) -> bool {
    let line = line.trim();
    line.starts_with(':') && line[1..].contains(':')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn from_path_recognizes_extensions() {
        assert_eq!(
            MarkupFormat::from_path(Path::new("readme.md")),
            Some(MarkupFormat::Markdown)
        );
        assert_eq!(
            MarkupFormat::from_path(Path::new("guide.rst")),
            Some(MarkupFormat::Rest)
        );
        assert_eq!(
            MarkupFormat::from_path(Path::new("book.adoc")),
            Some(MarkupFormat::AsciiDoc)
        );
        assert_eq!(MarkupFormat::from_path(Path::new("notes.txt")), None);
    }

    #[test]
    fn sniff_content_detects_asciidoc_and_rest() {
        assert_eq!(
            MarkupFormat::sniff_content("= Title\n\nBody"),
            MarkupFormat::AsciiDoc
        );
        assert_eq!(
            MarkupFormat::sniff_content("== Section\n\nBody"),
            MarkupFormat::AsciiDoc
        );
        assert_eq!(
            MarkupFormat::sniff_content(".. note::\n\n   text"),
            MarkupFormat::Rest
        );
        assert_eq!(
            MarkupFormat::sniff_content("Title\n=====\n\nBody"),
            MarkupFormat::Rest
        );
        assert_eq!(
            MarkupFormat::sniff_content(":Author: Jane Doe\n\nBody"),
            MarkupFormat::Rest
        );
    }
}
