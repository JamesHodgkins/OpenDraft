//! The native format's block table and block references.
//!
//! A definition is written once, in the drawing's `blocks` list, with its
//! objects in the block's own coordinates. An insert names its block by id.
//! Definitions may nest in any order (a redefined block can insert a newer
//! one), so reading resolves them by id, refusing a block that inserts
//! itself, however indirectly, or nests deeper than the core's limit.

use std::collections::HashMap;
use std::sync::Arc;

use od_core::block::{block_name_problem, MAX_EXPANSION, MAX_NESTING};
use od_core::{
    Attributes, BlockDef, BlockEntity, BlockId, Color, DimStyle, Entity, LayerId, Lineweight,
};
use serde::{Deserialize, Serialize};

use super::dimension_style::StylePool;
use super::entity::{from_point, invalid, point, EntityDto, PointDto};
use super::{attr_from_dto, attr_to_dto, AttrDto, ColorDto};
use crate::FileError;

/// Definitions already read, by id, for resolving inserts.
pub(super) type Blocks = HashMap<u64, Arc<BlockDef>>;

#[derive(Serialize, Deserialize)]
pub(super) struct BlockDto {
    id: u64,
    name: String,
    base: PointDto,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    description: String,
    /// In draw order.
    objects: Vec<BlockObjectDto>,
}

#[derive(Serialize, Deserialize)]
struct BlockObjectDto {
    layer: u64,
    color: AttrDto<ColorDto>,
    linetype: AttrDto<String>,
    lineweight: AttrDto<u16>,
    #[serde(flatten)]
    scale: super::linetype::ScaleDto,
    entity: EntityDto,
}

impl BlockDto {
    pub(super) fn from_block(block: &BlockDef, styles: &mut StylePool) -> Result<Self, FileError> {
        let objects = block
            .entities
            .iter()
            .map(|e| {
                Ok(BlockObjectDto {
                    layer: e.attrs.layer.0,
                    color: attr_to_dto(&e.attrs.color, |c| (*c).into()),
                    linetype: attr_to_dto(&e.attrs.linetype, String::clone),
                    lineweight: attr_to_dto(&e.attrs.lineweight, |w| w.0),
                    scale: super::linetype::ScaleDto::of(&e.attrs),
                    entity: EntityDto::from_entity(&e.entity, styles)?,
                })
            })
            .collect::<Result<_, FileError>>()?;
        Ok(Self {
            id: block.id.0,
            name: block.name.clone(),
            base: point(block.base),
            description: block.description.clone(),
            objects,
        })
    }
}

/// Every definition, resolved, in the file's order. `layer_ok` says whether
/// a layer id exists.
pub(super) fn load_blocks(
    dtos: &[BlockDto],
    dim_styles: &[DimStyle],
    layer_ok: impl Fn(u64) -> bool,
) -> Result<Vec<Arc<BlockDef>>, FileError> {
    let mut by_id = HashMap::new();
    for dto in dtos {
        if by_id.insert(dto.id, dto).is_some() {
            return invalid(format!("block {} appears twice", dto.id));
        }
        if let Some(why) = block_name_problem(&dto.name) {
            return invalid(format!("block \"{}\": {why}", dto.name));
        }
    }
    let mut done = Blocks::new();
    let mut path = Vec::new();
    let mut sizes = HashMap::new();
    for dto in dtos {
        resolve(
            dto.id, &by_id, &layer_ok, dim_styles, &mut done, &mut path, &mut sizes,
        )?;
    }
    Ok(dtos
        .iter()
        .filter_map(|d| done.get(&d.id).cloned())
        .collect())
}

