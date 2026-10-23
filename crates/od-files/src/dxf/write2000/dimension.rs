//! DIMENSION entities and the pieces around them: each dimension names an
//! anonymous block (`*D1`, `*D2`...) that holds its picture, and a DIMSTYLE
//! from the table, with the variables it overrides as extended data.
//!
//! Only the kinds that map one to one onto a DIMENSION are written (linear,
//! aligned, radius, diameter, angular and ordinate); the others are written
//! as their pictures (see `dimensions::is_real`). **Nothing written here has
//! been opened in AutoCAD**; ezdxf reads it and its audit passes.

use std::f64::consts::TAU;

use od_core::{DimKind, DimText, Dimension, Document, Entity, Object, Point};

use super::{hex, Ids};
use crate::dxf::dimstyle_codec::{codes, differences, text_of, xdata_code};
use crate::dxf::write::{finite, Out};
use crate::FileError;

/// The text of the style's DIMSTYLE record: AutoCAD's defaults with the
/// variables `style` keeps written over them.
pub(super) fn style_pairs(
    style: &od_core::DimStyle,
    text_style: Option<u32>,
) -> Vec<(i32, String)> {
    let mut pairs: Vec<(i32, String)> = super::tables::DIMSTYLE
        .iter()
        .map(|(code, value)| (*code, (*value).to_owned()))
        .collect();
    for (code, value) in codes(style) {
        let text = text_of(&value);
        match pairs.iter_mut().find(|(c, _)| *c == code) {
            Some(slot) => slot.1 = text,
            None => pairs.push((code, text)),
        }
    }
    if let Some(handle) = text_style {
        pairs.push((340, hex(handle)));
    }
    pairs
}

/// The block a real dimension names: its picture, owned by `record`.
pub(super) fn block_pieces(object: &Object) -> Vec<Object> {
    object
        .dimension_picture()
        .unwrap_or_default()
        .iter()
        .map(|piece| {
            Object::new(
                od_core::EntityId(0),
                piece.clone(),
                object.attrs.clone(),
                None,
            )
        })
        .collect()
}

fn point(out: &mut Out, code: i32, p: Point) -> Result<(), FileError> {
    out.point3(code, finite(p.x)?, finite(p.y)?, 0.0);
    Ok(())
}

/// The groups of a DIMENSION after the entity header, the way AutoCAD
/// orders them: the common part, then the kind's.
pub(super) fn entity(
    out: &mut Out,
    ids: &Ids,
    document: &Document,
    object: &Object,
    d: &Dimension,
) -> Result<(), FileError> {
    let Some((block, _)) = ids.dim_blocks.get(&object.id) else {
        return Err(FileError::Invalid("a dimension has no block".into()));
    };
    let Some(centre) = d.text_centre() else {
        return Err(FileError::Invalid("a dimension draws nothing".into()));
    };
    let (kind_flag, definition) = match &d.kind {
        DimKind::Linear {
            line_point,
            rotation,
            ..
        } => (i32::from(rotation.is_none()), *line_point),
        DimKind::Radial {
            center,
            diameter: false,
            ..
        } => (4, *center),
        DimKind::Radial {
            center,
            chord_point,
            diameter: true,
            ..
        } => (
            3,
            Point::new(
                2.0 * center.x - chord_point.x,
                2.0 * center.y - chord_point.y,
            ),
        ),
        DimKind::Angular {
            arc_point,
            p2,
            lines,
            ..
        } => (
            if *lines { 2 } else { 5 },
            if *lines { *p2 } else { *arc_point },
        ),
        DimKind::Ordinate { x_datum, .. } => {
            (6 + if *x_datum { 64 } else { 0 }, Point::new(0.0, 0.0))
        }
        DimKind::ArcLength { .. } => return Err(super::super::write::unexploded()),
    };
    let flags = kind_flag | if d.text_pos.is_some() { 128 } else { 0 };
    out.pair(100, "AcDbDimension");
    out.pair(2, block);
    point(out, 10, definition)?;
    point(out, 11, centre)?;
    out.pair(70, flags);
    out.pair(71, 5);
    match &d.text {
        DimText::Measured => {}
        DimText::Hidden => out.pair(1, " "),
        DimText::Custom(text) => out.pair(1, text),
    }
    out.pair(3, &d.style.name);
    if let Some(angle) = d.text_angle {
        out.pair(53, finite(angle.rem_euclid(TAU).to_degrees())?);
    }
    match &d.kind {
        DimKind::Linear {
            p1,
            p2,
            rotation,
            oblique,
            ..
        } => {
            out.pair(100, "AcDbAlignedDimension");
            point(out, 13, *p1)?;
            point(out, 14, *p2)?;
            // An aligned dimension's line runs the way its points do.
            let line = rotation.unwrap_or_else(|| (p2.y - p1.y).atan2(p2.x - p1.x));
            out.pair(50, finite(line.rem_euclid(TAU).to_degrees())?);
            if let Some(oblique) = oblique {
                // Measured from the dimension line.
                out.pair(52, finite((oblique - line).rem_euclid(TAU).to_degrees())?);
            }
            if rotation.is_some() {
                out.pair(100, "AcDbRotatedDimension");
            }
        }
        DimKind::Radial {
            chord_point,
            leader_length,
            diameter,
            ..
        } => {
            out.pair(
                100,
                if *diameter {
                    "AcDbDiametricDimension"
                } else {
                    "AcDbRadialDimension"
                },
            );
            point(out, 15, *chord_point)?;
            out.pair(40, finite(*leader_length)?);
        }
        DimKind::Angular {
            vertex,
            p1,
            p2,
            arc_point,
            lines,
        } => {
            if *lines {
                out.pair(100, "AcDb2LineAngularDimension");
                point(out, 13, *vertex)?;
                point(out, 14, *p1)?;
                point(out, 15, *vertex)?;
                point(out, 16, *arc_point)?;
            } else {
                out.pair(100, "AcDb3PointAngularDimension");
                point(out, 13, *p1)?;
                point(out, 14, *p2)?;
                point(out, 15, *vertex)?;
            }
        }
        DimKind::Ordinate {
            feature,
            leader_end,
            ..
        } => {
            out.pair(100, "AcDbOrdinateDimension");
            point(out, 13, *feature)?;
            point(out, 14, *leader_end)?;
        }
        DimKind::ArcLength { .. } => return Err(super::super::write::unexploded()),
    }
    overrides(out, document, d);
    Ok(())
}

