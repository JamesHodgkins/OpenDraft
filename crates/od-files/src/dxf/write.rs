//! Writing DXF as R12 (AC1009): the oldest version every CAD program still
//! opens, with no handles or object tables to get subtly wrong. The price is
//! what R12 cannot hold: lineweights, and colours beyond the 255 AutoCAD
//! indexes (the nearest index is written). The notes returned say so.

use std::collections::BTreeSet;
use std::fmt::Display;

use od_core::{arc, Attr, Color, Document, Entity, InsertUnits, Layer, LayerId, Lineweight, Point};

use super::units::header as units_header;
use crate::{Encoded, FileError};
use od_core::aci::{aci_rgb, nearest_aci};

/// Characters DXF forbids in layer names.
const FORBIDDEN: &[char] = &[
    '<', '>', '/', '\\', '"', ':', ';', '?', '*', '|', '=', '`', ',',
];

pub(super) struct Out {
    pub(super) text: String,
}

impl Out {
    pub(super) fn new() -> Self {
        Self {
            text: String::new(),
        }
    }

    pub(super) fn pair(&mut self, code: i32, value: impl Display) {
        self.text.push_str(&format!("{code:>3}\r\n{value}\r\n"));
    }

    pub(super) fn section(&mut self, name: &str) {
        self.pair(0, "SECTION");
        self.pair(2, name);
    }

    pub(super) fn end_section(&mut self) {
        self.pair(0, "ENDSEC");
    }

    fn point(&mut self, base: i32, x: f64, y: f64) {
        self.point3(base, x, y, 0.0);
    }

    pub(super) fn point3(&mut self, base: i32, x: f64, y: f64, z: f64) {
        self.pair(base, x);
        self.pair(base + 10, y);
        self.pair(base + 20, z);
    }
}

/// A layer or linetype name as DXF wants it: forbidden characters replaced.
/// Latin-1 letters stay (the file is written in code page 1252, see
/// `latin1`); anything beyond that is written as `\U+XXXX`.
pub(super) fn dxf_name(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if FORBIDDEN.contains(&c) || c.is_control() {
            out.push('_');
        } else if u32::from(c) < 0x100 {
            out.push(c);
        } else {
            let mut buffer = [0u16; 2];
            for unit in c.encode_utf16(&mut buffer) {
                out.push_str(&format!("\\U+{unit:04X}"));
            }
        }
    }
    out
}

/// The text as the single bytes of code page 1252 that R12 files use. Every
/// character is below U+0100 by now: names beyond that were escaped.
pub(super) fn latin1(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
        .collect()
}

/// The AutoCAD index for a colour, and whether it is only an approximation.
pub(super) fn aci_for(color: Color) -> (i32, bool) {
    match color {
        Color::Foreground => (7, false),
        Color::Rgb(r, g, b) => {
            let index = nearest_aci((r, g, b));
            (i32::from(index), aci_rgb(index) != (r, g, b))
        }
    }
}

pub(super) fn linetype_for(name: &str) -> String {
    let name = dxf_name(name).to_uppercase();
    if name.is_empty() || name == "BYLAYER" || name == "BYBLOCK" {
        "CONTINUOUS".to_owned()
    } else {
        name
    }
}

/// Dimensions are written as the pieces they are drawn with (see `dimensions`).
pub(super) fn unexploded() -> FileError {
    FileError::Invalid("an entity the DXF writer cannot write reached it".into())
}

pub(super) fn finite(value: f64) -> Result<f64, FileError> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(FileError::Invalid(
            "a coordinate is not a finite number".into(),
        ))
    }
}

/// Whether the drawing needs R2000: R12 has one unnamed paper space, so
/// layouts need it, and R12 has no ellipse, spline, hatch or multiline text (AutoCAD turns
/// them into lines, arcs and polylines when saving that far back, which we
/// do not guess at).
fn needs_r2000(document: &Document) -> bool {
    !document.layouts().is_empty()
        || document.objects().iter().any(|o| {
            matches!(
                o.entity,
                Entity::Ellipse(_)
                    | Entity::Spline(_)
                    | Entity::Hatch(_)
                    | Entity::MText(_)
                    | Entity::MLeader(_)
                    | Entity::Leader(_)
                    | Entity::Xline { .. }
                    | Entity::Ray { .. }
            )
        })
}

