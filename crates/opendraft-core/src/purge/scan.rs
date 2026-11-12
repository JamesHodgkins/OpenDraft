//! Finding what nothing uses, and what each thing that stays is used by.

use std::sync::Arc;

use super::{Blocked, Item, Purgeable};
use crate::document::LinetypeUse;
use crate::text_style::TextStyleId;
use crate::{BlockDef, BlockId, Document, Entity, EntityId, LayerId};

/// A named record with every reason it must stay; none means it can go.
struct Entry {
    item: Item,
    name: String,
    reasons: Vec<String>,
}

fn counted(n: usize, noun: &str) -> String {
    format!("{n} {noun}{}", if n == 1 { "" } else { "s" })
}

/// The blocks nothing uses, after removing others that only they used, in
/// table order.
pub fn unused_blocks(doc: &Document) -> Vec<Arc<BlockDef>> {
    let mut kept: Vec<Arc<BlockDef>> = doc.blocks().to_vec();
    loop {
        let used = |id: BlockId| {
            !doc.block_references(id).is_empty()
                || kept
                    .iter()
                    .any(|b| b.id != id && b.nested_blocks().any(|n| n == id))
        };
        let before = kept.len();
        let next: Vec<_> = kept.iter().filter(|b| used(b.id)).cloned().collect();
        kept = next;
        if kept.len() == before {
            break;
        }
    }
    doc.blocks()
        .iter()
        .filter(|b| !kept.iter().any(|k| k.id == b.id))
        .cloned()
        .collect()
}

fn block_entries(doc: &Document, out: &mut Vec<Entry>) {
    let unused = unused_blocks(doc);
    let gone = |id: BlockId| unused.iter().any(|u| u.id == id);
    for block in doc.blocks() {
        let mut reasons = Vec::new();
        if !gone(block.id) {
            let refs = doc.block_references(block.id).len();
            if refs > 0 {
                reasons.push(format!("Inserted {}", counted(refs, "time")));
            }
            for outer in doc.blocks() {
                if outer.id != block.id
                    && !gone(outer.id)
                    && outer.nested_blocks().any(|n| n == block.id)
                {
                    reasons.push(format!("Used inside block \"{}\"", outer.name));
                }
            }
        }
        out.push(Entry {
            item: Item::Block(block.id),
            name: block.name.clone(),
            reasons,
        });
    }
}

fn dim_style_entries(doc: &Document, out: &mut Vec<Entry>) {
    for (i, style) in doc.dim_styles().iter().enumerate() {
        let mut reasons = Vec::new();
        if i == 0 {
            reasons.push("The Standard style is permanent".to_owned());
        }
        if style.name.eq_ignore_ascii_case(&doc.dim_style().name) {
            reasons.push("It is the current dimension style".to_owned());
        }
        if doc.dim_style_in_use(&style.name) {
            reasons.push("Used by dimensions".to_owned());
        }
        out.push(Entry {
            item: Item::DimStyle(style.name.clone()),
            name: style.name.clone(),
            reasons,
        });
    }
}

fn frozen_in_viewport(doc: &Document, id: LayerId) -> bool {
    doc.layouts()
        .iter()
        .any(|l| l.viewports.iter().any(|v| v.is_frozen(id)))
}

fn layer_entries(doc: &Document, out: &mut Vec<Entry>) {
    for layer in doc.layers() {
        let mut reasons = Vec::new();
        if layer.id == LayerId::DEFAULT {
            reasons.push("Layer 0 is permanent".to_owned());
        }
        if layer.id == doc.current_layer() {
            reasons.push("It is the current layer".to_owned());
        }
        let on = doc
            .objects()
            .iter()
            .filter(|o| o.attrs.layer == layer.id)
            .count();
        if on > 0 {
            reasons.push(format!("{} on it", counted(on, "object")));
        } else if doc.layer_in_use(layer.id) {
            reasons.push("Used inside a block".to_owned());
        }
        if frozen_in_viewport(doc, layer.id) {
            reasons.push("Frozen in a viewport".to_owned());
        }
        out.push(Entry {
            item: Item::Layer(layer.id),
            name: layer.name.clone(),
            reasons,
        });
    }
}

fn linetype_entries(doc: &Document, out: &mut Vec<Entry>) {
    for record in doc.linetypes().records() {
        let mut reasons = Vec::new();
        if record.id.is_special() {
            reasons.push("It is a permanent linetype".to_owned());
        }
        let (mut by_layer, mut by_objects) = (false, false);
        for (_, who) in doc.linetype_uses().filter(|(id, _)| *id == record.id) {
            match who {
                LinetypeUse::NewObjects => {
                    reasons.push("New objects are drawn with it".to_owned());
                }
                LinetypeUse::Layer(layer) => {
                    by_layer = true;
                    reasons.push(format!("Used by layer \"{}\"", layer.name));
                }
                LinetypeUse::MLeaderStyle(style) => {
                    reasons.push(format!("Used by multileader style \"{}\"", style.name));
                }
                LinetypeUse::Object(_) | LinetypeUse::BlockObject(_) => by_objects = true,
            }
        }
        if by_objects && !by_layer {
            reasons.push("Used by objects or blocks".to_owned());
        }
        out.push(Entry {
            item: Item::Linetype(record.id),
            name: record.name.clone(),
            reasons,
        });
    }
}

