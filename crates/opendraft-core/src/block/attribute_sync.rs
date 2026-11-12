//! Keeping a block's inserts in step with its definitions (ATTSYNC), and
//! giving the definitions of a new block ids of their own.

use std::collections::{HashMap, HashSet};

use crate::block::attribute::{AttributeDefinitionId, AttributeReference, MAX_REFERENCES};
use crate::block::{BlockDef, BlockEntity};
use crate::Entity;

/// What synchronising one insert gives.
#[derive(Clone, Debug, PartialEq)]
pub struct Synced {
    pub references: Vec<AttributeReference>,
    /// Kept, with their value, and brought up to date with the definition.
    pub matched: usize,
    /// Made new, holding the definition's default.
    pub added: usize,
    /// References with no definition: kept after the others, or dropped.
    pub orphaned: usize,
}

/// A tag as compared: ASCII upper case, so tags that differ only in case are
/// the same tag.
fn key(tag: &str) -> String {
    tag.to_ascii_uppercase()
}

/// `current` (an insert's references) made to match `block`'s non-constant
/// definitions, in their order. A reference keeps its value; its tag, mode,
/// style, size and place come from the definition again. One with no source
/// that is the only orphan with a definition's tag is adopted by it.
/// `remove_orphans` drops the others rather than keeping them last.
pub fn sync_references(
    block: &BlockDef,
    current: &[AttributeReference],
    remove_orphans: bool,
) -> Synced {
    let mut by_source: HashMap<AttributeDefinitionId, usize> = HashMap::new();
    let mut orphans: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, r) in current.iter().enumerate() {
        match r.source {
            Some(id) => {
                by_source.entry(id).or_insert(i);
            }
            None => orphans.entry(key(&r.tag)).or_default().push(i),
        }
    }
    let mut used = HashSet::new();
    let mut out = Vec::new();
    let (mut matched, mut added) = (0, 0);
    for (_, d) in block
        .attribute_definitions()
        .filter(|(_, d)| !d.flags.constant())
    {
        if out.len() >= MAX_REFERENCES {
            break;
        }
        let found = by_source.get(&d.id).copied().or_else(|| {
            match orphans.get(&key(&d.tag)).map(Vec::as_slice) {
                Some([only]) if !used.contains(only) => Some(*only),
                _ => None,
            }
        });
        match found {
            Some(i) => {
                used.insert(i);
                matched += 1;
                out.push(d.reference(current[i].value.clone()));
            }
            None => {
                added += 1;
                out.push(d.reference(d.default_value.clone()));
            }
        }
    }
    let left: Vec<&AttributeReference> = current
        .iter()
        .enumerate()
        .filter(|(i, _)| !used.contains(i))
        .map(|(_, r)| r)
        .collect();
    let orphaned = left.len();
    if !remove_orphans {
        out.extend(left.into_iter().map(|r| AttributeReference {
            source: None,
            ..r.clone()
        }));
    }
    Synced {
        references: out,
        matched,
        added,
        orphaned,
    }
}

