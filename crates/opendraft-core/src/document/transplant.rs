//! Moving objects from one drawing into another (the clipboard).
//!
//! [`Document::extract`] makes a small drawing of some objects and every
//! table they might need. [`Document::absorb`] brings such a drawing's
//! objects into another one, matching layers, text styles, multileader and
//! dimension styles, linetypes and blocks by name and copying the ones the
//! target lacks. What the target already has by a name wins, as in
//! AutoCAD. The change can be undone and redone exactly with
//! [`Document::unabsorb`] and [`Document::reabsorb`].

use std::collections::HashMap;
use std::sync::Arc;

use super::{after, Document};
use crate::block::{BlockDef, BlockEntity, BlockId, Insert};
use crate::dimension::DimStyle;
use crate::layer::{Attr, Attributes, Layer, LayerId};
use crate::layout::LayoutId;
use crate::linetype::{LinetypeId, LinetypeTable};
use crate::mleader::{MLeaderContent, MLeaderStyle};
use crate::text_style::{TextStyle, TextStyleId};
use crate::{Entity, EntityId, Object, Point};

/// Where the absorbed objects go.
#[derive(Clone, Debug, PartialEq)]
pub enum Placement {
    /// As separate objects, moved by `(dx, dy)`.
    Loose { dx: f64, dy: f64 },
    /// As one new block called `name` (base point `base` in the source's
    /// coordinates) and a reference to it at `at`.
    Block {
        name: String,
        base: Point,
        at: Point,
    },
}

/// Every table `absorb` can add to, as it stood at one moment.
#[derive(Clone, Debug, PartialEq)]
struct Tables {
    layers: Vec<Layer>,
    next_layer_id: u64,
    text_styles: Vec<TextStyle>,
    next_text_style_id: u64,
    mleader_styles: Vec<MLeaderStyle>,
    next_mleader_style_id: u64,
    dim_styles: Vec<DimStyle>,
    linetypes: LinetypeTable,
    blocks: Vec<Arc<BlockDef>>,
    next_block_id: u64,
}

/// Something a transfer between drawings could not carry over exactly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransferWarning {
    /// The drawing already has a linetype of this name with another pattern;
    /// the drawing's own is used.
    LinetypeDiffers(String),
    /// The source used a linetype its own table lacks; it is drawn continuous.
    LinetypeMissing(u64),
    /// A linetype could not be added (its name was refused); it is drawn
    /// continuous.
    LinetypeRefused(String),
}

impl std::fmt::Display for TransferWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LinetypeDiffers(name) => write!(
                f,
                "Linetype {name} differs from this drawing's; this drawing's is used."
            ),
            Self::LinetypeMissing(id) => {
                write!(f, "Linetype {id} is not in the source; drawn continuous.")
            }
            Self::LinetypeRefused(name) => {
                write!(f, "Linetype {name} could not be added; drawn continuous.")
            }
        }
    }
}

/// What an absorb did, so it can be undone and done again.
#[derive(Clone, Debug)]
pub struct Absorbed {
    before: Tables,
    after: Tables,
    objects: Vec<Object>,
    /// What did not carry over exactly, once each.
    pub warnings: Vec<TransferWarning>,
}

impl Absorbed {
    /// The ids of the objects it added.
    pub fn ids(&self) -> Vec<EntityId> {
        self.objects.iter().map(|o| o.id).collect()
    }
}

impl Document {
    fn tables(&self) -> Tables {
        Tables {
            layers: self.layers.clone(),
            next_layer_id: self.next_layer_id,
            text_styles: self.text_styles.clone(),
            next_text_style_id: self.next_text_style_id,
            mleader_styles: self.mleader_styles.clone(),
            next_mleader_style_id: self.next_mleader_style_id,
            dim_styles: self.dim_styles.clone(),
            linetypes: self.linetypes.clone(),
            blocks: self.blocks.clone(),
            next_block_id: self.next_block_id,
        }
    }