fn resolve(
    id: u64,
    by_id: &HashMap<u64, &BlockDto>,
    layer_ok: &impl Fn(u64) -> bool,
    dim_styles: &[DimStyle],
    done: &mut Blocks,
    path: &mut Vec<u64>,
    sizes: &mut HashMap<BlockId, usize>,
) -> Result<Arc<BlockDef>, FileError> {
    if let Some(block) = done.get(&id) {
        return Ok(block.clone());
    }
    let Some(dto) = by_id.get(&id) else {
        return invalid(format!(
            "an insert refers to block {id}, which does not exist"
        ));
    };
    if path.contains(&id) {
        return invalid(format!("block \"{}\" inserts itself", dto.name));
    }
    if path.len() >= MAX_NESTING {
        return invalid(format!("block \"{}\" is nested too deeply", dto.name));
    }
    path.push(id);
    for object in &dto.objects {
        if let EntityDto::Insert { block, .. } = &object.entity {
            resolve(*block, by_id, layer_ok, dim_styles, done, path, sizes)?;
        }
    }
    path.pop();
    let mut entities = Vec::with_capacity(dto.objects.len());
    for object in &dto.objects {
        if !layer_ok(object.layer) {
            return invalid(format!(
                "block \"{}\" has an object on layer {}, which does not exist",
                dto.name, object.layer
            ));
        }
        let mut attrs = Attributes {
            layer: LayerId(object.layer),
            color: attr_from_dto(object.color.clone(), Color::from),
            linetype: attr_from_dto(object.linetype.clone(), |s| s),
            lineweight: attr_from_dto(object.lineweight.clone(), Lineweight),
            ..Attributes::on_layer(LayerId(object.layer))
        };
        object.scale.apply(&mut attrs);
        entities.push(BlockEntity {
            entity: object.entity.to_entity(done, dim_styles)?,
            attrs,
        });
    }
    let block = Arc::new(BlockDef {
        id: BlockId(id),
        name: dto.name.trim().to_owned(),
        base: from_point(dto.base)?,
        description: dto.description.clone(),
        entities,
    });
    if block.expansion_size(sizes) > MAX_EXPANSION {
        return invalid(format!(
            "block \"{}\" expands to too many objects (its inserts nest and repeat other blocks)",
            block.name
        ));
    }
    done.insert(id, block.clone());
    Ok(block)
}

/// The entity for an insert's fields, resolving its block in `blocks`.
pub(super) fn insert_entity(
    blocks: &Blocks,
    block: u64,
    insertion: PointDto,
    scale: [f64; 2],
    rotation: f64,
) -> Result<Entity, FileError> {
    let Some(def) = blocks.get(&block) else {
        return invalid(format!(
            "an insert refers to block {block}, which does not exist"
        ));
    };
    let insert = od_core::Insert {
        block: def.clone(),
        insertion: from_point(insertion)?,
        scale_x: scale[0],
        scale_y: scale[1],
        rotation,
    };
    if !insert.is_valid() {
        return invalid("a block insert has a zero or non-finite scale or rotation");
    }
    Ok(Entity::Insert(Box::new(insert)))
}

#[cfg(test)]
mod tests {
    use od_core::{Attr, Document, Insert, Layer, Point};

    use crate::{FileError, FileFormat, NativeFormat};

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    /// Two blocks, the second inserting the first, on a layer of their own,
    /// placed twice in the drawing.
    fn nested() -> Document {
        let mut doc = Document::default();
        let walls = doc.allocate_layer_id();
        doc.push_layer(Layer::new(walls, "Walls"));
        let inner = Arc::new(BlockDef {
            id: doc.allocate_block_id(),
            name: "Inner".into(),
            base: p(1.0, 1.0),
            description: "a circle".into(),
            entities: vec![
                BlockEntity {
                    entity: Entity::Circle {
                        center: p(1.0, 1.0),
                        radius: 0.5,
                    },
                    attrs: Attributes {
                        color: Attr::Set(Color::Rgb(0, 128, 255)),
                        ..Attributes::on_layer(walls)
                    },
                },
                BlockEntity {
                    entity: Entity::Ellipse(
                        od_core::Ellipse::new(p(1.0, 1.0), p(0.0, 2.0), 0.5, 1.0, 4.0).unwrap(),
                    ),
                    attrs: Attributes::on_layer(walls),
                },
                BlockEntity {
                    entity: Entity::Spline(Box::new(
                        od_core::Spline::from_fit_points(
                            vec![p(0.0, 0.0), p(1.0, 2.0), p(3.0, 1.0)],
                            od_core::KnotParameterization::Chord,
                        )
                        .unwrap(),
                    )),
                    attrs: Attributes::on_layer(walls),
                },
            ],
        });
        let mut placed = Insert::new(inner.clone(), p(3.0, 0.0));
        placed.scale_x = -2.0;
        placed.rotation = 0.5;
        let outer = Arc::new(BlockDef {
            id: doc.allocate_block_id(),
            name: "Outer".into(),
            base: p(0.0, 0.0),
            description: String::new(),
            entities: vec![BlockEntity {
                entity: Entity::Insert(Box::new(placed)),
                attrs: Attributes::on_layer(LayerId::DEFAULT),
            }],
        });
        doc.push_block(inner);
        doc.push_block(outer.clone());
        doc.add(Entity::Insert(Box::new(Insert::new(
            outer.clone(),
            p(0.0, 0.0),
        ))));
        let mut second = Insert::new(outer, p(10.0, 5.0));
        second.scale_y = 3.0;
        doc.add(Entity::Insert(Box::new(second)));
        doc
    }

