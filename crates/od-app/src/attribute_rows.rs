//! The Attributes section of the Properties panel: one row for each attribute
//! of the selected block reference, its tag and its value, the value edited
//! in place. For several references it lists the tags every one of them has
//! exactly once (a tag that repeats cannot be told apart, so the Enhanced
//! Attribute Editor is for that), and a value they do not share reads
//! "Varies".

use eframe::egui::{self, Align2, FontId, Ui};
use od_commands::ModifyEntities;
use od_core::block::attribute::normalized_tag;
use od_core::{Entity, EntityKind, Insert};
use od_ui::Theme;

use crate::properties_ui::{row_frame, text_cell};
use crate::state::AppState;

/// One row: a tag and what its attribute holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub tag: String,
    /// `None` when the references disagree.
    pub value: Option<String>,
    /// A data field is shown and not edited here.
    pub field: bool,
}

/// The index of the one attribute of `insert` called `tag`, if there is
/// exactly one (tags compare ignoring case).
fn only(insert: &Insert, tag: &str) -> Option<usize> {
    let key = normalized_tag(tag);
    let mut found = insert
        .attributes
        .iter()
        .enumerate()
        .filter(|(_, r)| normalized_tag(&r.tag) == key);
    let first = found.next()?.0;
    found.next().is_none().then_some(first)
}

/// The one attribute of `insert` called `tag`.
fn reference<'a>(insert: &'a Insert, tag: &str) -> Option<&'a od_core::block::AttributeReference> {
    only(insert, tag).map(|i| &insert.attributes[i])
}

/// The rows for `inserts`, in the order of the first one's attributes. A tag
/// that is not exactly once in each is left out.
pub fn rows(inserts: &[&Insert]) -> Vec<Row> {
    let Some((first, rest)) = inserts.split_first() else {
        return Vec::new();
    };
    first
        .attributes
        .iter()
        .filter_map(|r| {
            // Unique in the first one, so it is listed once.
            let mut all = vec![reference(first, &r.tag)?];
            for insert in rest {
                all.push(reference(insert, &r.tag)?);
            }
            let shown = all[0].value.display();
            Some(Row {
                tag: r.tag.clone(),
                value: all
                    .iter()
                    .all(|a| a.value.display() == shown)
                    .then_some(shown),
                field: all.iter().any(|a| a.value.has_fields()),
            })
        })
        .collect()
}

/// `insert` with the attribute `tag` holding `value`, or `None` when it has
/// no one such attribute, it is a data field, or it already holds `value`.
pub fn with_value(insert: &Insert, tag: &str, value: &str) -> Option<Insert> {
    let i = only(insert, tag)?;
    let r = &insert.attributes[i];
    if r.value.has_fields() || r.value.display() == value {
        return None;
    }
    let mut out = insert.clone();
    out.attributes[i].value = value.into();
    Some(out)
}

/// The rows drawn, with the value cells editable. Returns the tag and value
/// committed.
pub fn show(ui: &mut Ui, theme: &Theme, rows: &[Row]) -> Option<(String, String)> {
    let mut edit = None;
    for row in rows {
        let (label_cell, cell) = row_frame(ui, theme, &row.tag);
        ui.interact(
            label_cell,
            ui.id().with(("attribute_label", &row.tag)),
            egui::Sense::hover(),
        )
        .on_hover_text(&row.tag);
        if row.field {
            ui.painter().text(
                cell.left_center() + egui::vec2(8.0, 0.0),
                Align2::LEFT_CENTER,
                row.value.as_deref().unwrap_or("Varies"),
                FontId::proportional(theme.font_size_body),
                theme.colors.text_weak,
            );
            continue;
        }
        let id = egui::Id::new(("property_attribute", &row.tag));
        let shown = row.value.as_deref().unwrap_or_default();
        if let Some(committed) = text_cell(ui, theme, cell, id, shown) {
            edit = Some((row.tag.clone(), committed));
        }
    }
    edit
}

