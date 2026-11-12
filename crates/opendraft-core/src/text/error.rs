//! Why contents could not be read.

use std::fmt;

/// The resource a limit protects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextResource {
    ContentBytes,
    Nodes,
    Paragraphs,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextError {
    /// The contents are bigger than a limit allows.
    ResourceLimit {
        resource: TextResource,
        limit: usize,
    },
}

impl fmt::Display for TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceLimit { resource, limit } => {
                let what = match resource {
                    TextResource::ContentBytes => "bytes of text",
                    TextResource::Nodes => "formatting elements",
                    TextResource::Paragraphs => "paragraphs",
                };
                write!(f, "the text has more than {limit} {what}")
            }
        }
    }
}

impl std::error::Error for TextError {}