    fn round_trip(doc: &Document) -> Document {
        let bytes = NativeFormat.write(doc).unwrap().bytes;
        NativeFormat.read(&bytes).unwrap().document
    }

    #[test]
    fn blocks_and_their_references_survive_a_round_trip() {
        let doc = nested();
        let back = round_trip(&doc);
        assert_eq!(back.blocks(), doc.blocks());
        assert_eq!(back.objects(), doc.objects());
        assert_eq!(back.next_block_id(), doc.next_block_id());
        assert_eq!(back.validate(), Ok(()));
    }

    #[test]
    fn a_block_defined_before_the_one_it_inserts_still_reads() {
        let text = String::from_utf8(NativeFormat.write(&nested()).unwrap().bytes).unwrap();
        let mut file: serde_json::Value = serde_json::from_str(&text).unwrap();
        let blocks = file["drawing"]["blocks"].as_array_mut().unwrap();
        blocks.reverse();
        let back = NativeFormat
            .read(file.to_string().as_bytes())
            .unwrap()
            .document;
        assert_eq!(back.blocks()[0].name, "Outer");
        assert_eq!(back.validate(), Ok(()));
    }

    fn refused(edit: impl FnOnce(&mut serde_json::Value)) -> String {
        let text = String::from_utf8(NativeFormat.write(&nested()).unwrap().bytes).unwrap();
        let mut file: serde_json::Value = serde_json::from_str(&text).unwrap();
        edit(&mut file);
        match NativeFormat.read(file.to_string().as_bytes()) {
            Err(FileError::Invalid(why)) => why,
            Err(other) => panic!("{other:?}"),
            Ok(_) => panic!("a damaged file was loaded"),
        }
    }

    /// `levels` blocks above an empty one, each inserting the one below twice.
    fn doubling_chain(f: &mut serde_json::Value, levels: u64) {
        let template = f["drawing"]["blocks"][1]["objects"][0].clone();
        let block = |id: u64, objects: Vec<serde_json::Value>| {
            let mut b = f["drawing"]["blocks"][0].clone();
            b["id"] = serde_json::json!(id);
            b["name"] = serde_json::json!(format!("level{id}"));
            b["objects"] = serde_json::json!(objects);
            b
        };
        let mut blocks = vec![block(0, Vec::new())];
        for id in 1..=levels {
            let mut insert = template.clone();
            insert["entity"]["block"] = serde_json::json!(id - 1);
            blocks.push(block(id, vec![insert.clone(), insert]));
        }
        f["drawing"]["blocks"] = serde_json::json!(blocks);
        f["drawing"]["next_block_id"] = serde_json::json!(levels + 1);
        f["drawing"]["objects"][0]["entity"]["block"] = serde_json::json!(levels);
        f["drawing"]["objects"] = serde_json::json!([f["drawing"]["objects"][0].clone()]);
    }

    #[test]
    fn blocks_that_repeat_each_other_into_millions_of_objects_are_refused() {
        let text = String::from_utf8(NativeFormat.write(&nested()).unwrap().bytes).unwrap();
        let mut file: serde_json::Value = serde_json::from_str(&text).unwrap();
        // 2^15 objects: large, but fine.
        doubling_chain(&mut file, 15);
        let back = NativeFormat.read(file.to_string().as_bytes());
        assert!(back.is_ok(), "{:?}", back.err());
        // 2^40, from forty small definitions.
        let why = refused(|f| doubling_chain(f, 40));
        assert!(why.contains("too many objects"), "{why}");
    }

    #[test]
    fn loops_missing_blocks_and_zero_scales_are_refused() {
        let why = refused(|f| {
            // Make Inner insert Outer, which inserts Inner.
            let outer_object = f["drawing"]["blocks"][1]["objects"][0].clone();
            f["drawing"]["blocks"][0]["objects"] = serde_json::json!([outer_object]);
            f["drawing"]["blocks"][0]["objects"][0]["entity"]["block"] = serde_json::json!(2);
        });
        assert!(why.contains("inserts itself"), "{why}");
        let why = refused(|f| f["drawing"]["objects"][0]["entity"]["block"] = serde_json::json!(9));
        assert!(why.contains("does not exist"), "{why}");
        let why = refused(|f| {
            f["drawing"]["objects"][0]["entity"]["scale"] = serde_json::json!([0.0, 1.0])
        });
        assert!(why.contains("zero"), "{why}");
        let why = refused(|f| f["drawing"]["blocks"][0]["name"] = serde_json::json!("a|b"));
        assert!(why.contains("cannot contain"), "{why}");
    }
}