impl AppState {
    /// Give the attribute `tag` of every selected block reference (of `kind`,
    /// when the panel is narrowed to one) the value `value`: one undo step.
    pub fn set_attribute_value(&mut self, tag: &str, value: &str, kind: Option<EntityKind>) {
        self.interrupt_command();
        let doc = &mut self.docs[self.active];
        let document = doc.session.document();
        let changes: Vec<_> = doc
            .selection
            .iter()
            .filter_map(|&id| {
                let object = document.object(id)?;
                if kind.is_some_and(|k| object.entity.kind() != k) {
                    return None;
                }
                let Entity::Insert(insert) = &object.entity else {
                    return None;
                };
                Some((
                    id,
                    Entity::Insert(Box::new(with_value(insert, tag, value)?)),
                ))
            })
            .collect();
        if changes.is_empty() {
            return;
        }
        self.message = format!("Changed the attribute {tag}");
        doc.session
            .perform(ModifyEntities::new("Edit attribute", changes));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribute_test_util::{attributed_insert, values};

    fn insert_of(state: &AppState, id: od_core::EntityId) -> Insert {
        match state.session().document().get(id) {
            Some(Entity::Insert(i)) => (**i).clone(),
            _ => panic!("an insert"),
        }
    }

    #[test]
    fn a_reference_lists_its_attributes_in_order_with_their_values() {
        let (state, id) = attributed_insert();
        let i = insert_of(&state, id);
        let r = rows(&[&i]);
        let shown: Vec<_> = r
            .iter()
            .map(|r| (r.tag.as_str(), r.value.as_deref()))
            .collect();
        assert_eq!(shown, [("A", Some("dflt")), ("B", Some("dflt"))]);
        assert!(r.iter().all(|r| !r.field));
    }

    #[test]
    fn several_references_show_shared_tags_and_say_varies_when_they_differ() {
        let (state, id) = attributed_insert();
        let a = insert_of(&state, id);
        let mut b = a.clone();
        b.attributes[1].value = "other".into();
        let r = rows(&[&a, &b]);
        assert_eq!(r[0].value.as_deref(), Some("dflt"));
        assert_eq!(r[1].value, None, "Varies");
        // A tag the second one lacks, or has twice, is not listed.
        let mut lacking = a.clone();
        lacking.attributes.remove(0);
        assert_eq!(rows(&[&a, &lacking]).len(), 1);
        let mut twice = a.clone();
        twice.attributes.push(twice.attributes[0].clone());
        assert_eq!(rows(&[&a, &twice]).len(), 1);
        assert!(rows(&[&twice]).iter().all(|r| r.tag != "A"));
    }

    #[test]
    fn a_value_is_set_by_tag_ignoring_case_and_a_field_or_no_change_is_left_alone() {
        let (state, id) = attributed_insert();
        let i = insert_of(&state, id);
        let out = with_value(&i, "b", "new").unwrap();
        assert_eq!(out.attributes[1].value.display(), "new");
        assert_eq!(out.attributes[0], i.attributes[0]);
        assert_eq!(with_value(&i, "B", "dflt"), None);
        assert_eq!(with_value(&i, "NOPE", "x"), None);
        let mut field = i.clone();
        let f = od_core::block::DataField::new("AcVar", "x", None);
        field.attributes[0].value =
            od_core::block::FieldText::Runs(vec![od_core::block::FieldRun::Field(f)]);
        assert_eq!(with_value(&field, "A", "x"), None);
        assert!(rows(&[&field])[0].field);
    }

    #[test]
    fn editing_in_the_panel_changes_the_selected_references_in_one_undo_step() {
        let (mut state, id) = attributed_insert();
        state.select_ids([id], false);
        state.set_attribute_value("A", "from the panel", None);
        assert_eq!(values(&state, id), ["from the panel", "dflt"]);
        assert_eq!(state.session().document().validate(), Ok(()));
        // The same value again is no edit; a kind that is not selected is none.
        let before = state.session().revision();
        state.set_attribute_value("A", "from the panel", None);
        state.set_attribute_value("B", "x", Some(EntityKind::Line));
        assert_eq!(state.session().revision(), before);
        state.undo();
        assert_eq!(values(&state, id), ["dflt", "dflt"]);
    }
}
