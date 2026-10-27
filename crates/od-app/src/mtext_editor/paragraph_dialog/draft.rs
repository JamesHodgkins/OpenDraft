//! What the Paragraph dialog holds while it is open: the values as AutoCAD
//! shows them, and the one `ParagraphEdit` that OK turns them into. No egui.
//!
//! AutoCAD's "First line" and "Hanging" are both positions measured from the
//! left edge: the first line starts at one and the other lines at the other.
//! The text stores the left indent and the first line relative to it, so
//! `first_line = left + first`, `hanging = left`.

use od_core::text::{ParagraphAlignment, ParagraphEdit, ParagraphStyle};

/// Most tab stops a paragraph may carry (the core's limit).
const MAX_TABS: usize = 32;

/// The dialog's values. Cancel throws it away; OK applies [`Draft::edit`].
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    /// Where the paragraph looked when the draft was made, so the dialog
    /// can tell the caret has moved to a different paragraph.
    pub seed: ParagraphStyle,
    pub first_line: f64,
    pub hanging: f64,
    /// Ascending, no repeats.
    pub tabs: Vec<f64>,
    pub selected_tab: Option<usize>,
    /// The position typed in the Tab box.
    pub tab_position: f64,
    /// "Paragraph Alignment" is ticked.
    pub align_on: bool,
    /// One of left, centre, right, justified.
    pub alignment: ParagraphAlignment,
    /// "Paragraph Spacing" is ticked.
    pub spacing_on: bool,
    pub before: f64,
    pub after: f64,
    /// "Paragraph Line Spacing" is ticked; only the Multiple kind is known.
    pub line_on: bool,
    pub multiple: f64,
}

impl Draft {
    pub fn of(style: &ParagraphStyle) -> Self {
        let align_on = !matches!(
            style.alignment,
            ParagraphAlignment::Default | ParagraphAlignment::Distributed
        );
        Self {
            seed: style.clone(),
            first_line: style.left_indent + style.first_indent,
            hanging: style.left_indent,
            tabs: style.tabs.iter().map(|t| t.offset).collect(),
            selected_tab: None,
            tab_position: 0.0,
            align_on,
            alignment: if align_on {
                style.alignment
            } else {
                ParagraphAlignment::Left
            },
            spacing_on: style.space_before != 0.0 || style.space_after != 0.0,
            before: style.space_before,
            after: style.space_after,
            line_on: style.line_spacing.is_some(),
            multiple: style.line_spacing.unwrap_or(1.0),
        }
    }

    /// The edit OK applies: every value the dialog holds.
    pub fn edit(&self) -> ParagraphEdit {
        ParagraphEdit {
            first_indent: Some(self.first_line - self.hanging),
            left_indent: Some(self.hanging),
            alignment: Some(if self.align_on {
                self.alignment
            } else {
                ParagraphAlignment::Default
            }),
            tabs: Some(self.tabs.clone()),
            space_before: Some(if self.spacing_on { self.before } else { 0.0 }),
            space_after: Some(if self.spacing_on { self.after } else { 0.0 }),
            line_spacing: Some(self.line_on.then_some(self.multiple)),
        }
    }

    /// Add the typed position as a tab stop. False if it is a repeat, below
    /// zero, or the paragraph already has the most it may.
    pub fn add_tab(&mut self) -> bool {
        let at = self.tab_position;
        let usable = at.is_finite() && at >= 0.0;
        if !usable || self.tabs.len() >= MAX_TABS || self.has_tab(at) {
            return false;
        }
        self.tabs.push(at);
        self.tabs.sort_by(f64::total_cmp);
        self.selected_tab = self.tabs.iter().position(|t| *t == at);
        true
    }

    /// Move the selected tab stop to the typed position.
    pub fn modify_tab(&mut self) -> bool {
        let Some(i) = self.selected_tab.filter(|i| *i < self.tabs.len()) else {
            return false;
        };
        let at = self.tab_position;
        let others = self
            .tabs
            .iter()
            .enumerate()
            .any(|(k, t)| k != i && (t - at).abs() < 1e-9);
        if !at.is_finite() || at < 0.0 || others {
            return false;
        }
        self.tabs[i] = at;
        self.tabs.sort_by(f64::total_cmp);
        self.selected_tab = self.tabs.iter().position(|t| *t == at);
        true
    }

    /// Drop the selected tab stop.
    pub fn remove_tab(&mut self) -> bool {
        let Some(i) = self.selected_tab.filter(|i| *i < self.tabs.len()) else {
            return false;
        };
        self.tabs.remove(i);
        self.selected_tab = None;
        true
    }

    /// Pick a stop from the list; its position goes in the Tab box.
    pub fn select_tab(&mut self, i: usize) {
        if let Some(at) = self.tabs.get(i) {
            self.tab_position = *at;
            self.selected_tab = Some(i);
        }
    }