pub fn write(document: &Document) -> Result<Encoded, FileError> {
    if needs_r2000(document) {
        super::write2000::write(document)
    } else {
        write_r12(document)
    }
}

/// The drawing as R12. An ellipse or spline is refused, never approximated.
pub(super) fn write_r12(document: &Document) -> Result<Encoded, FileError> {
    let mut out = Out::new();
    let mut approximated = false;
    let mut lineweights = false;

    // Linetypes in use: the standard ones the app offers, plus whatever
    // imported drawings brought.
    let mut linetypes: BTreeSet<String> = BTreeSet::new();
    linetypes.insert("CONTINUOUS".into());
    for layer in document.layers() {
        linetypes.insert(super::linetypes::name_for(document, &layer.linetype));
        lineweights |= layer.lineweight != Lineweight::DEFAULT;
    }
    for object in document.model_objects() {
        if let Attr::Set(name) = &object.attrs.linetype {
            linetypes.insert(super::linetypes::name_for(document, name));
        }
        lineweights |= matches!(object.attrs.lineweight, Attr::Set(_));
    }
    linetypes.extend(super::linetypes::table_names(document));

    out.section("HEADER");
    out.pair(9, "$ACADVER");
    out.pair(1, "AC1009");
    out.pair(9, "$DWGCODEPAGE");
    out.pair(3, "ANSI_1252");
    for (name, code, value) in units_header(document.units()) {
        out.pair(9, name);
        out.pair(code, value);
    }
    out.pair(9, "$LTSCALE");
    out.pair(40, finite(document.linetype_scale())?);
    for (name, code, value) in super::point_style::header(document.point_style()) {
        out.pair(9, name);
        out.pair(code, value);
    }
    if let Some(bounds) = document.model_bounds() {
        out.pair(9, "$EXTMIN");
        out.point(10, finite(bounds.min.x)?, finite(bounds.min.y)?);
        out.pair(9, "$EXTMAX");
        out.point(10, finite(bounds.max.x)?, finite(bounds.max.y)?);
    }
    out.end_section();

    out.section("TABLES");
    out.pair(0, "TABLE");
    out.pair(2, "LTYPE");
    out.pair(70, linetypes.len());
    for name in &linetypes {
        let (description, dashes) = super::linetypes::definition(document, name);
        out.pair(0, "LTYPE");
        out.pair(2, name);
        out.pair(70, 0);
        out.pair(3, description);
        out.pair(72, 65);
        out.pair(73, dashes.len());
        out.pair(40, dashes.iter().map(|d| d.abs()).sum::<f64>());
        for dash in dashes {
            out.pair(49, dash);
        }
    }
    out.pair(0, "ENDTAB");

    out.pair(0, "TABLE");
    out.pair(2, "LAYER");
    out.pair(70, document.layers().len());
    for layer in document.layers() {
        let (index, approximate) = aci_for(layer.color);
        approximated |= approximate;
        out.pair(0, "LAYER");
        out.pair(2, dxf_name(&layer.name));
        out.pair(70, i32::from(layer.frozen) + 4 * i32::from(layer.locked));
        out.pair(62, if layer.on { index } else { -index });
        out.pair(6, super::linetypes::name_for(document, &layer.linetype));
    }
    out.pair(0, "ENDTAB");
    out.end_section();

    let layer_name = |id: LayerId| -> String {
        document
            .layer(id)
            .map_or_else(|| "0".to_owned(), |l: &Layer| dxf_name(&l.name))
    };

    out.section("ENTITIES");
    for object in document.model_objects() {
        let layer = layer_name(object.attrs.layer);
        // Group 8 (layer), then colour and linetype when set on the object.
        let mut common = |out: &mut Out| -> Result<(), FileError> {
            out.pair(8, &layer);
            if let Attr::Set(name) = &object.attrs.linetype {
                out.pair(6, super::linetypes::name_for(document, name));
            }
            if object.attrs.linetype_scale != 1.0 {
                out.pair(48, finite(object.attrs.linetype_scale)?);
            }
            if let Attr::Set(color) = object.attrs.color {
                let (index, approximate) = aci_for(color);
                approximated |= approximate;
                out.pair(62, index);
            }
            Ok(())
        };
        match &object.entity {
            Entity::Dimension(_) | Entity::MText(_) | Entity::Insert(_) | Entity::MLeader(_) => {
                return Err(unexploded())
            }
            Entity::Leader(_) => {
                return Err(FileError::Unsupported(
                    "R12 DXF cannot hold leaders; this drawing needs R2000".into(),
                ))
            }
            // `needs_r2000` sends these to the R2000 writer.
            Entity::Ellipse(_) => {
                return Err(FileError::Unsupported(
                    "R12 DXF cannot hold ellipses; this drawing needs R2000".into(),
                ))
            }
            Entity::Spline(_) => {
                return Err(FileError::Unsupported(
                    "R12 DXF cannot hold splines; this drawing needs R2000".into(),
                ))
            }
            Entity::Hatch(_) => {
                return Err(FileError::Unsupported(
                    "R12 DXF cannot hold hatches; this drawing needs R2000".into(),
                ))
            }
            Entity::Xline { .. } | Entity::Ray { .. } => {
                return Err(FileError::Unsupported(
                    "R12 DXF cannot hold construction lines or rays; this drawing needs R2000"
                        .into(),
                ))
            }
            Entity::Point { at } => {
                out.pair(0, "POINT");
                common(&mut out)?;
                out.point(10, finite(at.x)?, finite(at.y)?);
            }
            Entity::Line { start, end } => {
                out.pair(0, "LINE");
                common(&mut out)?;
                out.point(10, finite(start.x)?, finite(start.y)?);
                out.point(11, finite(end.x)?, finite(end.y)?);
            }
            Entity::Circle { center, radius } => {
                out.pair(0, "CIRCLE");
                common(&mut out)?;
                out.point(10, finite(center.x)?, finite(center.y)?);
                out.pair(40, finite(*radius)?);
            }
            Entity::Arc {
                center,
                radius,
                start,
                end,
            } => {
                out.pair(0, "ARC");
                common(&mut out)?;
                out.point(10, finite(center.x)?, finite(center.y)?);
                out.pair(40, finite(*radius)?);
                out.pair(50, finite(arc::normalize(*start).to_degrees())?);
                out.pair(51, finite(arc::normalize(*end).to_degrees())?);
            }
            Entity::Rectangle { min, max } => {
                let corners = [
                    Point::new(min.x, min.y),
                    Point::new(max.x, min.y),
                    Point::new(max.x, max.y),
                    Point::new(min.x, max.y),
                ];
                polyline(
                    &mut out,
                    &mut common,
                    &layer,
                    &corners,
                    &[],
                    &[],
                    true,
                    object.attrs.continuous_generation,
                )?;
            }
            Entity::Polyline {
                points,
                closed,
                bulges,
                widths,
            } => {
                polyline(
                    &mut out,
                    &mut common,
                    &layer,
                    points,
                    bulges,
                    widths,
                    *closed,
                    object.attrs.continuous_generation,
                )?;
            }
        }
    }
    out.end_section();
    out.pair(0, "EOF");

    let mut notes = Vec::new();
    if approximated {
        notes.push("Colours were matched to the nearest AutoCAD colour index.".to_owned());
    }
    if lineweights {
        notes.push("Lineweights are not part of this DXF version and were left out.".to_owned());
    }
    if document
        .model_objects()
        .any(|o| matches!(o.entity, Entity::Rectangle { .. }))
    {
        notes.push("Rectangles were written as closed polylines.".to_owned());
    }
    if document.units().insert != InsertUnits::UNITLESS {
        notes.push(
            "The insertion units are not part of this DXF version and were left out.".to_owned(),
        );
    }
    Ok(Encoded {
        bytes: latin1(&out.text),
        notes,
    })
}

