//! PURGE: what in a drawing nothing uses any more. Pure; `purgeable` lists
//! it and the command removes it through the document's own actions.

use std::sync::Arc;

use crate::text_style::TextStyleId;
use crate::{Attr, BlockDef, BlockId, Document, Entity, EntityId, LayerId};

/// A kind of purgeable thing, in the order the dialog lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    Blocks,
    DimStyles,
    Layers,
    Linetypes,
    MLeaderStyles,
    TextStyles,
    ZeroLength,
    EmptyText,
}

impl Category {
    pub const ALL: [Category; 8] = [
        Category::Blocks,
        Category::DimStyles,
        Category::Layers,
        Category::Linetypes,
        Category::MLeaderStyles,
        Category::TextStyles,
        Category::ZeroLength,
        Category::EmptyText,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Category::Blocks => "Blocks",
            Category::DimStyles => "Dimension styles",
            Category::Layers => "Layers",
            Category::Linetypes => "Linetypes",
            Category::MLeaderStyles => "Multileader styles",
            Category::TextStyles => "Text styles",
            Category::ZeroLength => "Zero-length geometry",
            Category::EmptyText => "Empty text objects",
        }
    }
}

/// One thing that can go. Styles and linetypes are named by their name, as
/// the drawing refers to them.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Block(BlockId),
    DimStyle(String),
    Layer(LayerId),
    Linetype(String),
    MLeaderStyle(String),
    TextStyle(TextStyleId),
    ZeroLength(Vec<EntityId>),
    EmptyText(Vec<EntityId>),
}

impl Item {
    pub fn category(&self) -> Category {
        match self {
            Item::Block(_) => Category::Blocks,
            Item::DimStyle(_) => Category::DimStyles,
            Item::Layer(_) => Category::Layers,
            Item::Linetype(_) => Category::Linetypes,
            Item::MLeaderStyle(_) => Category::MLeaderStyles,
            Item::TextStyle(_) => Category::TextStyles,
            Item::ZeroLength(_) => Category::ZeroLength,
            Item::EmptyText(_) => Category::EmptyText,
        }
    }
}

/// An item with the name to show for it.
#[derive(Clone, Debug, PartialEq)]
pub struct Purgeable {
    pub item: Item,
    pub name: String,
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

/// Everything that can go now, by category. Removing some of it can free
/// more (a layer a purged block was the last to use), so callers wanting
/// everything ask again until the list is empty.
pub fn purgeable(doc: &Document) -> Vec<Purgeable> {
    let mut out = Vec::new();
    let named = |item: Item, name: &str| Purgeable {
        item,
        name: name.to_owned(),
    };
    for block in unused_blocks(doc) {
        out.push(named(Item::Block(block.id), &block.name));
    }
    let dim_styles: Vec<_> = doc
        .dim_styles()
        .iter()
        .skip(1)
        .filter(|s| {
            !s.name.eq_ignore_ascii_case(&doc.dim_style().name) && !doc.dim_style_in_use(&s.name)
        })
        .collect();
    for style in &dim_styles {
        out.push(named(Item::DimStyle(style.name.clone()), &style.name));
    }
    for layer in doc.layers() {
        if doc.layer_delete_blocker(layer.id).is_none() {
            out.push(named(Item::Layer(layer.id), &layer.name));
        }
    }
    let new_linetype = match doc.new_object_style().1 {
        Attr::Set(name) => Some(name.as_str()),
        Attr::ByLayer => None,
    };
    for record in doc.linetypes().records() {
        let current = new_linetype.is_some_and(|n| n.eq_ignore_ascii_case(&record.name));
        if !record.id.is_special() && !current && !doc.linetype_in_use(&record.name) {
            out.push(named(Item::Linetype(record.name.clone()), &record.name));
        }
    }
    let mleader_styles: Vec<_> = doc
        .mleader_styles()
        .iter()
        .skip(1)
        .filter(|s| s.id != doc.current_mleader_style() && !doc.mleader_style_in_use(&s.name))
        .collect();
    for style in &mleader_styles {
        out.push(named(Item::MLeaderStyle(style.name.clone()), &style.name));
    }
    // A text style stays while a dimension or multileader style that stays
    // writes in it.
    let kept_by_styles = |id: TextStyleId| {
        doc.dim_styles()
            .iter()
            .filter(|s| !dim_styles.iter().any(|g| g.name == s.name))
            .any(|s| s.text_style == id)
            || doc
                .mleader_styles()
                .iter()
                .filter(|s| !mleader_styles.iter().any(|g| g.id == s.id))
                .any(|s| s.content.text_style == id)
    };
    for style in doc.text_styles().iter().skip(1) {
        if style.id != doc.current_text_style()
            && !doc.text_style_in_use(style.id)
            && !kept_by_styles(style.id)
        {
            out.push(named(Item::TextStyle(style.id), &style.name));
        }
    }
    let (zero, empty) = objects_to_purge(doc);
    if !zero.is_empty() {
        let name = count_name(zero.len());
        out.push(named(Item::ZeroLength(zero), &name));
    }
    if !empty.is_empty() {
        let name = count_name(empty.len());
        out.push(named(Item::EmptyText(empty), &name));
    }
    out
}

fn count_name(n: usize) -> String {
    format!("{n} object{}", if n == 1 { "" } else { "s" })
}

/// The zero-length lines, arcs and polylines, and the text with nothing in
/// it, leaving out objects that cannot be edited (a locked layer).
fn objects_to_purge(doc: &Document) -> (Vec<EntityId>, Vec<EntityId>) {
    let (mut zero, mut empty) = (Vec::new(), Vec::new());
    for object in doc.objects() {
        if doc.layer(object.attrs.layer).is_some_and(|l| l.locked) {
            continue;
        }
        match &object.entity {
            Entity::Line { .. } | Entity::Arc { .. } | Entity::Polyline { .. } => {
                if object.entity.measured().is_some_and(|m| m.length <= 1e-12) {
                    zero.push(object.id);
                }
            }
            Entity::MText(m) if m.contents.trim().is_empty() => empty.push(object.id),
            _ => {}
        }
    }
    (zero, empty)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Entity, MText, Point};

