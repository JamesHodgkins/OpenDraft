//! Attribute definition ids and the checks on definitions and references.

use std::collections::HashSet;

use super::{after, Document};
use crate::block::attribute::MAX_REFERENCES;
use crate::block::{AttributeDefinition, AttributeDefinitionId, Insert};
use crate::{DocumentError, Entity};

impl Document {
    /// An id no attribute definition has had or will have.
    pub fn allocate_attribute_definition_id(&mut self) -> AttributeDefinitionId {
        let id = AttributeDefinitionId(self.next_attribute_definition_id);
        self.next_attribute_definition_id = after(self.next_attribute_definition_id);
        id
    }

    /// The next attribute-definition id that would be handed out.
    pub fn next_attribute_definition_id(&self) -> u64 {
        self.next_attribute_definition_id
    }

    /// Keep the counter ahead of a definition that has just come in.
    pub(super) fn note_attribute_id(&mut self, entity: &Entity) {
        if let Entity::AttributeDefinition(d) = entity {
            self.next_attribute_definition_id =
                self.next_attribute_definition_id.max(after(d.id.0));
        }
    }

    pub(super) fn note_block_attribute_ids(&mut self, block: &crate::BlockDef) {
        for (_, d) in block.attribute_definitions() {
            self.next_attribute_definition_id =
                self.next_attribute_definition_id.max(after(d.id.0));
        }
    }

    /// Never hand out a definition id below `next` (a loaded drawing keeps
    /// its counter). Only ever raises it.
    pub fn reserve_attribute_definition_ids(&mut self, next: u64) {
        self.next_attribute_definition_id = self.next_attribute_definition_id.max(next);
    }

    /// Definitions: ids nonzero and behind the counter (and unique in a block), well formed,
    /// in a style that exists. References: well formed, each pointing at a
    /// distinct non-constant definition of its insert's block.
    pub(crate) fn validate_attributes(&self) -> Result<(), DocumentError> {
        let bad = |why: String| Err(DocumentError::BadAttribute(why));
        // Ids only have to tell the definitions of one block apart (a
        // reference finds its definition in its own insert's block). Loose
        // definitions can be copied, so they may repeat; they are given new
        // ids when they are made into a block.
        let range_ok =
            |d: &AttributeDefinition| d.id.0 != 0 && d.id.0 < self.next_attribute_definition_id;
        for object in self.objects() {
            if let Entity::AttributeDefinition(d) = &object.entity {
                if !range_ok(d) {
                    return bad(format!("definition id {} is out of range", d.id.0));
                }
                self.check_definition(d)?;
            }
        }
        for block in self.blocks() {
            let mut ids = HashSet::new();
            for (_, d) in block.attribute_definitions() {
                if !range_ok(d) || !ids.insert(d.id) {
                    return bad(format!(
                        "definition id {} in {} is repeated or out of range",
                        d.id.0, block.name
                    ));
                }
                self.check_definition(d)?;
            }
        }
        let inserts = self
            .objects()
            .iter()
            .filter_map(|o| match &o.entity {
                Entity::Insert(i) => Some(&**i),
                _ => None,
            })
            .chain(self.blocks().iter().flat_map(|b| {
                b.entities.iter().filter_map(|e| match &e.entity {
                    Entity::Insert(i) => Some(&**i),
                    _ => None,
                })
            }));
        for insert in inserts {
            self.check_references(insert)?;
        }
        Ok(())
    }

    fn check_definition(&self, d: &AttributeDefinition) -> Result<(), DocumentError> {
        d.validate()
            .map_err(|why| DocumentError::BadAttribute(format!("{}: {why}", d.tag)))?;
        if self.text_style(d.text.style).is_none() {
            return Err(DocumentError::BadAttribute(format!(
                "{} uses a missing text style",
                d.tag
            )));
        }
        Ok(())
    }

    fn check_references(&self, insert: &Insert) -> Result<(), DocumentError> {
        let bad = |why: String| Err(DocumentError::BadAttribute(why));
        if insert.attributes.len() > MAX_REFERENCES {
            return bad(format!("{} holds too many attributes", insert.block.name));
        }
        let mut sources = HashSet::new();
        for r in &insert.attributes {
            r.validate()
                .map_err(|why| DocumentError::BadAttribute(format!("{}: {why}", r.tag)))?;
            if self.text_style(r.text.style).is_none() {
                return bad(format!("{} uses a missing text style", r.tag));
            }
            if r.attrs
                .as_ref()
                .is_some_and(|a| self.layer(a.layer).is_none())
            {
                return bad(format!("{} is on a layer that does not exist", r.tag));
            }
            if let Some(id) = r.source {
                let direct = insert.block.attribute_definition(id).is_some_and(
                    |e| matches!(&e.entity, Entity::AttributeDefinition(d) if !d.flags.constant()),
                );
                if !direct || !sources.insert(id) {
                    return bad(format!(
                        "{} in {} does not match one definition",
                        r.tag, insert.block.name
                    ));
                }
            }
        }
        Ok(())
    }
}
