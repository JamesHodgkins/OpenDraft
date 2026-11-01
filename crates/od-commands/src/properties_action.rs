//! Undoable change of a drawing's properties (title, author and the rest).

use od_core::drawing_properties::DrawingProperties;
use od_core::Document;

use crate::{Action, Changes};

/// Replace the drawing's properties (the DWGPROPS dialog).
pub struct SetProperties {
    /// Applying and reverting both swap, so each leaves the other state ready.
    properties: DrawingProperties,
}

impl SetProperties {
    pub fn new(properties: DrawingProperties) -> Self {
        Self {
            properties: properties.sanitized(),
        }
    }

    fn swap(&mut self, doc: &mut Document) {
        self.properties = doc.set_properties(self.properties.clone());
    }
}

impl Action for SetProperties {
    fn name(&self) -> String {
        "Drawing Properties".to_owned()
    }

    fn apply(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn revert(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn changes(&self) -> Changes {
        Changes::properties()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn properties_change_and_come_back() {
        let mut doc = Document::default();
        let mut action = SetProperties::new(DrawingProperties {
            title: "Plan\n7".into(),
            comments: "kept\nlines".into(),
            ..DrawingProperties::default()
        });
        action.apply(&mut doc);
        assert_eq!(doc.properties().title, "Plan 7", "a title is one line");
        assert_eq!(doc.properties().comments, "kept\nlines");
        action.revert(&mut doc);
        assert!(doc.properties().is_empty());
        action.apply(&mut doc);
        assert_eq!(doc.properties().title, "Plan 7");
    }
}