    fn restore_tables(&mut self, tables: &Tables) {
        self.layers = tables.layers.clone();
        self.layer_positions = self
            .layers
            .iter()
            .enumerate()
            .map(|(i, l)| (l.id, i))
            .collect();
        self.next_layer_id = tables.next_layer_id;
        self.text_styles = tables.text_styles.clone();
        self.next_text_style_id = tables.next_text_style_id;
        self.mleader_styles = tables.mleader_styles.clone();
        self.next_mleader_style_id = tables.next_mleader_style_id;
        self.dim_styles = tables.dim_styles.clone();
        self.linetypes = tables.linetypes.clone();
        self.blocks = tables.blocks.clone();
        self.next_block_id = tables.next_block_id;
    }

    /// A drawing holding copies of the objects `ids` (in the model space
    /// they are in no layout) and this drawing's tables.
    pub fn extract(&self, ids: &[EntityId]) -> Document {
        let mut out = Document::default();
        out.restore_tables(&self.tables());
        out.current_layer = self.current_layer;
        out.units = self.units;
        let objects = ids
            .iter()
            .filter_map(|id| self.object(*id))
            .map(|o| Object::new(o.id, o.entity.clone(), o.attrs.clone(), None))
            .collect();
        out.push_many(objects);
        out
    }

    /// Bring the objects of `source` in, into `layout` (`None` for model
    /// space). Returns what is needed to undo it.
    pub fn absorb(
        &mut self,
        source: &Document,
        placement: &Placement,
        layout: Option<LayoutId>,
    ) -> Absorbed {
        let before = self.tables();
        let mut merge = Merge {
            dst: self,
            src: source,
            layers: HashMap::new(),
            text_styles: HashMap::new(),
            linetypes: HashMap::new(),
            warnings: Vec::new(),
            blocks: HashMap::new(),
        };
        let mut objects = Vec::new();
        match placement {
            Placement::Loose { dx, dy } => {
                for o in source.objects() {
                    let entity = merge.entity(&o.entity).translated(*dx, *dy);
                    let attrs = merge.attrs(&o.attrs);
                    objects.push((entity, attrs));
                }
            }
            Placement::Block { name, base, at } => {
                let entities: Vec<BlockEntity> = source
                    .objects()
                    .iter()
                    .map(|o| BlockEntity {
                        entity: merge.entity(&o.entity),
                        attrs: merge.attrs(&o.attrs),
                    })
                    .collect();
                let id = merge.dst.allocate_block_id();
                let def = Arc::new(BlockDef {
                    unit: Default::default(),
                    id,
                    name: name.clone(),
                    base: *base,
                    description: String::new(),
                    entities,
                });
                merge.dst.push_block(def.clone());
                let attrs = Attributes::on_layer(merge.dst.current_layer);
                objects.push((Entity::Insert(Box::new(Insert::new(def, *at))), attrs));
            }
        }
        let warnings = merge.warnings;
        let objects: Vec<Object> = objects
            .into_iter()
            .map(|(entity, attrs)| Object::new(self.allocate_id(), entity, attrs, layout))
            .collect();
        self.push_many(objects.clone());
        Absorbed {
            before,
            after: self.tables(),
            objects,
            warnings,
        }
    }

    /// Take back what `absorb` did.
    pub fn unabsorb(&mut self, absorbed: &Absorbed) {
        self.remove_many(&absorbed.ids());
        self.restore_tables(&absorbed.before);
    }

    /// Do again what `unabsorb` took back.
    pub fn reabsorb(&mut self, absorbed: &Absorbed) {
        self.restore_tables(&absorbed.after);
        self.push_many(absorbed.objects.clone());
    }

    /// A block name no block has, for pasting as a block (`A$C1`, `A$C2`...).
    pub fn unused_block_name(&self) -> String {
        (1..)
            .map(|n| format!("A$C{n}"))
            .find(|name| self.block_named(name).is_none())
            .unwrap_or_default()
    }
}

mod attributes;