/// The variables the dimension has changed from the style it names, as
/// `ACAD` / `DSTYLE` extended data.
fn overrides(out: &mut Out, document: &Document, d: &Dimension) {
    let base = document
        .dim_style_named(&d.style.name)
        .cloned()
        .unwrap_or_default();
    let changed = differences(&d.style, &base);
    if changed.is_empty() {
        return;
    }
    out.pair(1001, "ACAD");
    out.pair(1000, "DSTYLE");
    out.pair(1002, "{");
    for (code, value) in changed {
        out.pair(1070, code);
        out.pair(xdata_code(&value), text_of(&value));
    }
    out.pair(1002, "}");
}

/// Whether an object is a dimension this module writes.
pub(super) fn is_written(entity: &Entity) -> bool {
    matches!(entity, Entity::Dimension(d) if crate::dxf::dimensions::is_real(d))
}

/// The `*D` names of the drawing's real dimensions, in object order.
pub(super) fn block_names(document: &Document) -> Vec<(od_core::EntityId, String)> {
    document
        .objects()
        .iter()
        .filter(|o| is_written(&o.entity))
        .enumerate()
        .map(|(i, o)| (o.id, format!("*D{}", i + 1)))
        .collect()
}

#[cfg(test)]
mod tests {
    use od_core::{AngleFormat, DimStyle, Layer};

    use super::*;
    use crate::dxf::read;
    use crate::dxf::write2000;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn all_kinds() -> Vec<Dimension> {
        let style = DimStyle::default();
        let mut linear = Dimension::linear(p(0.0, 0.0), p(30.0, 10.0), p(15.0, 20.0), 0.5, &style);
        linear.text = DimText::Custom("<> TYP".to_owned());
        let mut aligned = Dimension::aligned(p(0.0, 0.0), p(30.0, 10.0), p(5.0, -8.0), &style);
        aligned.text_pos = Some(p(40.0, -12.0));
        aligned.text_angle = Some(0.25);
        let mut slanted = Dimension::linear(p(40.0, 0.0), p(70.0, 0.0), p(55.0, 10.0), 0.0, &style);
        if let DimKind::Linear { oblique, .. } = &mut slanted.kind {
            *oblique = Some(1.2);
        }
        let mut hidden = Dimension::radius(p(100.0, 0.0), p(110.0, 0.0), &style);
        hidden.text = DimText::Hidden;
        vec![
            linear,
            aligned,
            slanted,
            hidden,
            Dimension::diameter(p(100.0, 30.0), p(100.0, 40.0), &style),
            Dimension::angular(
                p(0.0, 50.0),
                p(20.0, 50.0),
                p(0.0, 70.0),
                p(14.0, 64.0),
                false,
                &style,
            ),
            Dimension::angular(
                p(50.0, 50.0),
                p(70.0, 50.0),
                p(70.0, 70.0),
                p(65.0, 53.0),
                true,
                &style,
            ),
            Dimension::ordinate(p(30.0, 80.0), p(30.0, 95.0), true, &style),
            Dimension::ordinate(p(30.0, 80.0), p(50.0, 80.0), false, &style),
        ]
    }

