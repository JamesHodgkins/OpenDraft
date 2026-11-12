//! Getting objects ready for [`super::overkill`]: which compare against
//! which, and which must not change. Shared by the OVERKILL command and the
//! audit, so both read a drawing the same way.

use std::collections::HashSet;

use super::{Candidate, Ignore, Settings};
use crate::{
    Attr, Attributes, Color, Document, Entity, EntityId, LayerId, LayoutId, LinetypeId, Lineweight,
};

/// What must match for two objects to be tidied against each other.
#[derive(PartialEq)]
struct Key {
    layout: Option<LayoutId>,
    layer: Option<LayerId>,
    color: Option<Attr<Color>>,
    linetype: Option<Attr<LinetypeId>>,
    linetype_scale: Option<u64>,
    lineweight: Option<Attr<Lineweight>>,
}

fn key(layout: Option<LayoutId>, attrs: &Attributes, ignore: Ignore) -> Key {
    Key {
        layout,
        layer: (!ignore.layer).then_some(attrs.layer),
        color: (!ignore.color).then(|| attrs.color.clone()),
        linetype: (!ignore.linetype).then(|| attrs.linetype.clone()),
        linetype_scale: (!ignore.linetype_scale).then(|| attrs.linetype_scale.to_bits()),
        lineweight: (!ignore.lineweight).then(|| attrs.lineweight.clone()),
    }
}

/// The objects other objects are tied to: what an associative hatch was
/// made from, what a dimension's points sit on, what a leader annotates.
fn tied_to(doc: &Document) -> HashSet<EntityId> {
    let mut tied = HashSet::new();
    for object in doc.objects() {
        match &object.entity {
            Entity::Dimension(d) => tied.extend(d.assoc.iter().map(|a| a.target)),
            Entity::Hatch(h) if h.associative => tied.extend(h.source_ids()),
            Entity::Leader(l) => tied.extend(l.annotation),
            _ => {}
        }
    }
    tied
}

/// The objects chosen, ready to compare.
pub struct Prepared {
    /// The drawing's id for each candidate, in the same order.
    pub ids: Vec<EntityId>,
    pub items: Vec<Candidate>,
    /// Objects left out because they are on hidden, frozen or locked layers
    /// (never when `include_unselectable` was asked for).
    pub skipped: usize,
}

impl Prepared {
    /// How many candidates must not be deleted or changed.
    pub fn protected(&self) -> usize {
        self.items.iter().filter(|c| c.protected).count()
    }
}

/// Group `chosen` by what must match under `settings`, and mark the objects
/// that others are tied to (when `settings.maintain_associative`). Objects
/// that are gone are dropped; those that cannot be picked (hidden, frozen or
/// locked) are skipped unless `include_unselectable`, which a read-only look
/// asks for.
pub fn prepare(
    doc: &Document,
    chosen: &[EntityId],
    settings: &Settings,
    include_unselectable: bool,
) -> Prepared {
    let tied = if settings.maintain_associative {
        tied_to(doc)
    } else {
        HashSet::new()
    };
    let mut skipped = 0usize;
    let mut found = Vec::new();
    for id in chosen {
        let Some(o) = doc.object(*id) else { continue };
        if include_unselectable || doc.is_selectable(o) {
            found.push((
                *id,
                o.entity.clone(),
                key(o.layout, &o.attrs, settings.ignore),
            ));
        } else {
            skipped += 1;
        }
    }
    let mut keys: Vec<&Key> = Vec::new();
    let mut ids = Vec::with_capacity(found.len());
    let mut items = Vec::with_capacity(found.len());
    for (id, entity, k) in &found {
        let group = keys.iter().position(|seen| *seen == k).unwrap_or_else(|| {
            keys.push(k);
            keys.len() - 1
        });
        ids.push(*id);
        items.push(Candidate {
            entity: entity.clone(),
            group,
            protected: tied.contains(id),
        });
    }
    Prepared {
        ids,
        items,
        skipped,
    }
}