/// The matching of one drawing's tables to another's while absorbing.
struct Merge<'a> {
    dst: &'a mut Document,
    src: &'a Document,
    layers: HashMap<LayerId, LayerId>,
    text_styles: HashMap<TextStyleId, TextStyleId>,
    /// The source's linetype ids to this drawing's: built as the objects are
    /// copied, and total (an id never maps to nothing), so a transfer cannot
    /// fail halfway. An id is only ever meaningful inside its own drawing.
    linetypes: HashMap<LinetypeId, LinetypeId>,
    warnings: Vec<TransferWarning>,
    blocks: HashMap<BlockId, Arc<BlockDef>>,
}

impl Merge<'_> {
    fn layer(&mut self, id: LayerId) -> LayerId {
        if let Some(done) = self.layers.get(&id) {
            return *done;
        }
        let found = match self.src.layer(id) {
            None => self.dst.current_layer,
            Some(layer) => match self.dst.layers.iter().find(|l| layer_match(l, layer)) {
                Some(same) => same.id,
                None => {
                    let mut copy = layer.clone();
                    copy.id = self.dst.allocate_layer_id();
                    copy.linetype = self.linetype(copy.linetype);
                    let id = copy.id;
                    self.dst.push_layer(copy);
                    id
                }
            },
        };
        self.layers.insert(id, found);
        found
    }

    /// The record of this drawing that stands for the source's linetype `id`.
    /// The three special records are the same in every drawing. Otherwise the
    /// drawing's record of that name is used (and a different pattern is
    /// reported: the drawing's own wins), or the source's record is added.
    fn linetype(&mut self, id: LinetypeId) -> LinetypeId {
        if let Some(done) = self.linetypes.get(&id) {
            return *done;
        }
        let found = if id.is_special() {
            id
        } else if let Some(record) = self.src.linetypes.get(id) {
            match self.dst.linetypes.named(&record.name) {
                Some(same) => {
                    if same.elements != record.elements {
                        self.warnings
                            .push(TransferWarning::LinetypeDiffers(record.name.clone()));
                    }
                    same.id
                }
                None => {
                    let mut copy = record.clone();
                    copy.id = LinetypeId(self.dst.linetypes.next_id());
                    let new = copy.id;
                    if self.dst.linetypes.push(copy).is_ok() {
                        new
                    } else {
                        self.warnings
                            .push(TransferWarning::LinetypeRefused(record.name.clone()));
                        LinetypeId::CONTINUOUS
                    }
                }
            }
        } else {
            self.warnings.push(TransferWarning::LinetypeMissing(id.0));
            LinetypeId::CONTINUOUS
        };
        self.linetypes.insert(id, found);
        found
    }

    fn text_style(&mut self, id: TextStyleId) -> TextStyleId {
        if let Some(done) = self.text_styles.get(&id) {
            return *done;
        }
        let found = match self.src.text_style(id) {
            None => TextStyleId::STANDARD,
            Some(style) => match self.dst.text_style_named(&style.name) {
                Some(same) => same.id,
                None => {
                    let mut copy = style.clone();
                    copy.id = self.dst.allocate_text_style_id();
                    let id = copy.id;
                    self.dst.push_text_style(copy);
                    id
                }
            },
        };
        self.text_styles.insert(id, found);
        found
    }

    fn attrs(&mut self, attrs: &Attributes) -> Attributes {
        let mut out = attrs.clone();
        out.layer = self.layer(attrs.layer);
        if let Attr::Set(id) = &attrs.linetype {
            out.linetype = Attr::Set(self.linetype(*id));
        }
        out
    }

    fn dim_style(&mut self, style: &DimStyle) -> DimStyle {
        let mut out = style.clone();
        out.text_style = self.text_style(style.text_style);
        if self.dst.dim_style_named(&out.name).is_none() {
            self.dst.dim_styles.push(out.clone());
        }
        out
    }

    fn mleader_style(&mut self, name: &str) {
        if self.dst.mleader_style_named(name).is_some() {
            return;
        }
        if let Some(style) = self.src.mleader_style_named(name) {
            let mut copy = style.clone();
            copy.content.text_style = self.text_style(copy.content.text_style);
            if let Attr::Set(id) = copy.leader_linetype {
                copy.leader_linetype = Attr::Set(self.linetype(id));
            }
            copy.id = crate::mleader::MLeaderStyleId(self.dst.next_mleader_style_id);
            self.dst.next_mleader_style_id = after(self.dst.next_mleader_style_id);
            self.dst.mleader_styles.push(copy);
        }
    }

    /// The block in the target that stands for `def`: the one of its name, or
    /// a new copy.
    fn block(&mut self, def: &Arc<BlockDef>) -> Arc<BlockDef> {
        if let Some(done) = self.blocks.get(&def.id) {
            return done.clone();
        }
        let found = match self.dst.block_named(&def.name) {
            Some(same) => same.clone(),
            None => {
                let entities = def
                    .entities
                    .iter()
                    .map(|e| BlockEntity {
                        entity: self.entity(&e.entity),
                        attrs: self.attrs(&e.attrs),
                    })
                    .collect();
                let copy = Arc::new(BlockDef {
                    unit: def.unit,
                    id: self.dst.allocate_block_id(),
                    name: def.name.clone(),
                    base: def.base,
                    description: def.description.clone(),
                    entities,
                });
                self.dst.push_block(copy.clone());
                copy
            }
        };
        self.blocks.insert(def.id, found.clone());
        found
    }
    fn entity(&mut self, entity: &Entity) -> Entity {
        match entity {
            Entity::MText(m) => {
                let mut m = m.clone();
                m.style = self.text_style(m.style);
                let fields = self.detached(&m.fields_as_text());
                m.set_fields_from(fields);
                Entity::MText(m)
            }
            Entity::Text(t) => {
                let mut t = t.clone();
                t.placement.style = self.text_style(t.placement.style);
                Entity::Text(t)
            }
            Entity::Insert(i) => {
                let mut i = i.clone();
                i.block = self.block(&i.block);
                self.insert_attributes(&mut i);
                Entity::Insert(i)
            }
            Entity::AttributeDefinition(d) => {
                let mut d = d.clone();
                self.attribute_definition(&mut d);
                Entity::AttributeDefinition(d)
            }
            Entity::Dimension(d) => {
                let mut d = d.clone();
                d.style = self.dim_style(&d.style);
                // Its objects have other ids here, or none.
                d.assoc.clear();
                Entity::Dimension(d)
            }
            Entity::MLeader(l) => {
                let mut l = l.clone();
                self.mleader_style(&l.style_name);
                if let MLeaderContent::MText(t) = &mut l.content {
                    t.text.style = self.text_style(t.text.style);
                }
                Entity::MLeader(l)
            }
            other => other.clone(),
        }
    }
}

