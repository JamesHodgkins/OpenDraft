//! `drawing.summary`: what is in a drawing, or a window of it, without
//! reading every entity: counts by kind and layer, the blocks and their
//! positions, the text a reader sees, the box round it all, and a coarse
//! density grid. It is the overview a caller uses to decide where to look
//! closer with `entities.query` or `view.render`.

use std::collections::{BTreeMap, HashMap};

use od_api::audit_ops::{
    BlockCount, Density, KindCount, LayerCount, SummaryArgs, SummaryResult, TextItem,
};
use od_api::entity::PointDto;
use od_api::limits::{DENSITY_CELLS, LISTED_DEFAULT, LISTED_MAX};
use od_api::{ApiError, WireId};
use od_core::{Bounds, Entity, LayerId, MText, Point};
use serde_json::Value;

use super::command_runs::encode;
use super::read::parse;
use super::resolve;
use super::scope::{finite_bounds, objects_in, space_of};
use crate::state::AppState;

/// Longest text shown for one object.
const TEXT_MAX: usize = 120;

/// What a reader sees of a text, on one line and bounded. Text the parser
/// cannot read is shown as written.
fn one_line(m: &MText) -> String {
    let plain = od_core::text::parse(&m.contents)
        .map(|parsed| parsed.plain_text(&m.contents))
        .unwrap_or_else(|_| m.contents.clone());
    let lines: Vec<&str> = plain.lines().map(str::trim).collect();
    let flat: String = lines
        .join(" / ")
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    flat.chars().take(TEXT_MAX).collect()
}

/// The cell a coordinate falls in along one side of the grid.
fn cell(value: f64, low: f64, high: f64) -> usize {
    if high <= low {
        return 0;
    }
    let along = (value - low) / (high - low) * DENSITY_CELLS as f64;
    (along.floor().max(0.0) as usize).min(DENSITY_CELLS - 1)
}

fn density(window: Bounds, centres: &[Point]) -> Density {
    let mut counts = vec![vec![0usize; DENSITY_CELLS]; DENSITY_CELLS];
    let inside = |c: &&Point| {
        (window.min.x..=window.max.x).contains(&c.x) && (window.min.y..=window.max.y).contains(&c.y)
    };
    for c in centres.iter().filter(inside) {
        let row = cell(c.y, window.min.y, window.max.y);
        let column = cell(c.x, window.min.x, window.max.x);
        counts[row][column] += 1;
    }
    Density {
        bounds: window.into(),
        columns: DENSITY_CELLS,
        rows: DENSITY_CELLS,
        counts,
    }
}

/// `drawing.summary`.
pub(super) fn summary(
    state: &AppState,
    instance: &str,
    arguments: Value,
) -> Result<Value, ApiError> {
    let args: SummaryArgs = parse(arguments)?;
    let doc = resolve(state, instance, &args.document_id)?;
    let document = doc.session.document();
    let space = space_of(document, args.space.as_ref())?;
    let in_scope = objects_in(document, space, args.bounds.as_ref())?;
    let limit = args.limit.unwrap_or(LISTED_DEFAULT).clamp(1, LISTED_MAX);

    let mut kinds: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut layers: HashMap<LayerId, usize> = HashMap::new();
    let mut blocks: BTreeMap<String, (usize, Point)> = BTreeMap::new();
    let mut texts = Vec::new();
    let mut texts_total = 0usize;
    let mut union: Option<Bounds> = None;
    let mut centres = Vec::new();
    for object in &in_scope {
        *kinds.entry(object.entity.kind().wire_name()).or_default() += 1;
        *layers.entry(object.attrs.layer).or_default() += 1;
        match &object.entity {
            Entity::Insert(insert) => {
                blocks
                    .entry(insert.block.name.clone())
                    .and_modify(|(count, _)| *count += 1)
                    .or_insert((1, insert.insertion));
            }
            Entity::MText(m) => {
                texts_total += 1;
                if texts.len() < limit {
                    texts.push(TextItem {
                        entity_id: WireId(object.id.0),
                        text: one_line(m),
                        at: m.insertion.into(),
                    });
                }
            }
            _ => {}
        }
        let b = object.bounds();
        if b.is_finite() {
            union = Some(union.map_or(b, |u| u.union(b)));
            centres.push(Point::new(
                (b.min.x + b.max.x) / 2.0,
                (b.min.y + b.max.y) / 2.0,
            ));
        }
    }

    let mut by_kind: Vec<KindCount> = kinds
        .into_iter()
        .map(|(kind, count)| KindCount {
            kind: kind.to_owned(),
            count,
        })
        .collect();
    by_kind.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.kind.cmp(&b.kind)));

    let layers_total = layers.len();
    let mut by_layer: Vec<LayerCount> = layers
        .into_iter()
        .map(|(id, count)| LayerCount {
            layer_id: WireId(id.0),
            name: document
                .layer(id)
                .map(|l| l.name.chars().filter(|c| !c.is_control()).collect())
                .unwrap_or_default(),
            count,
        })
        .collect();
    by_layer.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
    by_layer.truncate(limit);

    let blocks_total = blocks.len();
    let mut block_list: Vec<BlockCount> = blocks
        .into_iter()
        .map(|(name, (count, at))| BlockCount {
            name: name.chars().filter(|c| !c.is_control()).collect(),
            count,
            first_at: PointDto::from(at),
        })
        .collect();
    block_list.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
    block_list.truncate(limit);

    // The grid covers the window asked for, else everything that has a box.
    let window = match &args.bounds {
        Some(b) => Some(finite_bounds(b)?),
        None => union,
    };
    encode(&SummaryResult {
        document_id: args.document_id,
        revision: WireId(doc.session.revision()),
        scanned: in_scope.len(),
        bounds: union.map(Into::into),
        by_kind,
        by_layer,
        layers_total,
        blocks: block_list,
        blocks_total,
        texts,
        texts_total,
        density: window.map(|w| density(w, &centres)),
    })
}
