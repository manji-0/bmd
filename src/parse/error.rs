//! Parse-layer errors before domain validation.

use super::format::MarkupFormat;

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("{format}: {message}")]
    Syntax {
        format: MarkupFormat,
        message: String,
    },

    #[error("{format}: invalid heading level {level}")]
    InvalidHeadingLevel { format: MarkupFormat, level: u8 },
}

impl ParseError {
    pub fn syntax(format: MarkupFormat, message: impl Into<String>) -> Self {
        Self::Syntax {
            format,
            message: message.into(),
        }
    }
}
