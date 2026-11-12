//! A drawing as the importer wants it: layers, the flat shapes of model
//! space in drawing order, and the named blocks with their shapes, with a
//! report of what was left out. It is built
//! in two passes (decode every object, then resolve references), because
//! objects refer to each other by handle in any order.

use std::sync::atomic::AtomicBool;

use crate::attrib;
use crate::database::{Database, Visit};
use crate::entities::{self, InsertData, Vertex2dData};
use crate::error::{DwgError, DwgErrorKind, DwgStage};
use crate::handles::DwgHandle;
use crate::header::DwgHeader;
use crate::limits::DwgLimits;
use crate::lineweight::{lineweight, DwgLineweight, BY_LAYER};
use crate::pass::{Look, Pass, Record, Stored};
use crate::report::{Degradation, ImportReport, Skipped};
use crate::shapes::{self, DwgAttribute, Shape, MAX_ABS_COORDINATE};
use crate::spline;
use crate::tables::{self, BlockData, DwgLinetypeDef};
use crate::text::{self, DwgTextStyle};
use crate::types::{FixedType, ObjectKind};
use crate::version::DwgVersion;

/// A colour, once "by block" has been resolved to "by layer".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DwgColor {
    ByLayer,
    ByBlock,
    /// An AutoCAD Color Index, 1 to 255.
    Aci(u8),
    True(u8, u8, u8),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DwgLinetype {
    ByLayer,
    ByBlock,
    Named(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct DwgLayer {
    pub handle: DwgHandle,
    pub name: String,
    pub frozen: bool,
    pub locked: bool,
    pub on: bool,
    pub color: DwgColor,
    pub linetype: Option<String>,
    pub lineweight: DwgLineweight,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DwgEntity {
    pub handle: DwgHandle,
    /// The handle of one of the drawing's layers.
    pub layer: DwgHandle,
    pub color: DwgColor,
    pub linetype: DwgLinetype,
    /// The entity's own linetype scale (1 when it has none or it is unusable).
    pub linetype_scale: f64,
    pub lineweight: DwgLineweight,
    pub shape: Shape,
}

/// A named block definition. Anonymous blocks (the pictures of dimensions,
/// hatches and dynamic blocks) and external references are not read.
#[derive(Clone, Debug, PartialEq)]
pub struct DwgBlock {
    pub handle: DwgHandle,
    pub name: String,
    /// The point of the block's coordinates an insert puts at its insertion point.
    pub base: [f64; 2],
    pub description: String,
    /// In the block's own coordinates, in drawing order.
    pub entities: Vec<DwgEntity>,
}

#[derive(Debug)]
pub struct Drawing {
    pub version: DwgVersion,
    pub header: DwgHeader,
    /// In the order of the layer table, which starts with layer 0.
    pub layers: Vec<DwgLayer>,
    /// The linetype table, including the three special records.
    pub linetypes: Vec<DwgLinetypeDef>,
    /// The entities of model space, in drawing order.
    pub entities: Vec<DwgEntity>,
    /// The named blocks, in the order they were made.
    pub blocks: Vec<DwgBlock>,
    /// The text styles, in handle order (the order they were made).
    pub text_styles: Vec<DwgTextStyle>,
    pub report: ImportReport,
}

fn decode(v: &mut Visit<'_, '_>, limits: &DwgLimits, pass: &mut Pass) -> Result<(), DwgError> {
    let (Some(common), Some(refs)) = (&v.common, &v.refs) else {
        return Ok(());
    };
    let ObjectKind::Fixed(kind) = v.kind else {
        if v.is_entity {
            pass.others.insert(v.handle, v.kind.name());
        }
        return Ok(());
    };
    let s = &mut *v.streams;
    let stored = match kind {
        FixedType::Layer => {
            let layer = tables::read_layer(s, v.handle, limits)?;
            pass.layers.insert(v.handle, layer);
            return Ok(());
        }
        FixedType::Linetype => {
            pass.linetypes
                .insert(v.handle, tables::read_linetype(s, limits)?);
            return Ok(());
        }
        FixedType::BlockHeader => {
            pass.blocks
                .insert(v.handle, tables::read_block_header(s, v.handle, limits)?);
            return Ok(());
        }
        FixedType::Style => {
            // A style that does not read leaves its text on the standard one.
            if let Ok(style) = text::read_style(s, v.handle, limits) {
                pass.styles.insert(v.handle, style);
            }
            return Ok(());
        }
        // Text that does not read is left out and counted, not a damaged file.
        FixedType::Text => match text::read_text(s, v.handle, limits) {
            Ok(t) => Stored::Text(t),
            Err(_) => return unread(pass, v.handle, "TEXT"),
        },
        FixedType::Attrib | FixedType::Attdef => {
            match attrib::read_attrib(s, v.handle, v.version, limits, kind == FixedType::Attdef) {
                Ok(a) if kind == FixedType::Attrib => Stored::Attrib(a),
                Ok(a) => Stored::Attdef(a),
                Err(_) => return unread(pass, v.handle, kind.name()),
            }
        }
        FixedType::Mtext => match text::read_mtext(s, v.handle, limits) {
            Ok(t) => Stored::MText(t),
            Err(_) => return unread(pass, v.handle, "MTEXT"),
        },
        FixedType::Point => Stored::Point(entities::read_point(s)?),
        FixedType::Line => Stored::Line(entities::read_line(s)?),
        FixedType::Circle => Stored::Circle(entities::read_circle(s)?),
        FixedType::Arc => Stored::Arc(entities::read_arc(s)?),
        FixedType::Ellipse => Stored::Ellipse(entities::read_ellipse(s)?),
        FixedType::Spline => Stored::Spline(spline::read_spline(s, v.version, limits)?),
        FixedType::Lwpolyline => Stored::LwPolyline(entities::read_lwpolyline(s, limits)?),
        FixedType::Polyline2d => Stored::Polyline(entities::read_polyline2d(s, limits)?),
        FixedType::Vertex2d => Stored::Vertex(entities::read_vertex2d(s)?),
        FixedType::Insert => Stored::Insert(entities::read_insert(s, v.handle, limits)?),
        FixedType::Block | FixedType::Endblk | FixedType::Seqend => return Ok(()),
        other => {
            if other.is_entity() {
                pass.others.insert(v.handle, other.name().to_owned());
            }
            return Ok(());
        }
    };
    let entity = common.entity;
    let look = Look {
        layer: refs.layer,
        color_index: entity.map_or(256, |e| e.color.index),
        true_color: entity.and_then(|e| e.color.rgb),
        linetype_flags: entity.map_or(0, |e| e.linetype_flags),
        linetype: refs.linetype,
        linetype_scale: entity.map_or(1.0, |e| e.linetype_scale),
        lineweight: entity.map_or(BY_LAYER, |e| e.lineweight),
        invisible: entity.is_some_and(|e| e.invisible & 1 != 0),
    };
    pass.records.insert(v.handle, Record { look, stored });
    Ok(())
}

/// Notes a text entity whose fields did not read.
fn unread(pass: &mut Pass, handle: DwgHandle, name: &str) -> Result<(), DwgError> {
    pass.others.insert(handle, format!("{name} (unreadable)"));
    Ok(())
}

pub(crate) fn table_color(c: &crate::common::TableColor) -> DwgColor {
    if let Some(rgb) = c.true_color() {
        return DwgColor::True((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
    }
    DwgColor::Aci(c.aci().unwrap_or(7))
}

fn invalid(handle: DwgHandle) -> DwgError {
    DwgError::new(DwgErrorKind::InvalidObject, DwgStage::Object).of_object(handle)
}

/// The look of one entity with every handle resolved.
fn attributes(
    handle: DwgHandle,
    look: &Look,
    pass: &Pass,
    report: &mut ImportReport,
) -> Result<(DwgHandle, DwgColor, DwgLinetype, DwgLineweight), DwgError> {
    let layer = look
        .layer
        .filter(|l| pass.layers.contains_key(l))
        .ok_or_else(|| invalid(handle))?;
    let color = match (look.true_color, look.color_index) {
        (Some(rgb), _) => DwgColor::True((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8),
        (None, 256) => DwgColor::ByLayer,
        (None, 0 | 257) => DwgColor::ByBlock,
        (None, i) if i <= 255 => DwgColor::Aci(i as u8),
        (None, _) => DwgColor::ByLayer,
    };
    let linetype = match look.linetype_flags {
        1 => DwgLinetype::ByBlock,
        2 => DwgLinetype::Named("Continuous".to_owned()),
        3 => match look.linetype.and_then(|h| pass.linetypes.get(&h)) {
            Some(l) if l.name.eq_ignore_ascii_case("bylayer") => DwgLinetype::ByLayer,
            Some(l) if l.name.eq_ignore_ascii_case("byblock") => DwgLinetype::ByBlock,
            Some(l) => DwgLinetype::Named(l.name.clone()),
            None => {
                report.degrade(Degradation::MissingLinetype);
                DwgLinetype::ByLayer
            }
        },
        _ => DwgLinetype::ByLayer,
    };
    let weight = lineweight(look.lineweight);
    Ok((layer, color, linetype, weight))
}

fn shape_of(record: &Record, pass: &Pass, report: &mut ImportReport) -> Result<Shape, Skipped> {
    match &record.stored {
        Stored::Point(d) => shapes::point(d, report),
        Stored::Line(d) => shapes::line(d, report),
        Stored::Circle(d) => shapes::circle(d, report),
        Stored::Arc(d) => shapes::arc(d, report),
        Stored::Ellipse(d) => shapes::ellipse(d, report),
        Stored::Spline(d) => spline::spline(d, report),
        Stored::LwPolyline(d) => shapes::lwpolyline(d, report),
        Stored::Polyline(header) => {
            let vertices: Vec<&Vertex2dData> = header
                .vertices
                .iter()
                .filter_map(|h| match pass.records.get(h).map(|r| &r.stored) {
                    Some(Stored::Vertex(v)) => Some(v),
                    _ => None,
                })
                .collect();
            shapes::polyline2d(header, &vertices, report)
        }
        Stored::Vertex(_) => Err(Skipped::InvalidGeometry),
        Stored::Text(d) => text::text(d, report),
        // An attribute belongs to the reference that owns it (see `insert`).
        Stored::Attrib(_) => Err(Skipped::Unsupported("ATTRIB".into())),
        Stored::Attdef(d) => attrib::definition(d, report),
        Stored::MText(d) => text::mtext(d, report),
        Stored::Insert(d) => match d.block.and_then(|h| Some((h, pass.blocks.get(&h)?))) {
            Some((h, b)) if is_named(b) => {
                let mut shape = shapes::insert(d, h, report)?;
                if let Shape::Insert { attributes, .. } = &mut shape {
                    *attributes = owned_attributes(d, pass, report);
                }
                Ok(shape)
            }
            Some((_, b)) if b.is_xref => Err(Skipped::ExternalReference),
            Some((_, b)) if b.anonymous => Err(Skipped::AnonymousBlockReference),
            _ => Err(Skipped::InvalidGeometry),
        },
    }
}

/// Whether a block record is a named block the importer reads: not a space,
/// not anonymous, not an external reference.
fn is_named(block: &BlockData) -> bool {
    !block.is_xref && !block.anonymous && !block.name.starts_with('*')
}

/// The attributes a reference owns, in order. One that cannot be read is
/// counted in the report and left out.
fn owned_attributes(d: &InsertData, pass: &Pass, report: &mut ImportReport) -> Vec<DwgAttribute> {
    let mut out = Vec::new();
    for handle in &d.attribute_handles {
        let Some(record) = pass.records.get(handle) else {
            continue;
        };
        let Stored::Attrib(data) = &record.stored else {
            continue;
        };
        match attrib::attribute(data, record.look.invisible, report) {
            Ok(attribute) => out.push(attribute),
            Err(why) => report.skip(why),
        }
    }
    out
}

/// The supported entities among `handles` (a block's, in its order), with
/// everything left out counted in the report.
fn entities_of(
    handles: &[DwgHandle],
    pass: &Pass,
    report: &mut ImportReport,
) -> Result<Vec<DwgEntity>, DwgError> {
    let mut entities = Vec::new();
    for handle in handles.iter().copied() {
        let Some(record) = pass.records.get(&handle) else {
            if let Some(name) = pass.others.get(&handle) {
                report.skip(Skipped::Unsupported(name.clone()));
            }
            continue;
        };
        if record.look.invisible {
            report.skip(Skipped::Invisible);
            continue;
        }
        let (before_d, before_n) = (
            report.degraded.values().sum::<u64>(),
            report.normalized.len(),
        );
        let (layer, color, linetype, lineweight) = attributes(handle, &record.look, pass, report)?;
        match shape_of(record, pass, report) {
            Ok(shape) => {
                let untouched = report.degraded.values().sum::<u64>() == before_d
                    && report.normalized.len() == before_n;
                report.exact_entities += u64::from(untouched);
                entities.push(DwgEntity {
                    handle,
                    layer,
                    color,
                    linetype,
                    linetype_scale: record.look.linetype_scale,
                    lineweight,
                    shape,
                });
            }
            Err(why) => report.skip(why),
        }
    }
    Ok(entities)
}

impl Drawing {
    /// Reads the drawing in `bytes`: the layers and the supported entities of
    /// model space. Everything else is counted in the report.
    pub fn read(
        bytes: &[u8],
        limits: &DwgLimits,
        cancelled: &AtomicBool,
    ) -> Result<Drawing, DwgError> {
        let db = Database::load(bytes, limits, cancelled)?;
        let mut pass = Pass::default();
        db.visit(limits, cancelled, |v| decode(v, limits, &mut pass))?;

        let mut report = ImportReport::new(db.version);
        let model = db
            .header
            .model_space
            .and_then(|h| pass.blocks.get(&h))
            .ok_or_else(|| DwgError::new(DwgErrorKind::InvalidObject, DwgStage::Header))?;

        let entities = entities_of(&model.entities, &pass, &mut report)?;
        let mut blocks = Vec::new();
        for (&block_handle, block) in &pass.blocks {
            if Some(block_handle) == db.header.model_space {
                continue;
            }
            if block.name.to_ascii_lowercase().starts_with("*paper_space") {
                for handle in &block.entities {
                    if pass.records.contains_key(handle) || pass.others.contains_key(handle) {
                        report.skip(Skipped::PaperSpace);
                    }
                }
                continue;
            }
            if !is_named(block) {
                continue;
            }
            let base = [block.base[0], block.base[1]];
            if !base
                .iter()
                .all(|v| v.is_finite() && v.abs() <= MAX_ABS_COORDINATE)
            {
                report.skip(Skipped::InvalidGeometry);
                continue;
            }
            blocks.push(DwgBlock {
                handle: block_handle,
                name: block.name.clone(),
                base,
                description: block.description.clone(),
                entities: entities_of(&block.entities, &pass, &mut report)?,
            });
        }

        let layers = pass
            .layers
            .iter()
            .map(|(&handle, l)| DwgLayer {
                handle,
                name: l.name.clone(),
                frozen: l.frozen,
                locked: l.locked,
                on: l.on,
                color: table_color(&l.color),
                linetype: l
                    .linetype
                    .and_then(|h| pass.linetypes.get(&h))
                    .map(|l| l.name.clone()),
                lineweight: match lineweight(l.lineweight) {
                    DwgLineweight::ByLayer => DwgLineweight::Default,
                    other => other,
                },
            })
            .collect();
        let linetypes = pass.linetypes.values().cloned().collect();
        let text_styles = pass.styles.values().cloned().collect();
        Ok(Drawing {
            version: db.version,
            header: db.header,
            layers,
            linetypes,
            entities,
            blocks,
            text_styles,
            report,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::TableColor;
    use crate::tables::LayerData;

    const LAYER: DwgHandle = DwgHandle(0x10);

    fn named(name: &str) -> DwgLinetypeDef {
        DwgLinetypeDef {
            name: name.to_owned(),
            description: String::new(),
            pattern: None,
            complex: false,
        }
    }

    fn pass() -> Pass {
        let mut pass = Pass::default();
        let color = TableColor {
            index: 0,
            packed: 0xC300_0007,
            name: None,
            book_name: None,
        };
        let layer = LayerData {
            name: "0".into(),
            frozen: false,
            locked: false,
            on: true,
            lineweight: 31,
            color,
            linetype: None,
        };
        pass.layers.insert(LAYER, layer);
        pass.linetypes.insert(DwgHandle(0x20), named("DASHED"));
        pass.linetypes.insert(DwgHandle(0x21), named("ByBlock"));
        pass
    }

    fn look() -> Look {
        Look {
            layer: Some(LAYER),
            color_index: 256,
            true_color: None,
            linetype_flags: 0,
            linetype: None,
            linetype_scale: 1.0,
            lineweight: BY_LAYER,
            invisible: false,
        }
    }

    fn resolve(look: &Look) -> (DwgColor, DwgLinetype, DwgLineweight, ImportReport) {
        let mut report = ImportReport::new(DwgVersion::R2013);
        let (layer, c, t, w) = attributes(DwgHandle(1), look, &pass(), &mut report).unwrap();
        assert_eq!(layer, LAYER);
        (c, t, w, report)
    }

    #[test]
    fn everything_by_layer_is_exact() {
        let (c, t, w, report) = resolve(&look());
        assert_eq!(
            (c, t, w),
            (
                DwgColor::ByLayer,
                DwgLinetype::ByLayer,
                DwgLineweight::ByLayer
            )
        );
        assert!(report.degraded.is_empty());
    }

    #[test]
    fn explicit_attributes_are_kept() {
        let l = Look {
            color_index: 3,
            linetype_flags: 3,
            linetype: Some(DwgHandle(0x20)),
            lineweight: 8,
            ..look()
        };
        let (c, t, w, report) = resolve(&l);
        assert_eq!(c, DwgColor::Aci(3));
        assert_eq!(t, DwgLinetype::Named("DASHED".into()));
        assert_eq!(w, DwgLineweight::Hundredths(30));
        assert!(report.degraded.is_empty());
        let l = Look {
            true_color: Some(0x10_20_30),
            linetype_flags: 2,
            ..look()
        };
        let (c, t, _, _) = resolve(&l);
        assert_eq!(
            (c, t),
            (
                DwgColor::True(0x10, 0x20, 0x30),
                DwgLinetype::Named("Continuous".into())
            )
        );
    }

    #[test]
    fn by_block_is_kept() {
        let l = Look {
            color_index: 0,
            linetype_flags: 1,
            lineweight: crate::lineweight::BY_BLOCK,
            ..look()
        };
        let (c, t, w, report) = resolve(&l);
        assert_eq!(
            (c, t, w),
            (
                DwgColor::ByBlock,
                DwgLinetype::ByBlock,
                DwgLineweight::ByBlock
            )
        );
        assert!(report.degraded.is_empty());
        let l = Look {
            linetype_flags: 3,
            linetype: Some(DwgHandle(0x21)),
            ..look()
        };
        assert_eq!(resolve(&l).1, DwgLinetype::ByBlock);
    }

    #[test]
    fn a_missing_linetype_falls_back_with_a_note() {
        let l = Look {
            linetype_flags: 3,
            linetype: Some(DwgHandle(0x99)),
            ..look()
        };
        let (_, t, _, report) = resolve(&l);
        assert_eq!(t, DwgLinetype::ByLayer);
        assert_eq!(report.degraded.get(&Degradation::MissingLinetype), Some(&1));
    }

    #[test]
    fn an_entity_on_a_layer_that_does_not_exist_is_corruption() {
        let l = Look {
            layer: Some(DwgHandle(0x77)),
            ..look()
        };
        let mut report = ImportReport::new(DwgVersion::R2013);
        let err = attributes(DwgHandle(5), &l, &pass(), &mut report).unwrap_err();
        assert_eq!(err.kind, DwgErrorKind::InvalidObject);
        assert_eq!(err.object_handle, Some(DwgHandle(5)));
        let none = Look {
            layer: None,
            ..look()
        };
        assert!(attributes(DwgHandle(5), &none, &pass(), &mut report).is_err());
    }

    #[test]
    fn lineweight_codes() {
        assert_eq!(lineweight(0), DwgLineweight::Hundredths(0));
        assert_eq!(lineweight(8), DwgLineweight::Hundredths(30));
        assert_eq!(lineweight(23), DwgLineweight::Hundredths(211));
        assert_eq!(lineweight(29), DwgLineweight::ByLayer);
        assert_eq!(lineweight(31), DwgLineweight::Default);
        assert_eq!(lineweight(24), DwgLineweight::Default);
    }
}