    fn read_back(document: &Document) -> Document {
        let bytes = write2000::write(document).unwrap().bytes;
        read::read(&bytes).unwrap().document
    }

    fn near(a: Point, b: Point) -> bool {
        (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
    }

    #[test]
    fn every_kind_comes_back_with_its_points_text_and_style() {
        let mut document = Document::default();
        let styled = DimStyle {
            text_height: 4.0,
            precision: 3,
            angle_format: AngleFormat::DecimalDegrees,
            angle_precision: 1,
            ..DimStyle::default()
        };
        document.set_dim_style(styled);
        let wanted = all_kinds();
        for d in &wanted {
            document.add(Entity::Dimension(Box::new(d.clone())));
        }
        let back = read_back(&document);
        let got: Vec<Dimension> = back
            .objects()
            .iter()
            .filter_map(|o| match &o.entity {
                Entity::Dimension(d) => Some((**d).clone()),
                _ => None,
            })
            .collect();
        assert_eq!(got.len(), wanted.len());
        for (got, want) in got.iter().zip(&wanted) {
            assert_eq!(
                std::mem::discriminant(&got.kind),
                std::mem::discriminant(&want.kind)
            );
            assert!(
                got.points()
                    .iter()
                    .zip(want.points())
                    .all(|(a, b)| near(*a, b)),
                "{want:?}"
            );
            assert_eq!(got.text, want.text);
            assert_eq!(got.text_pos.is_some(), want.text_pos.is_some());
            assert_eq!(got.text_angle.is_some(), want.text_angle.is_some());
            assert!((got.measurement().unwrap() - want.measurement().unwrap()).abs() < 1e-9);
            // They were made in the default style, which the file's table
            // does not have; the dimensions carry the difference.
            assert_eq!(got.style.text_height, want.style.text_height);
        }
    }

    #[test]
    fn the_style_table_and_a_dimensions_overrides_survive() {
        let mut document = Document::default();
        let mut big = DimStyle {
            name: "Big".to_owned(),
            text_height: 6.0,
            arrow: od_core::Arrow::Tick,
            ..DimStyle::default()
        };
        document.set_dim_style(big.clone());
        let mut d = Dimension::aligned(p(0.0, 0.0), p(10.0, 0.0), p(0.0, 5.0), &big);
        d.style.precision = 0;
        document.add(Entity::Dimension(Box::new(d)));
        let back = read_back(&document);
        assert!(back.dim_style_named("Big").is_some());
        assert_eq!(
            back.dim_styles()[0].name,
            "Standard",
            "Standard stays first"
        );
        big.name = "Big".to_owned();
        let Entity::Dimension(got) = &back.objects()[0].entity else {
            panic!("a dimension");
        };
        assert_eq!(got.style.name, "Big");
        assert_eq!(got.style.text_height, 6.0);
        assert_eq!(got.style.arrow, od_core::Arrow::Tick);
        assert_eq!(got.style.precision, 0, "the override came back");
        assert_eq!(got.label(), "10");
    }

    #[test]
    fn a_dimension_keeps_its_layer_and_its_block_is_anonymous() {
        let mut document = Document::default();
        let layer = document.allocate_layer_id();
        document.push_layer(Layer::new(layer, "Dims"));
        document.add_with(
            Entity::Dimension(Box::new(all_kinds().remove(0))),
            od_core::Attributes::on_layer(layer),
        );
        let text = String::from_utf8(write2000::write(&document).unwrap().bytes).unwrap();
        assert!(text.contains("AcDbDimension"));
        assert!(text.contains("*D1"));
        let back = read_back(&document);
        let object = &back.objects()[0];
        assert_eq!(back.layer(object.attrs.layer).unwrap().name, "Dims");
    }
}