/// An old-style POLYLINE: a header, one VERTEX per point, then SEQEND.
#[allow(clippy::too_many_arguments)]
fn polyline(
    out: &mut Out,
    common: &mut impl FnMut(&mut Out) -> Result<(), FileError>,
    layer: &str,
    points: &[Point],
    bulges: &[f64],
    widths: &[(f64, f64)],
    closed: bool,
    continuous: bool,
) -> Result<(), FileError> {
    out.pair(0, "POLYLINE");
    common(out)?;
    out.pair(66, 1);
    out.point(10, 0.0, 0.0);
    out.pair(70, i32::from(closed) | if continuous { 128 } else { 0 });
    for (i, p) in points.iter().enumerate() {
        out.pair(0, "VERTEX");
        out.pair(8, layer);
        out.point(10, finite(p.x)?, finite(p.y)?);
        // A bulge turns the segment from this vertex to the next into an arc.
        let bulge = od_core::bulge::at(bulges, i);
        if bulge != 0.0 {
            out.pair(42, finite(bulge)?);
        }
        // Start and end width of that same segment.
        let (start, end) = od_core::width::at(widths, i);
        if start != 0.0 || end != 0.0 {
            out.pair(40, finite(start)?);
            out.pair(41, finite(end)?);
        }
    }
    out.pair(0, "SEQEND");
    out.pair(8, layer);
    Ok(())
}