    fn line(x1: f64, x2: f64) -> Entity {
        Entity::Line {
            start: Point::new(x1, 0.0),
            end: Point::new(x2, 0.0),
        }
    }

    fn names(doc: &Document, category: Category) -> Vec<String> {
        purgeable(doc)
            .into_iter()
            .filter(|p| p.item.category() == category)
            .map(|p| p.name)
            .collect()
    }

    #[test]
    fn a_new_drawing_offers_only_its_unused_stock_linetypes() {
        let found = purgeable(&Document::default());
        assert!(!found.is_empty());
        assert!(found
            .iter()
            .all(|p| p.item.category() == Category::Linetypes));
    }

    #[test]
    fn zero_length_geometry_and_empty_text_are_found_and_the_rest_left() {
        let mut doc = Document::default();
        doc.add(line(0.0, 0.0));
        doc.add(line(0.0, 5.0));
        doc.add(Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: 0.0,
        });
        let empty = MText::new(Point::new(0.0, 0.0), 2.5, "  ", TextStyleId::STANDARD);
        doc.add(Entity::MText(Box::new(empty)));
        assert_eq!(names(&doc, Category::ZeroLength), ["1 object"]);
        assert_eq!(names(&doc, Category::EmptyText), ["1 object"]);
    }

    #[test]
    fn a_layer_goes_only_when_nothing_is_on_it_and_it_is_not_current_or_zero() {
        let mut doc = Document::default();
        let id = doc.allocate_layer_id();
        doc.push_layer(crate::Layer::new(id, "Spare"));
        assert_eq!(names(&doc, Category::Layers), ["Spare"]);
        doc.add_with(line(0.0, 1.0), crate::Attributes::on_layer(id));
        assert!(names(&doc, Category::Layers).is_empty());
    }

    #[test]
    fn a_linetype_goes_only_when_no_layer_object_or_default_uses_it() {
        let mut doc = Document::default();
        assert!(names(&doc, Category::Linetypes).contains(&"DASHED".to_owned()));
        let mut attrs = crate::Attributes::on_layer(LayerId::DEFAULT);
        attrs.linetype = Attr::Set("DASHED".to_owned());
        doc.add_with(line(0.0, 1.0), attrs);
        let left = names(&doc, Category::Linetypes);
        assert!(!left.contains(&"DASHED".to_owned()) && left.contains(&"DOT".to_owned()));
    }
}
