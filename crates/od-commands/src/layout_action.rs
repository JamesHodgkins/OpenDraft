//! Undoable changes to a drawing's layouts and their viewports.

use od_core::{Document, Layout, PlotSettings};

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

/// Replace the Model tab's plot settings. One undo step swaps them back.
pub struct SetModelPlot {
    name: String,
    settings: PlotSettings,
}

impl SetModelPlot {
    pub fn new(name: impl Into<String>, settings: PlotSettings) -> Self {
        Self {
            name: name.into(),
            settings,
        }
    }

    fn swap(&mut self, doc: &mut Document) {
        let new = std::mem::replace(&mut self.settings, PlotSettings::for_model());
        self.settings = doc.set_model_plot(new);
    }
}

impl Action for SetModelPlot {
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
    fn model_plot_settings_change_and_come_back() {
        let mut doc = Document::default();
        let before = doc.model_plot().clone();
        let mut changed = before.clone();
        changed.margins_mm = [5.0; 4];
        let mut action = SetModelPlot::new("Page setup", changed.clone());
        action.apply(&mut doc);
        assert_eq!(doc.model_plot(), &changed);
        action.revert(&mut doc);
        assert_eq!(doc.model_plot(), &before);
        action.apply(&mut doc);
        assert_eq!(doc.model_plot(), &changed);
        assert_eq!(doc.validate(), Ok(()));
    }

    #[test]
    fn layouts_change_and_come_back() {
        let mut doc = Document::default();
        let vp = doc.allocate_viewport_id();
        let layout = Layout {
            id: doc.allocate_layout_id(),
            name: "Sheet".into(),
            width: 297.0,
            height: 210.0,
            plot: od_core::PlotSettings::for_layout(297.0, 210.0),
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
