//! The drawing's layouts. They are replaced as a list (`set_layouts`), which
//! is what one undo step swaps; the helpers here only read and allocate.

use crate::document::after;
use crate::layout::{Layout, LayoutId, Viewport, ViewportId};
use crate::{Document, Point};

/// Gap between the edge of a new sheet and its first viewport, in millimetres.
const SHEET_MARGIN: f64 = 10.0;

impl Document {
    /// Layouts in tab order.
    pub fn layouts(&self) -> &[Layout] {
        &self.layouts
    }

    pub fn layout(&self, id: LayoutId) -> Option<&Layout> {
        self.layouts.iter().find(|l| l.id == id)
    }

    /// Replace every layout, keeping the id counters ahead of the new ones.
    /// Returns the old list.
    pub fn set_layouts(&mut self, layouts: Vec<Layout>) -> Vec<Layout> {
        for layout in &layouts {
            self.next_layout_id = self.next_layout_id.max(after(layout.id.0));
            for vp in &layout.viewports {
                self.next_viewport_id = self.next_viewport_id.max(after(vp.id.0));
            }
        }
        std::mem::replace(&mut self.layouts, layouts)
    }

    pub fn allocate_layout_id(&mut self) -> LayoutId {
        let id = LayoutId(self.next_layout_id);
        self.next_layout_id = after(self.next_layout_id);
        id
    }

    pub fn allocate_viewport_id(&mut self) -> ViewportId {
        let id = ViewportId(self.next_viewport_id);
        self.next_viewport_id = after(self.next_viewport_id);
        id
    }

    /// The next layout and viewport ids that would be handed out.
    pub fn next_layout_ids(&self) -> (u64, u64) {
        (self.next_layout_id, self.next_viewport_id)
    }

    /// Raise the layout counters so ids of deleted layouts are never reused.
    pub fn reserve_layout_ids(&mut self, next_layout: u64, next_viewport: u64) {
        self.next_layout_id = self.next_layout_id.max(next_layout);
        self.next_viewport_id = self.next_viewport_id.max(next_viewport);
    }

    /// A new sheet with one viewport framing the drawing, numbered from the
    /// counters. Nothing is changed until `set_layouts` is given the result.
    pub fn new_layout(&self, name: impl Into<String>, width: f64, height: f64) -> Layout {
        let (layout, viewport) = self.next_layout_ids();
        let (w, h) = (
            (width - 2.0 * SHEET_MARGIN).max(1.0),
            (height - 2.0 * SHEET_MARGIN).max(1.0),
        );
        Layout {
            id: LayoutId(layout),
            name: name.into(),
            width,
            height,
            viewports: vec![Viewport::framing(
                ViewportId(viewport),
                Point::new(width / 2.0, height / 2.0),
                w,
                h,
                self.model_bounds(),
            )],
        }
    }

    /// A copy of a layout under a new name, with fresh layout and viewport ids.
    pub fn copied_layout(&self, source: LayoutId, name: impl Into<String>) -> Option<Layout> {
        let (layout, viewport) = self.next_layout_ids();
        let mut copy = self.layout(source)?.clone();
        copy.id = LayoutId(layout);
        copy.name = name.into();
        for (i, vp) in copy.viewports.iter_mut().enumerate() {
            vp.id = ViewportId(viewport + i as u64);
        }
        Some(copy)
    }

    /// What is drawn on a layout's sheet, as copies ready for another sheet.
    pub fn copied_sheet_objects(
        &self,
        layout: LayoutId,
    ) -> Vec<(crate::Entity, crate::Attributes)> {
        self.objects()
            .iter()
            .filter(|o| o.layout == Some(layout))
            .map(|o| (o.entity.clone(), o.attrs.clone()))
            .collect()
    }

    /// Whether a layout is already called `name` (ignoring case), other than `except`.
    pub fn layout_name_taken(&self, name: &str, except: Option<LayoutId>) -> bool {
        self.layouts
            .iter()
            .any(|l| Some(l.id) != except && l.name.eq_ignore_ascii_case(name))
    }

    /// A name no layout has: "Layout1", "Layout2", ...
    pub fn unused_layout_name(&self) -> String {
        (1..)
            .map(|n| format!("Layout{n}"))
            .find(|name| !self.layout_name_taken(name, None))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Viewport;
    use crate::Point;

    fn layout(doc: &mut Document, name: &str) -> Layout {
        let vp = doc.allocate_viewport_id();
        Layout {
            id: doc.allocate_layout_id(),
            name: name.into(),
            width: 297.0,
            height: 210.0,
            viewports: vec![Viewport::framing(
                vp,
                Point::new(148.0, 105.0),
                250.0,
                170.0,
                None,
            )],
        }
    }

    #[test]
    fn layout_names_are_unique_ignoring_case() {
        let mut doc = Document::default();
        assert_eq!(doc.unused_layout_name(), "Layout1");
        let a = layout(&mut doc, "Layout1");
        let id = a.id;
        doc.set_layouts(vec![a]);
        assert!(doc.layout_name_taken("LAYOUT1", None));
        assert!(!doc.layout_name_taken("layout1", Some(id)));
        assert_eq!(doc.unused_layout_name(), "Layout2");
    }

    #[test]
    fn new_and_copied_layouts_take_fresh_ids_and_stay_valid() {
        let mut doc = Document::default();
        let first = doc.new_layout("Layout1", 297.0, 210.0);
        assert_eq!(
            first.viewports[0].paper_bounds().min,
            Point::new(10.0, 10.0)
        );
        doc.set_layouts(vec![first.clone()]);
        let copy = doc.copied_layout(first.id, "Layout2").unwrap();
        assert_ne!(copy.id, first.id);
        assert_ne!(copy.viewports[0].id, first.viewports[0].id);
        assert_eq!(copy.viewports[0].scale, first.viewports[0].scale);
        doc.set_layouts(vec![first, copy]);
        assert_eq!(doc.validate(), Ok(()));
        assert!(doc.copied_layout(LayoutId(99), "x").is_none());
    }

    #[test]
    fn set_layouts_swaps_and_keeps_counters_ahead() {
        let mut doc = Document::default();
        let a = layout(&mut doc, "A");
        let (lid, vid) = (a.id, a.viewports[0].id);
        assert!(doc.set_layouts(vec![a.clone()]).is_empty());
        assert_eq!(doc.layout(lid), Some(&a));
        assert_eq!(doc.set_layouts(Vec::new()), vec![a]);
        assert!(
            doc.allocate_layout_id() > lid,
            "ids of removed layouts stay retired"
        );
        assert!(doc.allocate_viewport_id() > vid);
    }
}
