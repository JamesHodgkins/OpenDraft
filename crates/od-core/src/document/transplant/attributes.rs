//! Attributes crossing from one drawing into another.

use std::collections::HashMap;

use super::Merge;
use crate::block::{AttributeDefinition, FieldObjectRef, FieldText, Insert};
use crate::{Entity, TextStyleId};

impl Merge<'_> {
    /// A definition in the target's text style, its default value detached.
    pub(super) fn attribute_definition(&mut self, d: &mut AttributeDefinition) {
        d.text.style = self.text_style(d.text.style);
        d.default_value = self.detached(&d.default_value);
    }

    /// The references of `i`, whose block is already the target's, made fit
    /// the target: its text styles, and its fields detached.
    pub(super) fn insert_attributes(&mut self, i: &mut Insert) {
        for r in &mut i.attributes {
            r.text.style = self.text_style(r.text.style);
            r.value = self.detached(&r.value);
            r.attrs = r.attrs.as_ref().map(|a| self.attrs(a));
            // The target may hold another block of this name, whose
            // definition ids mean other things: a reference keeps its
            // definition only if that one has the same tag.
            r.source = r.source.filter(|id| {
                i.block.attribute_definition(*id).is_some_and(|e| {
                    matches!(&e.entity, Entity::AttributeDefinition(d)
                        if !d.flags.constant() && d.tag.eq_ignore_ascii_case(&r.tag))
                })
            });
        }
    }

    /// Text whose data fields point at the source's objects, for the target:
    /// a field keeps its code and last value, but what it pointed at has
    /// other ids there, so those references become unresolved.
    fn detached(&mut self, text: &FieldText) -> FieldText {
        if !text.has_fields() {
            return text.clone();
        }
        let mapped: HashMap<TextStyleId, TextStyleId> = text
            .object_refs()
            .iter()
            .filter_map(|r| match r {
                FieldObjectRef::TextStyle(id) => Some(*id),
                _ => None,
            })
            .map(|id| (id, self.text_style(id)))
            .collect();
        text.map_refs(&|r| match r {
            FieldObjectRef::TextStyle(id) => {
                FieldObjectRef::TextStyle(mapped.get(id).copied().unwrap_or(*id))
            }
            FieldObjectRef::Unresolved(token) => FieldObjectRef::Unresolved(token.clone()),
            other => FieldObjectRef::Unresolved(format!("{other:?}")),
        })
    }
}
