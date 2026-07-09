//! Error types for the markdown viewer.

use crate::domain::{DocumentError, LinkUrlError, TerminalSizeError};

/// Top-level application error.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("terminal size error: {0}")]
    TerminalSize(#[from] TerminalSizeError),

    #[error("terminal setup error: {0}")]
    TerminalSetup(String),

    #[error("invalid document: {0}")]
    Document(#[from] DocumentError),

    #[error("invalid link URL: {0}")]
    LinkUrl(#[from] LinkUrlError),

    #[error("markup parse error: {0}")]
    MarkupParse(String),

    #[error("markdown parse error: {0}")]
    MarkdownParse(String),

    #[error("mermaid render error: {0}")]
    MermaidRender(#[from] merman::render::raster::RasterError),

    #[error("mermaid source did not produce a diagram")]
    MermaidNoDiagram,

    #[error("image decode error: {0}")]
    ImageDecode(#[from] image::ImageError),

    #[error("terminal image error: {0}")]
    TerminalImage(String),

    #[error("no link is selected")]
    NoLinkSelected,

    #[error("failed to open link: {0}")]
    OpenLink(String),

    #[error("clipboard error: {0}")]
    Clipboard(String),

    #[error("unsupported input: {0}")]
    UnsupportedInput(String),

    #[error("GitHub fetch error: {0}")]
    GitHubFetch(String),
}

impl From<std::convert::Infallible> for AppError {
    fn from(value: std::convert::Infallible) -> Self {
        match value {}
    }
}

impl From<crate::parse::IntoDomainError> for AppError {
    fn from(value: crate::parse::IntoDomainError) -> Self {
        match value {
            crate::parse::IntoDomainError::Document(e) => Self::Document(e),
            crate::parse::IntoDomainError::LinkUrl(e) => Self::LinkUrl(e),
            crate::parse::IntoDomainError::InvalidHeadingLevel { level } => {
                Self::MarkupParse(format!("invalid heading level {level}"))
            }
        }
    }
}

impl From<crate::parse::ParseError> for AppError {
    fn from(value: crate::parse::ParseError) -> Self {
        Self::MarkupParse(value.to_string())
    }
}
