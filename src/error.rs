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

    #[error("unsupported input: {0}")]
    UnsupportedInput(String),
}