fn layer_match(a: &Layer, b: &Layer) -> bool {
    a.name.eq_ignore_ascii_case(&b.name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linetype::StrokeElement;
    use crate::{Color, Point};

    fn circle() -> Entity {
        Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: 1.0,
        }
    }

    fn on_new_layer(doc: &mut Document, name: &str, entity: Entity) -> EntityId {
        let id = doc.allocate_layer_id();
        let mut layer = Layer::new(id, name);
        layer.color = Color::Foreground;
        doc.push_layer(layer);
        doc.add_with(entity, Attributes::on_layer(id))
    }

    fn block_of(doc: &mut Document, name: &str) -> Arc<BlockDef> {
        let id = doc.allocate_block_id();
        let def = Arc::new(BlockDef {
            unit: Default::default(),
            id,
            name: name.to_owned(),
            base: Point::new(0.0, 0.0),
            description: String::new(),
            entities: vec![BlockEntity {
                entity: circle(),
                attrs: Attributes::on_layer(LayerId::DEFAULT),
            }],
        });
        doc.push_block(def.clone());
        def
    }

    #[test]
    fn a_layer_the_target_lacks_is_added_and_one_it_has_is_matched_by_name() {
        let mut src = Document::default();
        let a = on_new_layer(&mut src, "Walls", circle());
        let b = on_new_layer(&mut src, "Doors", circle());
        let extract = src.extract(&[a, b]);
        let mut dst = Document::default();
        let existing = dst.allocate_layer_id();
        dst.push_layer(Layer::new(existing, "walls"));
        let absorbed = dst.absorb(&extract, &Placement::Loose { dx: 0.0, dy: 0.0 }, None);
        let names: Vec<_> = dst.layers().iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["0", "walls", "Doors"]);
        let layers: Vec<_> = absorbed
            .ids()
            .iter()
            .map(|id| dst.object(*id).unwrap().attrs.layer)
            .collect();
        assert_eq!(layers[0], existing);
        assert_ne!(layers[1], existing);
    }

    #[test]
    fn absorbing_moves_the_objects_and_undo_and_redo_are_exact() {
        let mut src = Document::default();
        let a = src.add(circle());
        let extract = src.extract(&[a]);
        let mut dst = Document::default();
        let before = dst.clone();
        let absorbed = dst.absorb(&extract, &Placement::Loose { dx: 5.0, dy: 0.0 }, None);
        let after = dst.clone();
        let id = absorbed.ids()[0];
        assert!(matches!(
            dst.get(id),
            Some(Entity::Circle { center, .. }) if center.x == 5.0
        ));
        dst.unabsorb(&absorbed);
        assert_eq!(dst.objects(), before.objects());
        dst.reabsorb(&absorbed);
        assert_eq!(dst, after);
    }

    #[test]
    fn a_block_is_copied_once_and_nested_blocks_come_with_it() {
        let mut src = Document::default();
        let inner = block_of(&mut src, "Inner");
        let outer_id = src.allocate_block_id();
        let outer = Arc::new(BlockDef {
            unit: Default::default(),
            id: outer_id,
            name: "Outer".into(),
            base: Point::new(0.0, 0.0),
            description: String::new(),
            entities: vec![BlockEntity {
                entity: Entity::Insert(Box::new(Insert::new(inner, Point::new(1.0, 1.0)))),
                attrs: Attributes::on_layer(LayerId::DEFAULT),
            }],
        });
        src.push_block(outer.clone());
        let one = src.add(Entity::Insert(Box::new(Insert::new(
            outer.clone(),
            Point::new(0.0, 0.0),
        ))));
        let two = src.add(Entity::Insert(Box::new(Insert::new(
            outer,
            Point::new(9.0, 0.0),
        ))));
        let extract = src.extract(&[one, two]);
        let mut dst = Document::default();
        dst.absorb(&extract, &Placement::Loose { dx: 0.0, dy: 0.0 }, None);
        let names: Vec<_> = dst.blocks().iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, ["Inner", "Outer"]);
        assert_eq!(dst.objects().len(), 2);
    }

    #[test]
    fn pasting_as_a_block_makes_one_block_and_one_reference() {
        let mut src = Document::default();
        let a = src.add(circle());
        let extract = src.extract(&[a]);
        let mut dst = Document::default();
        let name = dst.unused_block_name();
        let placement = Placement::Block {
            name: name.clone(),
            base: Point::new(-1.0, -1.0),
            at: Point::new(10.0, 10.0),
        };
        let absorbed = dst.absorb(&extract, &placement, None);
        assert_eq!(absorbed.ids().len(), 1);
        assert_eq!(dst.block_named(&name).unwrap().entities.len(), 1);
        assert!(matches!(
            dst.get(absorbed.ids()[0]),
            Some(Entity::Insert(i)) if i.insertion == Point::new(10.0, 10.0)
        ));
    }

    #[test]
    fn a_text_style_is_matched_by_name_or_copied() {
        let mut src = Document::default();
        let id = src.allocate_text_style_id();
        src.push_text_style(TextStyle::new(id, "Notes"));
        let text = crate::MText::new(Point::new(0.0, 0.0), 2.5, "hi".to_owned(), id);
        let a = src.add(Entity::MText(Box::new(text)));
        let extract = src.extract(&[a]);
        let mut dst = Document::default();
        let absorbed = dst.absorb(&extract, &Placement::Loose { dx: 0.0, dy: 0.0 }, None);
        let style = dst.text_style_named("Notes").unwrap().id;
        assert!(matches!(
            dst.get(absorbed.ids()[0]),
            Some(Entity::MText(m)) if m.style == style
        ));
    }

    // ---- linetypes cross drawings through a map of ids, not names

    fn with_linetype(doc: &mut Document, name: &str, pattern: &[StrokeElement]) -> LinetypeId {
        let mut table = doc.linetypes().clone();
        let id = table.add(name, "", pattern.to_vec()).unwrap();
        doc.set_linetypes(table);
        id
    }

    fn drawn_with(doc: &mut Document, linetype: LinetypeId) -> EntityId {
        let attrs = Attributes {
            linetype: Attr::Set(linetype),
            ..Attributes::on_layer(LayerId::DEFAULT)
        };
        doc.add_with(circle(), attrs)
    }

    fn paste(src: &Document, ids: &[EntityId], dst: &mut Document) -> Absorbed {
        let extract = src.extract(ids);
        dst.absorb(&extract, &Placement::Loose { dx: 0.0, dy: 0.0 }, None)
    }

    use StrokeElement::{Dash, Gap};

    #[test]
    fn a_linetype_the_target_lacks_is_copied_with_a_new_id_and_the_objects_follow() {
        let mut src = Document::default();
        let mine = with_linetype(&mut src, "Mine", &[Dash(2.0), Gap(1.0)]);
        let id = drawn_with(&mut src, mine);
        let mut dst = Document::default();
        // Make ids differ, so a raw copy of the id would point at the wrong thing.
        let other = with_linetype(&mut dst, "Other", &[Dash(9.0), Gap(9.0)]);
        assert_eq!(other, mine, "the same id means different records here");
        let absorbed = paste(&src, &[id], &mut dst);
        let copied = dst.linetypes().named("Mine").expect("copied");
        assert_ne!(copied.id, mine);
        assert_eq!(copied.elements, vec![Dash(2.0), Gap(1.0)]);
        let new = dst.object(absorbed.ids()[0]).unwrap();
        assert_eq!(new.attrs.linetype, Attr::Set(copied.id));
        assert!(absorbed.warnings.is_empty());
        assert_eq!(dst.validate(), Ok(()));
    }

    #[test]
    fn the_special_records_are_the_same_in_every_drawing() {
        let mut src = Document::default();
        let id = drawn_with(&mut src, LinetypeId::CONTINUOUS);
        let mut dst = Document::default();
        let absorbed = paste(&src, &[id], &mut dst);
        let new = dst.object(absorbed.ids()[0]).unwrap();
        assert_eq!(new.attrs.linetype, Attr::Set(LinetypeId::CONTINUOUS));
        assert_eq!(
            dst.linetypes().records().len(),
            src.linetypes().records().len()
        );
    }

    #[test]
    fn a_linetype_the_target_has_by_name_is_reused_and_a_different_pattern_is_reported_once() {
        let mut src = Document::default();
        let a = with_linetype(&mut src, "Same", &[Dash(1.0), Gap(1.0)]);
        let b = with_linetype(&mut src, "Differs", &[Dash(5.0), Gap(5.0)]);
        let ids = [
            drawn_with(&mut src, a),
            drawn_with(&mut src, b),
            drawn_with(&mut src, b),
            drawn_with(&mut src, b),
        ];
        let mut dst = Document::default();
        let same = with_linetype(&mut dst, "SAME", &[Dash(1.0), Gap(1.0)]);
        let differs = with_linetype(&mut dst, "differs", &[Dash(2.0), Gap(2.0)]);
        let before = dst.linetypes().records().len();
        let absorbed = paste(&src, &ids, &mut dst);
        assert_eq!(dst.linetypes().records().len(), before, "nothing added");
        let used: Vec<_> = absorbed
            .ids()
            .iter()
            .map(|id| dst.object(*id).unwrap().attrs.linetype.clone())
            .collect();
        assert_eq!(
            used,
            [same, differs, differs, differs].map(Attr::Set),
            "the target's own records"
        );
        assert_eq!(
            absorbed.warnings,
            [TransferWarning::LinetypeDiffers("Differs".to_owned())],
            "once for the name, not once per object"
        );
        let kept = &dst.linetypes().named("differs").unwrap().elements;
        assert_eq!(*kept, vec![Dash(2.0), Gap(2.0)]);
    }

    #[test]
    fn a_layers_linetype_is_carried_with_its_layer() {
        let mut src = Document::default();
        let mine = with_linetype(&mut src, "Mine", &[Dash(3.0), Gap(1.0)]);
        let layer = src.allocate_layer_id();
        let mut l = Layer::new(layer, "Pipes");
        l.linetype = mine;
        src.push_layer(l);
        let id = src.add_with(circle(), Attributes::on_layer(layer));
        let mut dst = Document::default();
        paste(&src, &[id], &mut dst);
        let pipes = dst.layers().iter().find(|l| l.name == "Pipes").unwrap();
        assert_eq!(dst.linetypes().get(pipes.linetype).unwrap().name, "Mine");
        assert_eq!(dst.validate(), Ok(()));
    }

    #[test]
    fn a_block_brings_its_contents_linetypes_once() {
        let mut src = Document::default();
        let mine = with_linetype(&mut src, "Mine", &[Dash(3.0), Gap(1.0)]);
        let id = src.allocate_block_id();
        let def = Arc::new(BlockDef {
            unit: Default::default(),
            id,
            name: "B".to_owned(),
            base: Point::new(0.0, 0.0),
            description: String::new(),
            entities: vec![BlockEntity {
                entity: circle(),
                attrs: Attributes {
                    linetype: Attr::Set(mine),
                    ..Attributes::on_layer(LayerId::DEFAULT)
                },
            }],
        });
        src.push_block(def.clone());
        let insert = src.add(Entity::Insert(Box::new(Insert::new(
            def,
            Point::new(0.0, 0.0),
        ))));
        let mut dst = Document::default();
        with_linetype(&mut dst, "Taken", &[Dash(1.0)]);
        paste(&src, &[insert], &mut dst);
        paste(&src, &[insert], &mut dst);
        let copied: Vec<_> = dst
            .linetypes()
            .records()
            .iter()
            .filter(|r| r.name == "Mine")
            .collect();
        assert_eq!(copied.len(), 1, "added once, reused the second time");
        let block = dst.block_named("B").unwrap();
        assert_eq!(block.entities[0].attrs.linetype, Attr::Set(copied[0].id));
        assert_eq!(dst.validate(), Ok(()));
    }

    #[test]
    fn a_source_reference_to_nothing_is_drawn_continuous_and_reported() {
        let mut src = Document::default();
        let id = drawn_with(&mut src, LinetypeId(77));
        let mut dst = Document::default();
        let absorbed = paste(&src, &[id], &mut dst);
        let new = dst.object(absorbed.ids()[0]).unwrap();
        assert_eq!(new.attrs.linetype, Attr::Set(LinetypeId::CONTINUOUS));
        assert_eq!(absorbed.warnings, [TransferWarning::LinetypeMissing(77)]);
        assert_eq!(dst.validate(), Ok(()));
    }

    #[test]
    fn undoing_and_redoing_a_paste_restores_the_linetype_table_and_references() {
        let mut src = Document::default();
        let mine = with_linetype(&mut src, "Mine", &[Dash(2.0), Gap(1.0)]);
        let id = drawn_with(&mut src, mine);
        let mut dst = Document::default();
        let table_before = dst.linetypes().clone();
        let absorbed = paste(&src, &[id], &mut dst);
        let table_after = dst.linetypes().clone();
        let refs_after: Vec<_> = dst.linetype_uses().map(|(id, _)| id).collect();
        dst.unabsorb(&absorbed);
        assert_eq!(*dst.linetypes(), table_before);
        dst.reabsorb(&absorbed);
        assert_eq!(*dst.linetypes(), table_after);
        let refs_again: Vec<_> = dst.linetype_uses().map(|(id, _)| id).collect();
        assert_eq!(refs_again, refs_after);
        assert_eq!(dst.validate(), Ok(()));
    }
}
