//! Undoable changes to a drawing's layouts and their viewports.

use od_core::{Document, Layout};

use crate::{Action, Changes};

/// Replace the whole list of layouts (new, rename, delete, a viewport added,
/// moved, scaled or locked). One undo step swaps the list back.
pub struct SetLayouts {
    name: String,
    /// Applying and reverting both swap, so each leaves the other state ready.
    layouts: Vec<Layout>,
}

impl SetLayouts {
    pub fn new(name: impl Into<String>, layouts: Vec<Layout>) -> Self {
        Self {
            name: name.into(),
            layouts,
        }
    }

    fn swap(&mut self, doc: &mut Document) {
        self.layouts = doc.set_layouts(std::mem::take(&mut self.layouts));
    }
}

impl Action for SetLayouts {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn apply(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn revert(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn changes(&self) -> Changes {
        Changes::layouts()
    }
}

#[cfg(test)]
mod tests {
    use od_core::{LayoutId, Point, Viewport};

    use super::*;

    #[test]
    fn layouts_change_and_come_back() {
        let mut doc = Document::default();
        let vp = doc.allocate_viewport_id();
        let layout = Layout {
            id: doc.allocate_layout_id(),
            name: "Sheet".into(),
            width: 297.0,
            height: 210.0,
            viewports: vec![Viewport::framing(
                vp,
                Point::new(1.0, 1.0),
                10.0,
                10.0,
                None,
            )],
        };
        let id: LayoutId = layout.id;
        let mut action = SetLayouts::new("New layout", vec![layout]);
        action.apply(&mut doc);
        assert_eq!(doc.layouts().len(), 1);
        action.revert(&mut doc);
        assert!(doc.layouts().is_empty());
        action.apply(&mut doc);
        assert_eq!(doc.layouts()[0].id, id);
        assert_eq!(action.changes(), Changes::layouts());
        assert_eq!(doc.validate(), Ok(()));
    }
}
