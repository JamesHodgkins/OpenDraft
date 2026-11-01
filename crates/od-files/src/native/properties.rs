//! How a drawing's properties (title, author...) are written to the file.

use od_core::drawing_properties::DrawingProperties;
use serde::{Deserialize, Serialize};

use super::entity::invalid;
use crate::FileError;

/// Left out of the file when empty; each value is left out when empty too.
#[derive(Serialize, Deserialize, Default)]
pub(super) struct PropertiesDto {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    subject: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    author: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    keywords: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    comments: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    hyperlink_base: String,
}

impl PropertiesDto {
    pub(super) fn is_empty(&self) -> bool {
        self.title.is_empty()
            && self.subject.is_empty()
            && self.author.is_empty()
            && self.keywords.is_empty()
            && self.comments.is_empty()
            && self.hyperlink_base.is_empty()
    }

    pub(super) fn of(p: &DrawingProperties) -> Self {
        Self {
            title: p.title.clone(),
            subject: p.subject.clone(),
            author: p.author.clone(),
            keywords: p.keywords.clone(),
            comments: p.comments.clone(),
            hyperlink_base: p.hyperlink_base.clone(),
        }
    }

    /// The properties, refused if a value is over the limits.
    pub(super) fn into_properties(self) -> Result<DrawingProperties, FileError> {
        let properties = DrawingProperties {
            title: self.title,
            subject: self.subject,
            author: self.author,
            keywords: self.keywords,
            comments: self.comments,
            hyperlink_base: self.hyperlink_base,
        };
        match properties.validate() {
            Ok(()) => Ok(properties),
            Err(why) => invalid(why),
        }
    }
}
