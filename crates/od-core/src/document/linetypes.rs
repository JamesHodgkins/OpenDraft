//! The linetype table as the drawing holds it.

use super::Document;
use crate::linetype::{LinetypeId, LinetypeTable};
use crate::validate::DocumentError;
use crate::{Layer, MLeaderStyle, Object};

/// Who holds a reference to a linetype record.
#[derive(Clone, Copy, Debug)]
pub enum LinetypeUse<'a> {
    Layer(&'a Layer),
    /// The linetype new objects are drawn with.
    NewObjects,
    Object(&'a Object),
    /// An object inside a block definition (nested blocks are definitions
    /// of their own, so they are visited too).
    BlockObject(&'a crate::BlockDef),
    MLeaderStyle(&'a MLeaderStyle),
}

impl Document {
    pub fn linetypes(&self) -> &LinetypeTable {
        &self.linetypes
    }

    /// The drawing's `LTSCALE`.
    pub fn linetype_scale(&self) -> f64 {
        self.linetype_scale
    }

    /// Set `LTSCALE` and return the old value. A scale that is not a finite
    /// number above zero is ignored.
    pub fn set_linetype_scale(&mut self, scale: f64) -> f64 {
        let old = self.linetype_scale;
        if scale.is_finite() && scale > 0.0 {
            self.linetype_scale = scale;
        }
        old
    }

    /// Every explicit reference to a linetype record, with who holds it. An
    /// object that follows its layer holds none (the layer does). This is the
    /// one list that `linetype_in_use`, PURGE and `validate` all go by.
    pub fn linetype_uses(&self) -> impl Iterator<Item = (LinetypeId, LinetypeUse<'_>)> {
        let set = |attr: &crate::Attr<LinetypeId>| match attr {
            crate::Attr::Set(id) => Some(*id),
            crate::Attr::ByLayer => None,
        };
        let layers = self
            .layers()
            .iter()
            .map(|l| (l.linetype, LinetypeUse::Layer(l)));
        let new_objects = set(self.new_object_style().1).map(|id| (id, LinetypeUse::NewObjects));
        let objects = self
            .objects()
            .iter()
            .filter_map(move |o| set(&o.attrs.linetype).map(|id| (id, LinetypeUse::Object(o))));
        let blocks = self.blocks().iter().flat_map(move |b| {
            b.entities.iter().filter_map(move |e| {
                set(&e.attrs.linetype).map(|id| (id, LinetypeUse::BlockObject(b)))
            })
        });
        let styles = self.mleader_styles().iter().filter_map(move |s| {
            set(&s.leader_linetype).map(|id| (id, LinetypeUse::MLeaderStyle(s)))
        });
        layers
            .chain(new_objects)
            .chain(objects)
            .chain(blocks)
            .chain(styles)
    }

    /// Whether anything uses the linetype `id`. A linetype in use is not for
    /// deleting.
    pub fn linetype_in_use(&self, id: LinetypeId) -> bool {
        self.linetype_uses().any(|(used, _)| used == id)
    }

    /// Every reference names a record of the table, and none names `BYLAYER`
    /// or `BYBLOCK` (an object says ByLayer with `Attr::ByLayer`; ByBlock is
    /// not built).
    pub(crate) fn validate_linetypes(&self) -> Result<(), DocumentError> {
        for (id, who) in self.linetype_uses() {
            let place = match who {
                LinetypeUse::Layer(l) => format!("layer \"{}\"", l.name),
                LinetypeUse::NewObjects => "the new-object style".to_owned(),
                LinetypeUse::Object(o) => format!("object {}", o.id.0),
                LinetypeUse::BlockObject(b) => format!("block \"{}\"", b.name),
                LinetypeUse::MLeaderStyle(s) => format!("multileader style \"{}\"", s.name),
            };
            if self.linetypes().get(id).is_none() {
                return Err(DocumentError::BadLinetype(format!(
                    "{place} uses linetype {}, which does not exist",
                    id.0
                )));
            }
            if id == LinetypeId::BYLAYER || id == LinetypeId::BYBLOCK {
                return Err(DocumentError::BadLinetype(format!(
                    "{place} uses linetype {}, which only says where a linetype comes from",
                    id.0
                )));
            }
        }
        Ok(())
    }

    /// Swap in a whole table, returning the old one (what undo and loading
    /// use). The caller has built it through [`LinetypeTable`], which keeps
    /// names and ids unique.
    pub fn set_linetypes(&mut self, table: LinetypeTable) -> LinetypeTable {
        std::mem::replace(&mut self.linetypes, table)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::BlockEntity;
    use crate::{Attr, Attributes, BlockDef, Entity, LayerId, Point};
    use std::sync::Arc;

    fn circle() -> Entity {
        Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: 1.0,
        }
    }

    #[test]
    fn the_named_constants_are_the_stock_records() {
        let doc = Document::default();
        for (id, name) in [
            (LinetypeId::CONTINUOUS, "CONTINUOUS"),
            (LinetypeId::DASHED, "DASHED"),
            (LinetypeId::DOT, "DOT"),
            (LinetypeId::DASHDOT, "DASHDOT"),
            (LinetypeId::CENTER, "CENTER"),
            (LinetypeId::HIDDEN, "HIDDEN"),
            (LinetypeId::PHANTOM, "PHANTOM"),
        ] {
            assert_eq!(doc.linetypes().get(id).map(|r| r.name.as_str()), Some(name));
        }
    }

    #[test]
    fn every_kind_of_owner_is_scanned_and_nothing_else() {
        let mut doc = Document::default();
        let layer = doc.allocate_layer_id();
        let mut l = crate::Layer::new(layer, "L");
        l.linetype = LinetypeId::HIDDEN;
        doc.push_layer(l);
        doc.set_new_object_style(Attr::ByLayer, Attr::Set(LinetypeId::DOT), Attr::ByLayer);
        doc.add_with(
            circle(),
            Attributes {
                linetype: Attr::Set(LinetypeId::DASHED),
                ..Attributes::on_layer(layer)
            },
        );
        doc.add(circle()); // takes the new-object style, DOT
        let id = doc.allocate_block_id();
        doc.push_block(Arc::new(BlockDef {
            id,
            name: "B".to_owned(),
            base: Point::new(0.0, 0.0),
            description: String::new(),
            entities: vec![BlockEntity {
                entity: circle(),
                attrs: Attributes {
                    linetype: Attr::Set(LinetypeId::CENTER),
                    ..Attributes::on_layer(LayerId::DEFAULT)
                },
            }],
        }));
        let mut table = doc.mleader_styles().to_vec();
        table[0].leader_linetype = Attr::Set(LinetypeId::PHANTOM);
        doc.set_mleader_styles(table, crate::MLeaderStyleId::STANDARD, 1);
        let mut used: Vec<u64> = doc.linetype_uses().map(|(id, _)| id.0).collect();
        used.sort_unstable();
        // Layer 0 (1), the first object (4), the new-object style and the object
        // made with it (5, 5), the block (7), layer L (8), the style (9).
        assert_eq!(used, [1, 4, 5, 5, 7, 8, 9]);
        assert!(doc.linetype_in_use(LinetypeId::PHANTOM));
        assert!(!doc.linetype_in_use(LinetypeId::DASHDOT));
        assert_eq!(doc.validate(), Ok(()));
    }

    #[test]
    fn a_reference_to_nothing_or_to_a_special_record_makes_the_drawing_invalid() {
        let bad = |edit: &dyn Fn(&mut Document)| {
            let mut doc = Document::default();
            edit(&mut doc);
            matches!(doc.validate(), Err(DocumentError::BadLinetype(_)))
        };
        let on_object = |id: u64| {
            move |doc: &mut Document| {
                doc.add_with(
                    circle(),
                    Attributes {
                        linetype: Attr::Set(LinetypeId(id)),
                        ..Attributes::on_layer(LayerId::DEFAULT)
                    },
                );
            }
        };
        assert!(bad(&on_object(99)), "no such record");
        assert!(
            bad(&on_object(2)),
            "BYLAYER is Attr::ByLayer, not a record to name"
        );
        assert!(bad(&on_object(3)), "ByBlock is not built");
        assert!(!bad(&on_object(1)));
        assert!(bad(&|doc: &mut Document| {
            doc.set_new_object_style(Attr::ByLayer, Attr::Set(LinetypeId(99)), Attr::ByLayer);
        }));
        assert!(bad(&|doc: &mut Document| {
            let mut layer = doc.layer(LayerId::DEFAULT).unwrap().clone();
            layer.linetype = LinetypeId::BYLAYER;
            doc.replace_layer(layer);
        }));
    }

    #[test]
    fn renaming_a_linetype_changes_one_record_and_every_user_follows() {
        let mut doc = Document::default();
        let id = doc.add_with(
            circle(),
            Attributes {
                linetype: Attr::Set(LinetypeId::DASHED),
                ..Attributes::on_layer(LayerId::DEFAULT)
            },
        );
        let mut table = doc.linetypes().clone();
        let mut record = table.get(LinetypeId::DASHED).unwrap().clone();
        record.name = "Dashes".to_owned();
        table.replace(record).unwrap();
        doc.set_linetypes(table);
        let object = doc.object(id).unwrap();
        let name = &doc.linetypes().get(doc.linetype_of(object)).unwrap().name;
        assert_eq!(name, "Dashes");
        assert!(doc.dash_pattern(object).is_some());
        assert_eq!(doc.validate(), Ok(()));
    }
}
