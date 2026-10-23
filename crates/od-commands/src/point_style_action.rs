//! Undoable change of how point objects are drawn.

use od_core::{Document, PointStyle};

use crate::{Action, Changes};

/// Replace the drawing's `PDMODE` and `PDSIZE`.
pub struct SetPointStyle {
    /// Applying and reverting both swap, so each leaves the other state ready.
    style: PointStyle,
}

impl SetPointStyle {
    pub fn new(style: PointStyle) -> Self {
        Self { style }
    }

    fn swap(&mut self, doc: &mut Document) {
        self.style = doc.set_point_style(self.style);
    }
}

impl Action for SetPointStyle {
    fn name(&self) -> String {
        "Point style".to_owned()
    }

    fn apply(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn revert(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn changes(&self) -> Changes {
        Changes::point_style()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_style_changes_and_comes_back() {
        let mut doc = Document::default();
        let mut action = SetPointStyle::new(PointStyle {
            mode: 35,
            size: 2.0,
        });
        action.apply(&mut doc);
        assert_eq!(doc.point_style().mode, 35);
        action.revert(&mut doc);
        assert_eq!(doc.point_style(), PointStyle::default());
        action.apply(&mut doc);
        assert_eq!(doc.point_style().size, 2.0);
        assert_eq!(action.changes(), Changes::point_style());
    }
}
