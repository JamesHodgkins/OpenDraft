//! Data fields in multiline text.
//!
//! As in AutoCAD, the contents hold a marker, `%<\_FldIdx N>%`, where a field
//! shows, and the entity keeps the fields themselves in [`MText::fields`]
//! (marker `N` is `fields[N]`). Layout draws each marker as its field's value;
//! any other `%<...>%` span is still skipped. The same field may be marked
//! more than once, and a field no marker names is dropped by
//! [`MText::prune_fields`].

use crate::block::{
    DataField, FieldContext, FieldRun, FieldText, FieldUpdate, MAX_FIELDS_PER_TEXT,
};
use crate::text::{parse, NodeKind};
use crate::Document;

use super::MText;

const PREFIX: &str = "%<\\_FldIdx ";
const SUFFIX: &str = ">%";

/// The marker for field `index`.
pub fn marker(index: usize) -> String {
    format!("{PREFIX}{index}{SUFFIX}")
}

/// The field a marker span names.
pub fn marker_index(span: &str) -> Option<usize> {
    span.strip_prefix(PREFIX)?
        .strip_suffix(SUFFIX)?
        .parse()
        .ok()
}

impl MText {
    /// What each field shows, by index.
    pub fn field_values(&self) -> Vec<String> {
        self.fields.iter().map(DataField::display).collect()
    }

    /// The fields as one text, for the checks and operations that work on
    /// whole texts.
    pub fn fields_as_text(&self) -> FieldText {
        FieldText::Runs(self.fields.iter().cloned().map(FieldRun::Field).collect())
    }

    /// Put `field` in the text at byte `at` (a character boundary outside
    /// any control) and give back its index; `None` if the position is not
    /// valid or the text holds as many fields as it may.
    pub fn insert_field(&mut self, at: usize, field: DataField) -> Option<usize> {
        if !self.contents.is_char_boundary(at) || self.fields.len() >= MAX_FIELDS_PER_TEXT {
            return None;
        }
        let index = self.fields.len();
        self.contents.insert_str(at, &marker(index));
        self.fields.push(field);
        Some(index)
    }

