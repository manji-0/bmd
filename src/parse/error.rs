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

    pub fn invalid_heading_level(format: MarkupFormat, level: u8) -> Self {
        Self::InvalidHeadingLevel { format, level }
    }

    pub(crate) fn ensure_heading_level(format: MarkupFormat, level: u8) -> Result<(), Self> {
        if (1..=6).contains(&level) {
            Ok(())
        } else {
            Err(Self::invalid_heading_level(format, level))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MarkupFormat;
    use super::ParseError;

    #[test]
    fn ensure_heading_level_accepts_valid_levels() {
        assert!(ParseError::ensure_heading_level(MarkupFormat::Markdown, 1).is_ok());
    }

    #[test]
    fn ensure_heading_level_rejects_invalid_levels() {
        assert!(matches!(
            ParseError::ensure_heading_level(MarkupFormat::Rest, 9),
            Err(ParseError::InvalidHeadingLevel {
                format: MarkupFormat::Rest,
                level: 9,
            })
        ));
    }
}
