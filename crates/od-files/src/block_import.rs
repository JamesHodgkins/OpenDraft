//! Block definitions from an exchange file, for the DXF and DWG importers.
//!
//! A file names the block an insert places (DXF by name, DWG by handle) and
//! lists definitions in any order, but a `BlockDef` can only hold inserts of
//! blocks built before it. So the importer collects each definition with its
//! inserts unresolved, and [`ImportedBlocks::build`] builds them depth first,
//! then puts them in the table in the file's order. What cannot be kept is
//! counted for the warnings.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use od_core::block::{block_name_problem, MAX_NESTING};
use od_core::{Attributes, BlockDef, BlockEntity, BlockId, Document, Entity, Insert, Point};

use crate::plural;

/// A block reference whose block is known only by the file's key for it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PendingInsert<K> {
    pub block: K,
    pub insertion: Point,
    pub scale_x: f64,
    pub scale_y: f64,
    /// Radians counter-clockwise.
    pub rotation: f64,
}

/// One object of a definition being read.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Item<K> {
    Entity(Entity),
    Insert(PendingInsert<K>),
}

/// A definition as the file gives it.
#[derive(Clone, Debug)]
pub(crate) struct PendingBlock<K> {
    pub key: K,
    pub name: String,
    pub base: Point,
    pub description: String,
    pub items: Vec<(Item<K>, Attributes)>,
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Building,
    Refused,
}

/// The blocks built from a file, by the file's key, and what was lost.
pub(crate) struct ImportedBlocks<K> {
    built: HashMap<K, Arc<BlockDef>>,
    /// Definitions refused for a name AutoCAD would not allow, or one that
    /// repeats an earlier block's (ignoring case).
    bad_names: usize,
    /// References that would put a block inside itself, or nest past
    /// `MAX_NESTING`.
    looped: usize,
    /// References to a block that is not in the file (or was refused).
    missing: usize,
    /// References with a zero or non-finite scale, position or rotation.
    invalid: usize,
}

impl<K> Default for ImportedBlocks<K> {
    fn default() -> Self {
        ImportedBlocks {
            built: HashMap::new(),
            bad_names: 0,
            looped: 0,
            missing: 0,
            invalid: 0,
        }
    }
}

/// The sentence for references to anonymous blocks, which are not read.
pub(crate) fn anonymous_references(count: u64) -> String {
    format!(
        "Skipped {} to anonymous blocks, such as dynamic blocks (these are not imported yet).",
        plural(count, "block reference")
    )
}

/// The sentence for references to other drawings, which are not read.
pub(crate) fn external_references(count: u64) -> String {
    format!(
        "Skipped {} (xrefs are not imported).",
        plural(count, "external reference")
    )
}

impl<K: Clone + Eq + Hash> ImportedBlocks<K> {
    /// Builds every definition and adds them to `document`'s block table in
    /// the order given.
    pub(crate) fn build(document: &mut Document, pending: Vec<PendingBlock<K>>) -> Self {
        let mut blocks = ImportedBlocks::default();
        let mut by_key: HashMap<K, usize> = HashMap::new();
        let mut names: Vec<String> = Vec::new();
        let mut states: Vec<Option<State>> = vec![None; pending.len()];
        for (i, p) in pending.iter().enumerate() {
            let name = p.name.trim().to_lowercase();
            if block_name_problem(&p.name).is_some() || names.contains(&name) {
                blocks.bad_names += 1;
                states[i] = Some(State::Refused);
            } else {
                names.push(name);
            }
            by_key.entry(p.key.clone()).or_insert(i);
        }
        // Ids in the file's order, so the table reads as the file does.
        let ids: Vec<_> = pending
            .iter()
            .map(|_| document.allocate_block_id())
            .collect();
        let mut builder = Builder {
            pending: &pending,
            by_key: &by_key,
            ids: &ids,
            states: &mut states,
            blocks: &mut blocks,
        };
        for i in 0..pending.len() {
            builder.build(i, 0);
        }
        for p in &pending {
            if let Some(def) = blocks.built.get(&p.key) {
                if document.block(def.id).is_none() {
                    document.push_block(def.clone());
                }
            }
        }
        blocks
    }

    /// The reference as an entity, or `None` (counted) when its block is not
    /// there or its transform is not usable.
    pub(crate) fn insert(&mut self, p: PendingInsert<K>) -> Option<Entity> {
        let Some(block) = self.built.get(&p.block) else {
            self.missing += 1;
            return None;
        };
        self.entity(block.clone(), &p)
    }

    fn entity(&mut self, block: Arc<BlockDef>, p: &PendingInsert<K>) -> Option<Entity> {
        let insert = Insert {
            block,
            insertion: p.insertion,
            scale_x: p.scale_x,
            scale_y: p.scale_y,
            rotation: p.rotation,
        };
        if insert.is_valid() {
            Some(Entity::Insert(Box::new(insert)))
        } else {
            self.invalid += 1;
            None
        }
    }