impl BlockDef {
    /// Give a new id from `fresh` to each definition whose id is 0 or
    /// repeats an earlier one in this block, so the block's definitions can
    /// be told apart. Returns how many were changed.
    pub fn renumber_duplicate_attributes(
        &mut self,
        mut fresh: impl FnMut() -> AttributeDefinitionId,
    ) -> usize {
        let mut seen = HashSet::new();
        let mut changed = 0;
        for BlockEntity { entity, .. } in &mut self.entities {
            if let Entity::AttributeDefinition(d) = entity {
                if d.id.0 == 0 || !seen.insert(d.id) {
                    d.id = fresh();
                    seen.insert(d.id);
                    changed += 1;
                }
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::block::attribute::{AttributeFlags, AttributeText};
    use crate::block::{AttributeDefinition, FieldText};
    use crate::{Attributes, BlockId, LayerId, Point, TextStyleId};

    fn def(id: u64, tag: &str, flags: u16, default: &str) -> BlockEntity {
        BlockEntity {
            entity: Entity::AttributeDefinition(Box::new(AttributeDefinition {
                id: AttributeDefinitionId(id),
                tag: tag.into(),
                prompt: String::new(),
                default_value: FieldText::from(default),
                flags: AttributeFlags(flags),
                lock_position: false,
                field_length: 0,
                text: AttributeText::new(Point::default(), 1.0, TextStyleId::STANDARD),
            })),
            attrs: Attributes::on_layer(LayerId::DEFAULT),
        }
    }

    fn block(entities: Vec<BlockEntity>) -> Arc<BlockDef> {
        Arc::new(BlockDef {
            unit: Default::default(),
            id: BlockId(1),
            name: "B".into(),
            base: Point::default(),
            description: String::new(),
            entities,
        })
    }

    fn values(s: &Synced) -> Vec<(String, String)> {
        s.references
            .iter()
            .map(|r| (r.tag.clone(), r.value.display()))
            .collect()
    }

    #[test]
    fn values_are_kept_new_definitions_get_defaults_and_order_follows_the_block() {
        let old = block(vec![def(1, "A", 0, "a0"), def(2, "B", 0, "b0")]);
        let mut insert = old.new_references(|d| d.default_value.clone());
        insert[0].value = "mine".into();
        // The block now has B first, a new C, and A with a new tag.
        let new = block(vec![
            def(2, "B", 0, "b1"),
            def(3, "C", 0, "c1"),
            def(1, "A2", 0, "a1"),
        ]);
        let s = sync_references(&new, &insert, false);
        assert_eq!(
            values(&s),
            [("B", "b0"), ("C", "c1"), ("A2", "mine")].map(|(a, b)| (a.into(), b.into()))
        );
        assert_eq!((s.matched, s.added, s.orphaned), (2, 1, 0));
    }

    #[test]
    fn syncing_twice_changes_nothing() {
        let b = block(vec![
            def(1, "A", 0, "x"),
            def(2, "K", AttributeFlags::CONSTANT, "k"),
        ]);
        let mut insert = b.new_references(|d| d.default_value.clone());
        insert[0].value = "mine".into();
        let once = sync_references(&b, &insert, false);
        let twice = sync_references(&b, &once.references, false);
        assert_eq!(once.references, twice.references);
        assert_eq!(once.references.len(), 1, "a constant has no reference");
    }

    #[test]
    fn an_orphan_with_a_unique_tag_is_adopted_and_an_ambiguous_one_is_not() {
        let b = block(vec![def(5, "NAME", 0, "d"), def(6, "TWIN", 0, "d")]);
        let orphan = |tag: &str, value: &str| {
            let mut r = b.new_references(|_| value.into())[0].clone();
            r.source = None;
            r.tag = tag.into();
            r
        };
        let current = [
            orphan("name", "kept"),
            orphan("TWIN", "x"),
            orphan("twin", "y"),
        ];
        let s = sync_references(&b, &current, false);
        assert_eq!(s.references[0].value.display(), "kept");
        assert_eq!(s.references[0].source, Some(AttributeDefinitionId(5)));
        assert_eq!(
            s.references[1].value.display(),
            "d",
            "ambiguous: not adopted"
        );
        assert_eq!((s.matched, s.added, s.orphaned), (1, 1, 2));
        assert_eq!(s.references.len(), 4, "the two twins are kept as orphans");
        assert!(s.references[2..].iter().all(|r| r.source.is_none()));
        let dropped = sync_references(&b, &current, true);
        assert_eq!(dropped.references.len(), 2);
    }

    #[test]
    fn duplicate_ids_in_a_block_are_renumbered() {
        let mut b = (*block(vec![
            def(1, "A", 0, ""),
            def(1, "B", 0, ""),
            def(0, "C", 0, ""),
        ]))
        .clone();
        let mut next = 10;
        let changed = b.renumber_duplicate_attributes(|| {
            next += 1;
            AttributeDefinitionId(next)
        });
        let ids: Vec<_> = b.attribute_definitions().map(|(_, d)| d.id.0).collect();
        assert_eq!((changed, ids), (2, vec![1, 11, 12]));
    }
}