    /// The marker spans in the contents: their byte ranges and the index each
    /// names (`None` for a marker that points at nothing).
    fn markers(&self) -> Vec<(std::ops::Range<usize>, Option<usize>)> {
        let Ok(parsed) = parse(&self.contents) else {
            return Vec::new();
        };
        parsed
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Field)
            .filter_map(|n| {
                let span = self.contents.get(n.source.clone())?;
                marker_index(span)
                    .map(|i| (n.source.clone(), Some(i).filter(|i| *i < self.fields.len())))
            })
            .collect()
    }

    /// Drop the fields no marker names and number the rest in the order they
    /// first appear. Markers that name no field are left as they are.
    pub fn prune_fields(&mut self) {
        let markers = self.markers();
        let mut order: Vec<usize> = Vec::new();
        for (_, index) in &markers {
            if let Some(i) = index {
                if !order.contains(i) {
                    order.push(*i);
                }
            }
        }
        if order.len() == self.fields.len() && order.iter().enumerate().all(|(n, i)| n == *i) {
            return;
        }
        let mut contents = self.contents.clone();
        for (range, index) in markers.iter().rev() {
            if let Some(i) = index {
                let new = order.iter().position(|o| o == i).unwrap_or(0);
                contents.replace_range(range.clone(), &marker(new));
            }
        }
        self.fields = order.iter().map(|i| self.fields[*i].clone()).collect();
        self.contents = contents;
    }

    /// The fields replaced by those of `text` (as [`Self::fields_as_text`]
    /// gives them, possibly changed).
    pub(crate) fn set_fields_from(&mut self, text: FieldText) {
        if let FieldText::Runs(runs) = text {
            self.fields = runs
                .into_iter()
                .filter_map(|r| match r {
                    FieldRun::Field(f) => Some(f),
                    FieldRun::Literal(_) => None,
                })
                .collect();
        }
    }

    /// The text with every field brought up to date.
    pub fn evaluated(&self, doc: &Document, context: &FieldContext) -> MText {
        let mut text = self.clone();
        text.set_fields_from(self.fields_as_text().evaluated(doc, context));
        text
    }

    /// The contents with each marker replaced by what its field shows, for
    /// files that have no data fields. The value is written as plain text:
    /// its backslashes and braces are escaped.
    pub fn flattened_contents(&self) -> String {
        let mut contents = self.contents.clone();
        for (range, index) in self.markers().into_iter().rev() {
            if let Some(i) = index {
                let shown = self.fields[i].display();
                let mut plain = String::with_capacity(shown.len());
                for c in shown.chars() {
                    if matches!(c, '\\' | '{' | '}') {
                        plain.push('\\');
                    }
                    plain.push(c);
                }
                contents.replace_range(range, &plain);
            }
        }
        contents
    }

    /// Count what an update of this text did, into `update`.
    pub(crate) fn tally(&self, after: &MText, update: &mut FieldUpdate) {
        update.found += self.fields.len();
        update.failed += after
            .fields
            .iter()
            .filter(|f| f.status == crate::block::FieldStatus::EvaluationFailed)
            .count();
        update.changed += self
            .fields
            .iter()
            .zip(&after.fields)
            .filter(|(a, b)| a != b)
            .count();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{FieldStatus, FieldValue};
    use crate::{Point, TextStyleId};

    fn text(contents: &str) -> MText {
        MText::new(Point::new(0.0, 0.0), 2.5, contents, TextStyleId::STANDARD)
    }

    fn shown(v: &str) -> DataField {
        DataField::new("AcVar", "x", Some(FieldValue::Text(v.into())))
    }

    #[test]
    fn a_marker_round_trips_and_other_spans_are_not_markers() {
        assert_eq!(marker_index(&marker(12)), Some(12));
        assert_eq!(marker_index("%<\\AcVar Date>%"), None);
        assert_eq!(marker_index("%<\\_FldIdx x>%"), None);
    }

    #[test]
    fn inserting_puts_a_marker_at_the_place_and_keeps_the_field() {
        let mut t = text("ab");
        assert_eq!(t.insert_field(1, shown("F")), Some(0));
        assert_eq!(t.contents, format!("a{}b", marker(0)));
        assert_eq!(t.field_values(), ["F"]);
        assert_eq!(t.insert_field(99, shown("G")), None, "past the end");
    }

    #[test]
    fn pruning_drops_unused_fields_and_renumbers_the_rest_in_order() {
        let mut t = text(&format!("{}x{}", marker(2), marker(0)));
        t.fields = vec![shown("a"), shown("b"), shown("c")];
        t.prune_fields();
        assert_eq!(t.field_values(), ["c", "a"]);
        assert_eq!(t.contents, format!("{}x{}", marker(0), marker(1)));
    }

    #[test]
    fn pruning_keeps_a_field_marked_twice_once() {
        let mut t = text(&format!("{}{}", marker(0), marker(0)));
        t.fields = vec![shown("a")];
        t.prune_fields();
        assert_eq!(t.fields.len(), 1);
        assert_eq!(t.contents, format!("{}{}", marker(0), marker(0)));
    }

    #[test]
    fn pruning_a_tidy_text_changes_nothing() {
        let mut t = text(&format!("a{}", marker(0)));
        t.fields = vec![shown("a")];
        let before = t.clone();
        t.prune_fields();
        assert_eq!(t, before);
    }

    #[test]
    fn flattening_writes_the_values_as_plain_text() {
        let mut t = text(&format!("a {} b {{", marker(0)));
        t.fields = vec![shown("C:\\x{y}")];
        assert_eq!(t.flattened_contents(), "a C:\\\\x\\{y\\} b {");
        t.fields.clear();
        assert_eq!(
            t.flattened_contents(),
            t.contents,
            "a dangling marker stays"
        );
    }

    #[test]
    fn evaluating_updates_the_fields_and_leaves_the_rest() {
        let mut t = text(&format!("File {}", marker(0)));
        t.fields = vec![DataField::file_name()];
        let ctx = FieldContext {
            file_name: Some("plan.odx".into()),
            ..FieldContext::default()
        };
        let after = t.evaluated(&Document::default(), &ctx);
        assert_eq!(after.contents, t.contents);
        assert_eq!(after.field_values(), ["plan.odx"]);
        assert_eq!(after.fields[0].status, FieldStatus::Success);
    }
}