    /// The sentences for what did not come across.
    pub(crate) fn warnings(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.bad_names > 0 {
            out.push(format!(
                "Skipped {} whose names are not allowed or repeat another block's name.",
                plural(self.bad_names as u64, "block")
            ));
        }
        if self.looped > 0 {
            out.push(format!(
                "Skipped {} that would put a block inside itself.",
                plural(self.looped as u64, "block reference")
            ));
        }
        if self.missing > 0 {
            out.push(format!(
                "Skipped {} to blocks that are not in the file.",
                plural(self.missing as u64, "block reference")
            ));
        }
        if self.invalid > 0 {
            out.push(format!(
                "Skipped {} with a zero or invalid scale or position.",
                plural(self.invalid as u64, "block reference")
            ));
        }
        out
    }
}

/// The depth-first walk that builds one definition after the ones it holds.
struct Builder<'a, K> {
    pending: &'a [PendingBlock<K>],
    by_key: &'a HashMap<K, usize>,
    ids: &'a [BlockId],
    states: &'a mut Vec<Option<State>>,
    blocks: &'a mut ImportedBlocks<K>,
}

impl<K: Clone + Eq + Hash> Builder<'_, K> {
    /// Builds definition `i` (and first every block it inserts), unless it
    /// is built, refused or being built further up.
    fn build(&mut self, i: usize, depth: usize) {
        if self.states[i].is_some() || self.blocks.built.contains_key(&self.pending[i].key) {
            return;
        }
        self.states[i] = Some(State::Building);
        let pending = self.pending;
        let p = &pending[i];
        let mut entities = Vec::with_capacity(p.items.len());
        for (item, attrs) in &p.items {
            let entity = match item {
                Item::Entity(e) => Some(e.clone()),
                Item::Insert(insert) => self.nested(insert, depth),
            };
            if let Some(entity) = entity {
                entities.push(BlockEntity {
                    entity,
                    attrs: attrs.clone(),
                });
            }
        }
        let def = BlockDef {
            id: self.ids[i],
            name: p.name.trim().to_owned(),
            base: p.base,
            description: p.description.clone(),
            entities,
        };
        self.blocks.built.insert(p.key.clone(), Arc::new(def));
        self.states[i] = None;
    }

    fn nested(&mut self, insert: &PendingInsert<K>, depth: usize) -> Option<Entity> {
        let Some(&j) = self.by_key.get(&insert.block) else {
            self.blocks.missing += 1;
            return None;
        };
        if self.states[j] == Some(State::Building) || depth >= MAX_NESTING {
            self.blocks.looped += 1;
            return None;
        }
        self.build(j, depth + 1);
        match self.blocks.built.get(&insert.block) {
            Some(block) => {
                let block = block.clone();
                self.blocks.entity(block, insert)
            }
            None => {
                self.blocks.missing += 1;
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use od_core::LayerId;

    use super::*;

    fn line() -> (Item<&'static str>, Attributes) {
        let e = Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(1.0, 0.0),
        };
        (Item::Entity(e), Attributes::on_layer(LayerId::DEFAULT))
    }

    fn insert(of: &'static str) -> (Item<&'static str>, Attributes) {
        let i = PendingInsert {
            block: of,
            insertion: Point::new(2.0, 0.0),
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
        };
        (Item::Insert(i), Attributes::on_layer(LayerId::DEFAULT))
    }

    fn block(
        key: &'static str,
        items: Vec<(Item<&'static str>, Attributes)>,
    ) -> PendingBlock<&'static str> {
        PendingBlock {
            key,
            name: key.to_owned(),
            base: Point::new(0.0, 0.0),
            description: String::new(),
            items,
        }
    }

    #[test]
    fn a_block_may_insert_one_defined_after_it() {
        let mut doc = Document::default();
        let mut blocks = ImportedBlocks::build(
            &mut doc,
            vec![
                block("Outer", vec![insert("Inner")]),
                block("Inner", vec![line()]),
            ],
        );
        let names: Vec<&str> = doc.blocks().iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, ["Outer", "Inner"], "the file's order");
        let outer = doc.block_named("Outer").unwrap();
        let inner = doc.block_named("Inner").unwrap();
        assert!(outer.holds(inner.id));
        assert!(blocks.warnings().is_empty());
        let entity = blocks.insert(PendingInsert {
            block: "Outer",
            insertion: Point::new(5.0, 5.0),
            scale_x: 2.0,
            scale_y: 2.0,
            rotation: 0.0,
        });
        doc.add(entity.unwrap());
        doc.validate().unwrap();
    }

    #[test]
    fn loops_missing_blocks_and_bad_names_are_counted() {
        let mut doc = Document::default();
        let mut blocks = ImportedBlocks::build(
            &mut doc,
            vec![
                block("A", vec![line(), insert("B")]),
                block("B", vec![insert("A")]),
                block("C", vec![insert("Nowhere")]),
                block("bad|name", vec![line()]),
                block("a", vec![line()]),
            ],
        );
        assert_eq!(doc.blocks().len(), 3);
        doc.validate().unwrap();
        assert!(blocks
            .insert(PendingInsert {
                block: "A",
                insertion: Point::new(0.0, 0.0),
                scale_x: 0.0,
                scale_y: 1.0,
                rotation: 0.0,
            })
            .is_none());
        assert_eq!(
            blocks.warnings(),
            [
                "Skipped 2 blocks whose names are not allowed or repeat another block's name.",
                "Skipped 1 block reference that would put a block inside itself.",
                "Skipped 1 block reference to blocks that are not in the file.",
                "Skipped 1 block reference with a zero or invalid scale or position.",
            ]
        );
    }
}