    fn has_tab(&self, at: f64) -> bool {
        self.tabs.iter().any(|t| (t - at).abs() < 1e-9)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use od_core::text::{apply_paragraph_edit, paragraph_view_in};

    fn style(first: f64, left: f64) -> ParagraphStyle {
        ParagraphStyle {
            first_indent: first,
            left_indent: left,
            ..ParagraphStyle::default()
        }
    }

    #[test]
    fn first_line_and_hanging_are_positions_from_the_left_edge() {
        // A hanging indent: first line at 0, the rest at 4.
        let d = Draft::of(&style(-4.0, 4.0));
        assert_eq!((d.first_line, d.hanging), (0.0, 4.0));
        let e = d.edit();
        assert_eq!((e.first_indent, e.left_indent), (Some(-4.0), Some(4.0)));
        // An indented first line: first line at 4, the rest at 0.
        let d = Draft::of(&style(4.0, 0.0));
        assert_eq!((d.first_line, d.hanging), (4.0, 0.0));
    }

    #[test]
    fn ok_writes_the_control_autocad_wrote_for_a_hanging_indent() {
        let mut d = Draft::of(&ParagraphStyle::default());
        d.first_line = 0.0;
        d.hanging = 4.0;
        let done = apply_paragraph_edit("one\\Ptwo", 0..0, &d.edit()).unwrap();
        assert!(done.contents.starts_with("\\pxi-4,l4;one"));
        let view = paragraph_view_in(&done.contents, 0);
        assert_eq!(Draft::of(&view.style).first_line, 0.0);
        assert_eq!(Draft::of(&view.style).hanging, 4.0);
    }

    #[test]
    fn an_untouched_dialog_changes_nothing() {
        let contents = "\\pxi-3,l3,t3;1.\titem";
        let view = paragraph_view_in(contents, 0);
        let done = apply_paragraph_edit(contents, 0..0, &Draft::of(&view.style).edit()).unwrap();
        assert_eq!(done.contents, contents);
    }

    #[test]
    fn alignment_unticked_means_the_default() {
        let mut d = Draft::of(&ParagraphStyle::default());
        assert!(!d.align_on);
        assert_eq!(d.edit().alignment, Some(ParagraphAlignment::Default));
        d.align_on = true;
        d.alignment = ParagraphAlignment::Justified;
        assert_eq!(d.edit().alignment, Some(ParagraphAlignment::Justified));
        let right = ParagraphStyle {
            alignment: ParagraphAlignment::Right,
            ..ParagraphStyle::default()
        };
        let d = Draft::of(&right);
        assert!(d.align_on);
        assert_eq!(d.alignment, ParagraphAlignment::Right);
    }

    #[test]
    fn spacing_is_written_when_ticked_and_dropped_when_not() {
        // The strings AutoCAD wrote in private_material/text-test1.dxf.
        let contents = "\\pxb1,a1.5,sm1.1;text";
        let view = paragraph_view_in(contents, 0);
        let mut d = Draft::of(&view.style);
        assert!(d.spacing_on && d.line_on);
        assert_eq!((d.before, d.after, d.multiple), (1.0, 1.5, 1.1));
        let same = apply_paragraph_edit(contents, 0..0, &d.edit()).unwrap();
        assert_eq!(same.contents, contents);
        d.after = 3.0;
        d.line_on = false;
        let done = apply_paragraph_edit(contents, 0..0, &d.edit()).unwrap();
        assert_eq!(done.contents, "\\pxb1,a3;text");
        d.spacing_on = false;
        let done = apply_paragraph_edit(contents, 0..0, &d.edit()).unwrap();
        assert_eq!(done.contents, "text");
    }

    #[test]
    fn tab_stops_are_added_in_order_and_not_repeated() {
        let mut d = Draft::of(&ParagraphStyle::default());
        for at in [6.0, 2.0, 4.0] {
            d.tab_position = at;
            assert!(d.add_tab());
        }
        assert_eq!(d.tabs, [2.0, 4.0, 6.0]);
        assert_eq!(d.selected_tab, Some(1));
        d.tab_position = 4.0;
        assert!(!d.add_tab());
        d.tab_position = -1.0;
        assert!(!d.add_tab());
        d.tab_position = f64::NAN;
        assert!(!d.add_tab());
        assert_eq!(d.tabs, [2.0, 4.0, 6.0]);
    }

    #[test]
    fn modify_and_remove_work_on_the_selected_stop() {
        let mut d = Draft::of(&ParagraphStyle::default());
        for at in [2.0, 4.0, 6.0] {
            d.tab_position = at;
            d.add_tab();
        }
        d.selected_tab = None;
        assert!(!d.modify_tab() && !d.remove_tab());
        d.select_tab(0);
        assert_eq!(d.tab_position, 2.0);
        d.tab_position = 5.0;
        assert!(d.modify_tab());
        assert_eq!(d.tabs, [4.0, 5.0, 6.0]);
        assert_eq!(d.selected_tab, Some(1));
        d.tab_position = 6.0;
        assert!(!d.modify_tab(), "moving onto another stop is refused");
        assert!(d.remove_tab());
        assert_eq!(d.tabs, [4.0, 6.0]);
        assert_eq!(d.selected_tab, None);
    }

    #[test]
    fn the_most_stops_is_kept() {
        let mut d = Draft::of(&ParagraphStyle::default());
        for k in 0..MAX_TABS {
            d.tab_position = k as f64;
            assert!(d.add_tab());
        }
        d.tab_position = 99.0;
        assert!(!d.add_tab());
    }
}
