//! Undoable change of the dimension style new dimensions are made with.

use od_core::{DimStyle, Document};

use crate::{Action, Changes};

/// Replace the drawing's current dimension style (the DIM variables).
pub struct SetDimStyle {
    /// Applying and reverting both swap, so each leaves the other state ready.
    style: DimStyle,
}

impl SetDimStyle {
    pub fn new(style: DimStyle) -> Self {
        Self {
            style: style.sanitized(),
        }
    }

    fn swap(&mut self, doc: &mut Document) {
        self.style = doc.set_dim_style(self.style.clone());
    }
}

impl Action for SetDimStyle {
    fn name(&self) -> String {
        "Dimension style".to_owned()
    }

    fn apply(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn revert(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn changes(&self) -> Changes {
        Changes::dim_style()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_style_changes_and_comes_back() {
        let mut doc = Document::default();
        let mut action = SetDimStyle::new(DimStyle {
            text_height: 5.0,
            text_above: 99,
            ..DimStyle::default()
        });
        action.apply(&mut doc);
        assert_eq!(doc.dim_style().text_height, 5.0);
        assert_eq!(doc.dim_style().text_above, 4, "kept in range");
        action.revert(&mut doc);
        assert_eq!(*doc.dim_style(), DimStyle::default());
    }
}