fn mleader_style_entries(doc: &Document, out: &mut Vec<Entry>) {
    for (i, style) in doc.mleader_styles().iter().enumerate() {
        let mut reasons = Vec::new();
        if i == 0 {
            reasons.push("The Standard style is permanent".to_owned());
        }
        if style.id == doc.current_mleader_style() {
            reasons.push("It is the current multileader style".to_owned());
        }
        if doc.mleader_style_in_use(&style.name) {
            reasons.push("Used by multileaders".to_owned());
        }
        out.push(Entry {
            item: Item::MLeaderStyle(style.name.clone()),
            name: style.name.clone(),
            reasons,
        });
    }
}

/// Text styles also stay for the dimension and multileader styles that
/// stay, so those are worked out first (`styles`).
fn text_style_entries(doc: &Document, styles: &[Entry], out: &mut Vec<Entry>) {
    let going = |item: &Item| {
        styles
            .iter()
            .any(|e| e.item == *item && e.reasons.is_empty())
    };
    for (i, style) in doc.text_styles().iter().enumerate() {
        let mut reasons = Vec::new();
        if i == 0 {
            reasons.push("The Standard style is permanent".to_owned());
        }
        if style.id == doc.current_text_style() {
            reasons.push("It is the current text style".to_owned());
        }
        if doc.text_style_in_use(style.id) {
            reasons.push("Used by text, dimensions or multileaders".to_owned());
        }
        for d in doc.dim_styles() {
            if d.text_style == style.id && !going(&Item::DimStyle(d.name.clone())) {
                reasons.push(format!("Used by dimension style \"{}\"", d.name));
            }
        }
        for m in doc.mleader_styles() {
            if m.content.text_style == style.id && !going(&Item::MLeaderStyle(m.name.clone())) {
                reasons.push(format!("Used by multileader style \"{}\"", m.name));
            }
        }
        out.push(Entry {
            item: Item::TextStyle(style.id),
            name: style.name.clone(),
            reasons,
        });
    }
}

/// Every named record with its reasons, in the order of
/// [`Category::NAMED`](super::Category::NAMED).
fn entries(doc: &Document) -> Vec<Entry> {
    let mut styles = Vec::new();
    dim_style_entries(doc, &mut styles);
    mleader_style_entries(doc, &mut styles);
    let mut out = Vec::new();
    block_entries(doc, &mut out);
    dim_style_entries(doc, &mut out);
    layer_entries(doc, &mut out);
    linetype_entries(doc, &mut out);
    mleader_style_entries(doc, &mut out);
    text_style_entries(doc, &styles, &mut out);
    out
}

/// Every named item nothing uses.
pub fn purgeable(doc: &Document) -> Vec<Purgeable> {
    entries(doc)
        .into_iter()
        .filter(|e| e.reasons.is_empty())
        .map(|e| Purgeable {
            item: e.item,
            name: e.name,
        })
        .collect()
}

/// Every named item that has to stay, with each reason.
pub fn blocked(doc: &Document) -> Vec<Blocked> {
    entries(doc)
        .into_iter()
        .filter(|e| !e.reasons.is_empty())
        .map(|e| Blocked {
            item: e.item,
            name: e.name,
            reasons: e.reasons,
        })
        .collect()
}

/// The objects PURGE can delete without a name: zero-length lines, arcs,
/// circles and polylines, and text with nothing in it. Objects on locked
/// layers are left alone.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Unnamed {
    pub zero_length: Vec<EntityId>,
    pub empty_text: Vec<EntityId>,
}

pub fn unnamed(doc: &Document) -> Unnamed {
    unnamed_with(doc, false)
}

/// [`unnamed`], and with `include_locked` also those on locked layers: a
/// look at the drawing (an audit) reports them though PURGE leaves them.
pub fn unnamed_with(doc: &Document, include_locked: bool) -> Unnamed {
    let mut found = Unnamed::default();
    for object in doc.objects() {
        if !include_locked && doc.layer(object.attrs.layer).is_some_and(|l| l.locked) {
            continue;
        }
        match &object.entity {
            Entity::Line { .. }
            | Entity::Arc { .. }
            | Entity::Circle { .. }
            | Entity::Polyline { .. } => {
                if object.entity.measured().is_some_and(|m| m.length == 0.0) {
                    found.zero_length.push(object.id);
                }
            }
            Entity::MText(m) if m.contents.trim().is_empty() => found.empty_text.push(object.id),
            Entity::Text(t) if t.contents.trim().is_empty() => found.empty_text.push(object.id),
            _ => {}
        }
    }
    found
}

/// The objects in the drawing's spaces that use `item`, for selecting them.
/// Uses inside blocks and styles have no object to select.
pub fn users_of(doc: &Document, item: &Item) -> Vec<EntityId> {
    let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    let text_style = |id: TextStyleId, e: &Entity| match e {
        Entity::MText(m) => m.style == id,
        Entity::Text(t) => t.style() == id,
        Entity::MLeader(m) => m.text().is_some_and(|t| t.text.style == id),
        Entity::Dimension(d) => d.style.text_style == id,
        _ => false,
    };
    let used = |o: &crate::Object| match item {
        Item::Block(id) => matches!(&o.entity, Entity::Insert(i) if i.block.holds(*id)),
        Item::Layer(id) => o.attrs.layer == *id,
        Item::Linetype(id) => doc.linetype_of(o) == *id,
        Item::DimStyle(name) => {
            matches!(&o.entity, Entity::Dimension(d) if same(&d.style.name, name))
        }
        Item::MLeaderStyle(name) => {
            matches!(&o.entity, Entity::MLeader(m) if same(&m.style_name, name))
        }
        Item::TextStyle(id) => text_style(*id, &o.entity),
        Item::ZeroLength(_) | Item::EmptyText(_) => false,
    };
    doc.objects()
        .iter()
        .filter(|o| used(o))
        .map(|o| o.id)
        .collect()
}