#[cfg(test)]
mod tests {
    use od_core::{Attributes, Entity};

    use super::super::read::read;
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn text(document: &Document) -> String {
        String::from_utf8(write(document).unwrap().bytes).unwrap()
    }

    fn rich() -> Document {
        let mut document = Document::default();
        let walls = document.allocate_layer_id();
        document.push_layer(Layer {
            color: Color::Rgb(255, 0, 0),
            linetype: "Dashed".into(),
            locked: true,
            ..Layer::new(walls, "Walls")
        });
        let hidden = document.allocate_layer_id();
        document.push_layer(Layer {
            on: false,
            frozen: true,
            color: Color::Rgb(0, 0, 255),
            ..Layer::new(hidden, "Hidden")
        });
        document.add_with(
            Entity::Line {
                start: p(0.0, 0.0),
                end: p(10.5, -3.25),
            },
            Attributes::on_layer(walls),
        );
        document.add(Entity::Circle {
            center: p(1.0, 2.0),
            radius: 0.5,
        });
        document.add(Entity::Arc {
            center: p(5.0, 5.0),
            radius: 2.5,
            start: 5.0,
            end: 1.0,
        });
        document.add_with(
            Entity::Polyline {
                points: vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)],
                closed: true,
                bulges: vec![0.0, 0.5, -0.25],
                widths: Vec::new(),
            },
            Attributes {
                layer: LayerId::DEFAULT,
                color: Attr::Set(Color::Rgb(0, 255, 0)),
                linetype: Attr::Set("Center".into()),
                lineweight: Attr::ByLayer,
                linetype_scale: 2.5,
                continuous_generation: true,
            },
        );
        document.add(Entity::Point { at: p(7.5, -2.0) });
        document.set_point_style(od_core::PointStyle {
            mode: 35,
            size: -4.0,
        });
        document
    }

    #[test]
    fn what_we_write_we_can_read_back() {
        let document = rich();
        let encoded = write(&document).unwrap();
        let loaded = read(&encoded.bytes).unwrap();
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
        let back = loaded.document;
        // Angles go through degrees, which can differ in the last digit.
        let entities = |d: &Document| {
            d.objects()
                .iter()
                .map(|o| match o.entity.clone() {
                    Entity::Arc {
                        center,
                        radius,
                        start,
                        end,
                    } => Entity::Arc {
                        center,
                        radius,
                        start: (start * 1e9).round() / 1e9,
                        end: (end * 1e9).round() / 1e9,
                    },
                    other => other,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(entities(&back), entities(&document));
        assert_eq!(back.point_style(), document.point_style());

        let layer = |name: &str| {
            back.layers()
                .iter()
                .find(|l| l.name == name)
                .unwrap()
                .clone()
        };
        assert_eq!(layer("Walls").color, Color::Rgb(255, 0, 0));
        assert_eq!(layer("Walls").linetype, "Dashed");
        assert!(layer("Walls").locked);
        let hidden = layer("Hidden");
        assert!(!hidden.on && hidden.frozen && hidden.color == Color::Rgb(0, 0, 255));
        assert_eq!(layer("0").color, Color::Foreground);

        let polyline = &back.objects()[3];
        assert_eq!(polyline.attrs.color, Attr::Set(Color::Rgb(0, 255, 0)));
        assert_eq!(polyline.attrs.linetype, Attr::Set("Center".into()));
        assert_eq!(
            back.layer(back.objects()[0].attrs.layer).unwrap().name,
            "Walls"
        );
    }

    #[test]
    fn rectangles_become_closed_polylines() {
        let mut document = Document::default();
        document.add(Entity::Rectangle {
            min: p(1.0, 2.0),
            max: p(4.0, 6.0),
        });
        let encoded = write(&document).unwrap();
        assert!(encoded.notes.iter().any(|n| n.contains("Rectangles")));
        let back = read(&encoded.bytes).unwrap().document;
        assert_eq!(
            back.objects()[0].entity,
            Entity::Polyline {
                points: vec![p(1.0, 2.0), p(4.0, 2.0), p(4.0, 6.0), p(1.0, 6.0)],
                closed: true,
                bulges: Vec::new(),
                widths: Vec::new(),
            }
        );
    }

    #[test]
    fn notes_say_what_could_not_be_kept() {
        let mut document = Document::default();
        let id = document.allocate_layer_id();
        document.push_layer(Layer {
            color: Color::Rgb(12, 200, 99),
            lineweight: Lineweight(60),
            ..Layer::new(id, "Odd")
        });
        let notes = write(&document).unwrap().notes;
        assert!(notes.iter().any(|n| n.contains("nearest AutoCAD colour")));
        assert!(notes.iter().any(|n| n.contains("Lineweights")));
        assert!(write(&Document::default()).unwrap().notes.is_empty());
    }

    #[test]
    fn units_are_written_to_the_header_and_read_back() {
        let mut document = Document::default();
        let units = od_core::Units {
            length: od_core::LengthFormat::Architectural,
            length_precision: 4,
            angle: od_core::AngleFormat::DegreesMinutesSeconds,
            angle_precision: 2,
            clockwise: true,
            base_angle: std::f64::consts::FRAC_PI_2,
            insert: InsertUnits::from_code(1).unwrap(),
        };
        document.set_units(units);
        let encoded = write(&document).unwrap();
        assert!(encoded.notes.iter().any(|n| n.contains("insertion units")));
        let back = *read(&encoded.bytes).unwrap().document.units();
        assert_eq!(back.insert, InsertUnits::UNITLESS, "R12 cannot hold it");
        assert_eq!(
            back,
            od_core::Units {
                insert: InsertUnits::UNITLESS,
                ..units
            }
        );
    }

    #[test]
    fn names_are_made_safe_and_non_ascii_survives() {
        assert_eq!(dxf_name("a/b:c"), "a_b_c");
        assert_eq!(dxf_name("Café"), "Café", "Latin-1 letters are kept");
        assert_eq!(dxf_name("Ω1"), "\\U+03A91");
        let mut document = Document::default();
        let id = document.allocate_layer_id();
        document.push_layer(Layer::new(id, "Café <x>"));
        let bytes = write(&document).unwrap().bytes;
        assert!(bytes.contains(&0xE9), "one byte, in code page 1252");
        let back = read(&bytes).unwrap().document;
        assert!(back.layers().iter().any(|l| l.name == "Café _x_"));
    }

    #[test]
    fn the_file_has_the_r12_shape() {
        let t = text(&rich());
        assert!(t.starts_with(
            "  0\r\nSECTION\r\n  2\r\nHEADER\r\n  9\r\n$ACADVER\r\n  1\r\nAC1009\r\n"
        ));
        assert!(t.ends_with("  0\r\nEOF\r\n"));
        for section in ["HEADER", "TABLES", "ENTITIES"] {
            assert!(t.contains(&format!("  2\r\n{section}\r\n")), "{section}");
        }
        // Every linetype used has a definition.
        for name in ["CONTINUOUS", "DASHED", "CENTER"] {
            assert!(
                t.contains(&format!("  0\r\nLTYPE\r\n  2\r\n{name}\r\n")),
                "{name}"
            );
        }
    }

    #[test]
    fn an_independent_parser_accepts_the_output() {
        let document = rich();
        let bytes = write(&document).unwrap().bytes;
        let parsed = dxf::Drawing::load(&mut std::io::Cursor::new(bytes)).expect("a valid DXF");
        let layers: Vec<String> = parsed.layers().map(|l| l.name.clone()).collect();
        for name in ["0", "Walls", "Hidden"] {
            assert!(
                layers.iter().any(|l| l == name),
                "layer {name} in {layers:?}"
            );
        }
        let entities: Vec<_> = parsed.entities().collect();
        assert_eq!(entities.len(), 5);
        assert!(matches!(
            entities[4].specific,
            dxf::entities::EntityType::ModelPoint(_)
        ));
        assert!(matches!(
            entities[0].specific,
            dxf::entities::EntityType::Line(_)
        ));
        assert!(matches!(
            entities[1].specific,
            dxf::entities::EntityType::Circle(_)
        ));
        let dxf::entities::EntityType::Arc(arc) = &entities[2].specific else {
            panic!("expected an arc");
        };
        assert!((arc.radius - 2.5).abs() < 1e-9);
        assert!((arc.start_angle - 5.0_f64.to_degrees()).abs() < 1e-6);
        assert!((arc.end_angle - 1.0_f64.to_degrees()).abs() < 1e-6);
        let dxf::entities::EntityType::Polyline(poly) = &entities[3].specific else {
            panic!("expected a polyline");
        };
        assert!(poly.is_closed());
        assert_eq!(poly.vertices().count(), 3);
        // The arcs travel as bulges on the vertices they start from.
        let bulges: Vec<f64> = poly.vertices().map(|v| v.bulge).collect();
        assert_eq!(bulges, vec![0.0, 0.5, -0.25]);
        assert_eq!(entities[0].common.layer, "Walls");
    }

    #[test]
    fn polyline_widths_survive_a_round_trip() {
        let mut document = Document::default();
        let wide = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 3.0)],
            closed: false,
            bulges: Vec::new(),
            widths: vec![(0.5, 0.25), (0.1, 0.1), (0.0, 0.0)],
        };
        // A two-point polyline with a width must not be read back as a line.
        let single = Entity::Polyline {
            points: vec![p(0.0, 5.0), p(2.0, 5.0)],
            closed: false,
            bulges: Vec::new(),
            widths: vec![(1.0, 1.0), (0.0, 0.0)],
        };
        document.add(wide.clone());
        document.add(single.clone());
        let back = read(&write(&document).unwrap().bytes).unwrap().document;
        let kept: Vec<_> = back.objects().iter().map(|o| o.entity.clone()).collect();
        assert_eq!(kept, [wide, single]);
    }

    #[test]
    fn non_finite_coordinates_are_refused() {
        let mut document = Document::default();
        document.add(Entity::Line {
            start: p(f64::NAN, 0.0),
            end: p(1.0, 1.0),
        });
        assert!(matches!(write(&document), Err(FileError::Invalid(_))));
    }
}
