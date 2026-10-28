//! The edits that delete purged items from a drawing, one action for each
//! table or set of objects involved.

use od_commands::{
    Action, RemoveEntities, RemoveLayer, SetBlocks, SetDimStyles, SetLinetypes, SetMLeaderStyles,
    SetTextStyles,
};
use od_core::purge::Item;
use od_core::Document;

/// The edits that delete `items` from `doc`, worked out from it.
pub(crate) fn delete_actions(doc: &Document, items: &[Item]) -> Vec<Box<dyn Action>> {
    let has = |f: &dyn Fn(&Item) -> bool| items.iter().any(f);
    let mut actions: Vec<Box<dyn Action>> = Vec::new();
    if has(&|i| matches!(i, Item::Block(_))) {
        let kept = doc
            .blocks()
            .iter()
            .filter(|b| !items.contains(&Item::Block(b.id)))
            .cloned()
            .collect();
        actions.push(Box::new(SetBlocks::new("Purge", kept)));
    }
    if has(&|i| matches!(i, Item::DimStyle(_))) {
        let kept = doc
            .dim_styles()
            .iter()
            .filter(|s| !items.contains(&Item::DimStyle(s.name.clone())))
            .cloned()
            .collect();
        let current = doc.dim_style().clone();
        actions.push(Box::new(SetDimStyles::new(
            "Purge",
            kept,
            current,
            Vec::new(),
        )));
    }
    for item in items {
        if let Item::Layer(id) = item {
            actions.push(Box::new(RemoveLayer::new(*id)));
        }
    }
    if has(&|i| matches!(i, Item::Linetype(_))) {
        let mut table = doc.linetypes().clone();
        for item in items {
            if let Item::Linetype(id) = item {
                // Special records are never offered; ignore a refusal.
                let _ = table.remove(*id);
            }
        }
        actions.push(Box::new(SetLinetypes::new(table, doc.linetype_scale())));
    }
    if has(&|i| matches!(i, Item::MLeaderStyle(_))) {
        let kept = doc
            .mleader_styles()
            .iter()
            .filter(|s| !items.contains(&Item::MLeaderStyle(s.name.clone())))
            .cloned()
            .collect();
        let (current, next) = (doc.current_mleader_style(), doc.next_mleader_style_id());
        actions.push(Box::new(SetMLeaderStyles::new(
            "Purge",
            kept,
            current,
            next,
            Vec::new(),
        )));
    }
    if has(&|i| matches!(i, Item::TextStyle(_))) {
        let kept = doc
            .text_styles()
            .iter()
            .filter(|s| !items.contains(&Item::TextStyle(s.id)))
            .cloned()
            .collect();
        let (current, next) = (doc.current_text_style(), doc.next_text_style_id());
        actions.push(Box::new(SetTextStyles::new(kept, current, next)));
    }
    let mut objects = Vec::new();
    for item in items {
        if let Item::ZeroLength(ids) | Item::EmptyText(ids) = item {
            objects.extend(ids.iter().copied());
        }
    }
    if !objects.is_empty() {
        actions.push(Box::new(RemoveEntities::new(objects)));
    }
    actions
}
