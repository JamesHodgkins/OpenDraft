//! Writing DXF R2000 (AC1015), used when a drawing has layouts, ellipses or
//! splines: R12 has no ellipse or spline entity, and one unnamed paper space,
//! while R2000 keeps `ELLIPSE` and `SPLINE` as they are and named layouts,
//! their sheet sizes and
//! viewports (scale, lock, layers frozen in them) and what is drawn on each
//! sheet. The structure follows what AutoCAD writes (and what ezdxf, which
//! reads and writes it widely, produces): block records and blocks per
//! space, `LAYOUT` objects in the `ACAD_LAYOUT` dictionary, the first sheet's
//! entities in `ENTITIES` with group 67 = 1 and the others inside their
//! `*Paper_SpaceN` blocks. Not tested against AutoCAD itself.

use std::collections::{BTreeSet, HashMap};

use od_core::{Attr, Color, Document, LayerId, Lineweight};

mod entities;
mod hatch;
mod leader;
mod mleader;
mod objects;
mod tables;
mod text;

use entities::{write_object, write_viewports};
use objects::write_objects;
use tables::write_tables;

use super::units::header as units_header;
use super::write::{finite, latin1, Out};
use crate::{Encoded, FileError};

/// Hands out handles (hexadecimal ids) one after another.
struct Handles(u32);

impl Handles {
    fn next(&mut self) -> u32 {
        self.0 += 1;
        self.0
    }
}

fn hex(handle: u32) -> String {
    format!("{handle:X}")
}

/// The handles that other objects refer to, made before anything is written.
struct Ids {
    model_record: u32,
    model_layout: u32,
    /// Per layout, in tab order.
    records: Vec<u32>,
    layouts: Vec<u32>,
    overall: Vec<u32>,
    layers: HashMap<LayerId, u32>,
    placeholder: u32,
    /// One per text style, in table order.
    text_styles: Vec<u32>,
    bylayer_ltype: u32,
    /// The `ACAD_MLEADERSTYLE` dictionary and one per multileader style.
    mleader_dict: u32,
    mleader_styles: Vec<u32>,
    /// The text a leader annotates, which a leader names by handle.
    annotations: HashMap<od_core::EntityId, u32>,
}

/// `*Paper_Space`, `*Paper_Space0`, `*Paper_Space1`... by tab position.
fn block_name(index: usize) -> String {
    match index {
        0 => "*Paper_Space".to_owned(),
        n => format!("*Paper_Space{}", n - 1),
    }
}

/// The nearest lineweight AutoCAD accepts (hundredths of a millimetre).
fn weight_code(weight: Lineweight) -> i32 {
    if weight == Lineweight::DEFAULT {
        return -3;
    }
    let nearest = Lineweight::STANDARD
        .iter()
        .min_by_key(|w| w.0.abs_diff(weight.0))
        .copied()
        .unwrap_or(Lineweight::DEFAULT);
    i32::from(nearest.0)
}

fn true_color(color: Color) -> Option<i32> {
    match color {
        Color::Foreground => None,
        Color::Rgb(r, g, b) => Some((i32::from(r) << 16) | (i32::from(g) << 8) | i32::from(b)),
    }
}

pub fn write(document: &Document) -> Result<Encoded, FileError> {
    let layouts = document.layouts();
    let mut h = Handles(0x0F);
    let mut ids = Ids {
        model_record: h.next(),
        model_layout: h.next(),
        records: layouts.iter().map(|_| h.next()).collect(),
        layouts: layouts.iter().map(|_| h.next()).collect(),
        overall: layouts.iter().map(|_| h.next()).collect(),
        layers: HashMap::new(),
        placeholder: h.next(),
        text_styles: document.text_styles().iter().map(|_| h.next()).collect(),
        bylayer_ltype: h.next(),
        mleader_dict: h.next(),
        mleader_styles: document.mleader_styles().iter().map(|_| h.next()).collect(),
        annotations: HashMap::new(),
    };
    for id in leader::annotation_ids(document) {
        ids.annotations.insert(id, h.next());
    }
    let leaders = mleader::has_any(document);
    for layer in document.layers() {
        ids.layers.insert(layer.id, h.next());
    }

    let mut linetypes: BTreeSet<String> = BTreeSet::new();
    for layer in document.layers() {
        linetypes.insert(super::linetypes::name_for(document, &layer.linetype));
    }
    for object in document.objects() {
        if let Attr::Set(name) = &object.attrs.linetype {
            linetypes.insert(super::linetypes::name_for(document, name));
        }
    }
    linetypes.extend(super::linetypes::table_names(document));
    linetypes.remove("CONTINUOUS");

    let mut classes = Out::new();
    classes.section("CLASSES");
    for (name, class, app, flags, proxy, entity) in [
        (
            "ACDBDICTIONARYWDFLT",
            "AcDbDictionaryWithDefault",
            "ObjectDBX Classes",
            0,
            0,
            0,
        ),
        (
            "ACDBPLACEHOLDER",
            "AcDbPlaceHolder",
            "ObjectDBX Classes",
            0,
            0,
            0,
        ),
        ("LAYOUT", "AcDbLayout", "ObjectDBX Classes", 0, 0, 0),
        (
            "MLEADERSTYLE",
            "AcDbMLeaderStyle",
            "ACDB_MLEADERSTYLE_CLASS",
            4095,
            7,
            0,
        ),
        (
            "MULTILEADER",
            "AcDbMLeader",
            "ACDB_MLEADER_CLASS",
            1025,
            10,
            1,
        ),
    ] {
        if name.starts_with('M') && !leaders {
            continue;
        }
        classes.pair(0, "CLASS");
        classes.pair(1, name);
        classes.pair(2, class);
        classes.pair(3, app);
        classes.pair(90, flags);
        if proxy != 0 {
            classes.pair(91, proxy);
        }
        classes.pair(280, 0);
        classes.pair(281, entity);
    }
    classes.end_section();

    let mut tables = Out::new();
    write_tables(&mut tables, &mut h, &ids, document, &linetypes);

    // Model space entities, then the first sheet's, go in ENTITIES; the
    // other sheets' go inside their blocks.
    let mut blocks = Out::new();
    blocks.section("BLOCKS");
    let mut entities = Out::new();
    entities.section("ENTITIES");
    block(
        &mut blocks,
        &mut h,
        "*Model_Space",
        ids.model_record,
        |_, _| Ok(()),
    )?;
    for object in document.objects().iter().filter(|o| o.layout.is_none()) {
        write_object(
            &mut entities,
            &mut h,
            &ids,
            document,
            object,
            ids.model_record,
            false,
        )?;
    }
    for (i, layout) in layouts.iter().enumerate() {
        let owner = ids.records[i];
        let mut sheet = |out: &mut Out, h: &mut Handles| -> Result<(), FileError> {
            write_viewports(out, h, &ids, layout, i)?;
            for object in document
                .objects()
                .iter()
                .filter(|o| o.layout == Some(layout.id))
            {
                write_object(out, h, &ids, document, object, owner, true)?;
            }
            Ok(())
        };
        if i == 0 {
            block(&mut blocks, &mut h, &block_name(0), owner, |_, _| Ok(()))?;
            sheet(&mut entities, &mut h)?;
        } else {
            block(&mut blocks, &mut h, &block_name(i), owner, &mut sheet)?;
        }
    }
    blocks.end_section();
    entities.end_section();

    let mut objects = Out::new();
    write_objects(&mut objects, &mut h, &ids, document, leaders)?;

    let mut out = Out::new();
    out.section("HEADER");
    out.pair(9, "$ACADVER");
    // Multileaders need AutoCAD 2008's file (AC1021); nothing else here does.
    out.pair(1, if leaders { "AC1021" } else { "AC1015" });
    out.pair(9, "$ACADMAINTVER");
    out.pair(70, 6);
    out.pair(9, "$DWGCODEPAGE");
    out.pair(3, "ANSI_1252");
    out.pair(9, "$LTSCALE");
    out.pair(40, finite(document.linetype_scale())?);
    out.pair(9, "$INSBASE");
    out.point3(10, 0.0, 0.0, 0.0);
    let (lo, hi) = match document.model_bounds() {
        Some(b) => (
            (finite(b.min.x)?, finite(b.min.y)?),
            (finite(b.max.x)?, finite(b.max.y)?),
        ),
        None => ((1e20, 1e20), (-1e20, -1e20)),
    };
    out.pair(9, "$EXTMIN");
    out.point3(10, lo.0, lo.1, if lo.0 > 1e19 { 1e20 } else { 0.0 });
    out.pair(9, "$EXTMAX");
    out.point3(10, hi.0, hi.1, if hi.0 < -1e19 { -1e20 } else { 0.0 });
    out.pair(9, "$HANDSEED");
    out.pair(5, hex(h.0 + 1));
    out.pair(9, "$TILEMODE");
    out.pair(70, 1);
    out.pair(9, "$INSUNITS");
    out.pair(70, document.units().insert.code());
    for (name, code, value) in units_header(document.units()) {
        out.pair(9, name);
        out.pair(code, value);
    }
    out.end_section();
    for part in [classes, tables, blocks, entities, objects] {
        out.text.push_str(&part.text);
    }
    out.pair(0, "EOF");

    let has = |spline: bool| {
        document.objects().iter().any(|o| match o.entity {
            od_core::Entity::Spline(_) => spline,
            od_core::Entity::Ellipse(_) => !spline,
            _ => false,
        })
    };
    let hatches = document
        .objects()
        .iter()
        .any(|o| matches!(o.entity, od_core::Entity::Hatch(_)));
    let texts = document
        .objects()
        .iter()
        .any(|o| matches!(o.entity, od_core::Entity::MText(_)));
    let why = match (layouts.is_empty(), has(false), has(true)) {
        (true, false, false) if texts && !hatches => {
            "Written as DXF 2000 so that multiline text is kept (DXF R12 has none)."
        }
        (true, false, false) if hatches => {
            "Written as DXF 2000 so that hatches are kept (DXF R12 has none)."
        }
        (false, ..) => "Written as DXF 2000 so that layouts and viewports are kept.",
        (true, true, true) => {
            "Written as DXF 2000 so that ellipses and splines are kept (DXF R12 has neither)."
        }
        (true, false, true) => "Written as DXF 2000 so that splines are kept (DXF R12 has none).",
        _ => "Written as DXF 2000 so that ellipses are kept (DXF R12 has none).",
    };
    let mut notes = vec![
        why.to_owned(),
        "Rectangles were written as closed polylines.".to_owned(),
    ];
    if texts {
        notes.push(
            "Text styles that use the bundled font were written as Arial; columns were not written."
                .to_owned(),
        );
    }
    if hatches {
        notes.push(
            "Hatches were written without the objects they follow (not associative).".to_owned(),
        );
    }
    Ok(Encoded {
        bytes: latin1(&out.text),
        notes,
    })
}

/// A BLOCK ... ENDBLK pair around whatever `body` writes.
fn block(
    out: &mut Out,
    h: &mut Handles,
    name: &str,
    record: u32,
    mut body: impl FnMut(&mut Out, &mut Handles) -> Result<(), FileError>,
) -> Result<(), FileError> {
    out.pair(0, "BLOCK");
    out.pair(5, hex(h.next()));
    out.pair(330, hex(record));
    out.pair(100, "AcDbEntity");
    out.pair(8, 0);
    out.pair(100, "AcDbBlockBegin");
    out.pair(2, name);
    out.pair(70, 0);
    out.point3(10, 0.0, 0.0, 0.0);
    out.pair(3, name);
    out.pair(1, "");
    body(out, h)?;
    out.pair(0, "ENDBLK");
    out.pair(5, hex(h.next()));
    out.pair(330, hex(record));
    out.pair(100, "AcDbEntity");
    out.pair(8, 0);
    out.pair(100, "AcDbBlockEnd");
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use od_core::{
        Attributes, Ellipse, Entity, Layer, Layout, LayoutId, Point, Spline, ViewportId,
    };

    use super::*;

    /// A drawing with a Walls layer, a model line, and two layouts: "Plan" (a
    /// locked 1:50 viewport with Walls frozen in it, an off viewport, a line on
    /// the sheet) and "Detail" (a circle on its sheet).
    fn drawing() -> Document {
        let p = Point::new;
        let mut doc = Document::default();
        let walls = doc.allocate_layer_id();
        doc.push_layer(Layer::new(walls, "Walls"));
        doc.add_with(
            Entity::Line {
                start: p(0.0, 0.0),
                end: p(10.0, 5.0),
            },
            Attributes::on_layer(walls),
        );
        let mut plan = doc.new_layout("Plan", 420.0, 297.0);
        plan.viewports[0].scale = 0.02;
        plan.viewports[0].locked = true;
        plan.viewports[0].set_frozen(&[walls], true);
        plan.viewports.push(od_core::Viewport {
            id: ViewportId(plan.viewports[0].id.0 + 1),
            on: false,
            frozen: Vec::new(),
            ..plan.viewports[0].clone()
        });
        let plan_id = plan.id;
        let detail = Layout {
            id: LayoutId(plan_id.0 + 1),
            name: "Detail".into(),
            width: 200.0,
            height: 150.0,
            viewports: Vec::new(),
        };
        let detail_id = detail.id;
        doc.set_layouts(vec![plan, detail]);
        doc.set_active_layout(Some(plan_id));
        doc.add(Entity::Line {
            start: p(5.0, 5.0),
            end: p(50.0, 5.0),
        });
        doc.set_active_layout(Some(detail_id));
        doc.add(Entity::Circle {
            center: p(100.0, 75.0),
            radius: 30.0,
        });
        doc.set_active_layout(None);
        doc
    }

    type Pairs = Vec<(i32, String)>;

    /// The file as (group code, value) pairs.
    fn pairs(bytes: &[u8]) -> Pairs {
        let text = String::from_utf8_lossy(bytes).into_owned();
        let lines: Vec<&str> = text.split("\r\n").collect();
        lines
            .chunks(2)
            .filter(|c| c.len() == 2)
            .map(|c| (c[0].trim().parse().unwrap(), c[1].to_owned()))
            .collect()
    }

    /// The first object of `kind` at or after `from`: where it starts, and its
    /// pairs up to the next group 0.
    fn object<'a>(
        pairs: &'a Pairs,
        kind: &str,
        from: usize,
    ) -> Option<(usize, &'a [(i32, String)])> {
        let start = from
            + pairs[from..]
                .iter()
                .position(|(c, v)| *c == 0 && v == kind)?;
        let len = pairs[start + 1..]
            .iter()
            .position(|(c, _)| *c == 0)
            .unwrap_or(pairs.len());
        Some((start, &pairs[start..(start + 1 + len).min(pairs.len())]))
    }

    fn value(object: &[(i32, String)], code: i32) -> Option<&str> {
        object
            .iter()
            .find(|(c, _)| *c == code)
            .map(|(_, v)| v.as_str())
    }

    #[test]
    fn handles_are_unique_every_reference_resolves_and_the_seed_is_above_them_all() {
        let bytes = write(&drawing()).unwrap().bytes;
        let pairs = pairs(&bytes);
        let mut handles = HashSet::new();
        let mut max = 0;
        let mut seed = None;
        let mut in_seed = false;
        for (code, v) in &pairs {
            match code {
                9 => in_seed = v == "$HANDSEED",
                5 if in_seed => {
                    seed = u32::from_str_radix(v, 16).ok();
                    in_seed = false;
                }
                5 | 105 => {
                    assert!(handles.insert(v.clone()), "handle {v} used twice");
                    max = max.max(u32::from_str_radix(v, 16).unwrap());
                }
                _ => {}
            }
        }
        assert!(seed.unwrap() > max);
        for (code, v) in &pairs {
            if [330, 331, 340, 350, 390].contains(code) && v != "0" {
                assert!(
                    handles.contains(v),
                    "group {code} refers to missing handle {v}"
                );
            }
        }
        assert_eq!(pairs.last().unwrap(), &(0, "EOF".to_owned()));
    }

    #[test]
    fn spaces_layouts_and_their_entities_are_where_autocad_puts_them() {
        let pairs = pairs(&write(&drawing()).unwrap().bytes);
        assert_eq!(pairs.iter().find(|(c, _)| *c == 1).unwrap().1, "AC1015");
        let names: Vec<&str> = pairs
            .iter()
            .filter(|(c, v)| *c == 2 && v.starts_with('*'))
            .map(|(_, v)| v.as_str())
            .collect();
        assert_eq!(
            names.iter().filter(|n| **n == "*Model_Space").count(),
            2,
            "record and block"
        );
        assert!(names.contains(&"*Paper_Space") && names.contains(&"*Paper_Space0"));
        // Each layout has a LAYOUT object, and the sheet size is in it.
        let model = object(&pairs, "LAYOUT", 0).unwrap().0;
        let plan = object(&pairs, "LAYOUT", model + 1).unwrap();
        assert_eq!(
            (value(plan.1, 44), value(plan.1, 45), value(plan.1, 71)),
            (Some("420"), Some("297"), Some("1"))
        );
        let detail = object(&pairs, "LAYOUT", plan.0 + 1).unwrap();
        assert_eq!(
            (value(detail.1, 44), value(detail.1, 71)),
            (Some("200"), Some("2"))
        );
        // Model entities carry no group 67; sheet entities do, and the second
        // sheet's are inside its block.
        let entities_at = pairs
            .iter()
            .position(|(c, v)| *c == 2 && v == "ENTITIES")
            .unwrap();
        let line = object(&pairs, "LINE", entities_at).unwrap();
        assert!(value(line.1, 67).is_none());
        let sheet_line = object(&pairs, "LINE", line.0 + 1).unwrap().1;
        assert_eq!(value(sheet_line, 67), Some("1"));
        let block_at = pairs
            .iter()
            .position(|(c, v)| *c == 2 && v == "*Paper_Space0")
            .unwrap();
        let circle = object(&pairs, "CIRCLE", block_at).unwrap();
        assert!(circle.0 < entities_at, "inside the BLOCKS section");
        assert_eq!(value(circle.1, 67), Some("1"));
    }

    #[test]
    fn viewports_keep_scale_lock_visibility_and_frozen_layers() {
        let pairs = pairs(&write(&drawing()).unwrap().bytes);
        let walls_record = {
            let at = pairs
                .iter()
                .position(|(c, v)| *c == 2 && v == "Walls")
                .unwrap();
            pairs[..at]
                .iter()
                .rev()
                .find(|(c, _)| *c == 5)
                .unwrap()
                .1
                .clone()
        };
        let mut found = Vec::new();
        let mut from = 0;
        while let Some((at, vp)) = object(&pairs, "VIEWPORT", from) {
            found.push(vp.to_vec());
            from = at + 1;
        }
        assert_eq!(found.len(), 4, "Plan: sheet-wide + two; Detail: sheet-wide");
        assert_eq!(
            found.iter().filter(|v| value(v, 69) == Some("1")).count(),
            2,
            "one sheet-wide per layout"
        );
        let main = found
            .iter()
            .find(|v| value(v, 69) == Some("2") && value(v, 68) == Some("2"))
            .unwrap();
        let off = found.iter().find(|v| value(v, 68) == Some("0")).unwrap();
        let number =
            |o: &[(i32, String)], code| -> f64 { value(o, code).unwrap().parse().unwrap() };
        assert!(
            (number(main, 41) / number(main, 45) - 0.02).abs() < 1e-12,
            "paper height over model height is the scale"
        );
        assert_eq!(value(main, 90), Some("16384"), "locked");
        let frozen: Vec<&str> = main
            .iter()
            .filter(|(c, _)| *c == 331)
            .map(|(_, v)| v.as_str())
            .collect();
        assert_eq!(frozen, [walls_record.as_str()]);
        assert_eq!(value(off, 68), Some("0"), "off");
        assert!(off.iter().all(|(c, _)| *c != 331));
    }

    #[test]
    fn an_independent_parser_accepts_it() {
        let bytes = write(&drawing()).unwrap().bytes;
        let parsed = dxf::Drawing::load(&mut std::io::Cursor::new(bytes)).expect("a valid DXF");
        assert!(parsed.layers().any(|l| l.name == "Walls"));
        // It reads the entities it knows (the model line and the sheet line).
        assert_eq!(
            parsed
                .entities()
                .filter(|e| matches!(e.specific, dxf::entities::EntityType::Line(_)))
                .count(),
            2
        );
    }

    #[test]
    fn colours_lineweights_and_widths_are_kept_in_r2000() {
        let mut doc = drawing();
        doc.add_with(
            Entity::Polyline {
                points: vec![Point::new(0.0, 0.0), Point::new(1.0, 0.0)],
                closed: false,
                bulges: vec![0.5, 0.0],
                widths: vec![(0.2, 0.2), (0.0, 0.0)],
            },
            Attributes {
                color: Attr::Set(Color::Rgb(1, 2, 3)),
                lineweight: Attr::Set(Lineweight(71)),
                ..Attributes::on_layer(LayerId::DEFAULT)
            },
        );
        let pairs = pairs(&write(&doc).unwrap().bytes);
        let (_, poly) = object(&pairs, "LWPOLYLINE", 0).unwrap();
        assert_eq!(value(poly, 420), Some("66051"), "true colour 0x010203");
        assert_eq!(
            value(poly, 370),
            Some("70"),
            "the nearest standard lineweight"
        );
        assert_eq!(value(poly, 42), Some("0.5"));
        assert_eq!(value(poly, 40), Some("0.2"));
    }

    #[test]
    fn what_we_write_reads_back_with_its_layouts() {
        let original = drawing();
        let back = crate::dxf::read::read(&write(&original).unwrap().bytes)
            .unwrap()
            .document;
        let names: Vec<_> = back.layouts().iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["Plan", "Detail"]);
        let (plan, detail) = (&back.layouts()[0], &back.layouts()[1]);
        assert_eq!((plan.width, plan.height), (420.0, 297.0));
        assert_eq!((detail.width, detail.height), (200.0, 150.0));
        // The viewports: scale, place, lock, visibility and the frozen layer.
        let before = &original.layouts()[0].viewports;
        assert_eq!(plan.viewports.len(), 2);
        let walls = back.layers().iter().find(|l| l.name == "Walls").unwrap().id;
        for (a, b) in before.iter().zip(&plan.viewports) {
            assert_eq!((a.center, a.width, a.height), (b.center, b.width, b.height));
            assert!((a.scale - b.scale).abs() < 1e-9);
            assert_eq!(
                (a.view_center, a.locked, a.on),
                (b.view_center, b.locked, b.on)
            );
        }
        assert_eq!(plan.viewports[0].frozen, [walls]);
        assert!(plan.viewports[1].frozen.is_empty());
        // What was drawn on each sheet, and in the model.
        let sheet = |layout: &Layout| -> Vec<Entity> {
            back.objects_of_layout(layout.id)
                .into_iter()
                .filter_map(|id| back.get(id).cloned())
                .collect()
        };
        assert_eq!(
            sheet(plan),
            [Entity::Line {
                start: Point::new(5.0, 5.0),
                end: Point::new(50.0, 5.0)
            }]
        );
        assert_eq!(
            sheet(detail),
            [Entity::Circle {
                center: Point::new(100.0, 75.0),
                radius: 30.0
            }]
        );
        assert_eq!(back.model_objects().count(), 1);
        assert_eq!(back.validate(), Ok(()));
    }

    #[test]
    fn ellipses_need_r2000_and_come_back_exactly() {
        let mut doc = Document::default();
        let full = Ellipse::full(Point::new(1.0, 2.0), Point::new(3.0, 4.0), 0.25).unwrap();
        let arc =
            Ellipse::new(Point::new(-5.0, 0.0), Point::new(0.0, 2.0), 1e-6, 5.5, 1.0).unwrap();
        doc.add(Entity::Ellipse(full));
        doc.add(Entity::Ellipse(arc));
        let encoded = super::super::write::write(&doc).unwrap();
        assert!(encoded.notes[0].contains("so that ellipses are kept"));
        let bytes = encoded.bytes;
        assert!(String::from_utf8_lossy(&bytes).contains("AC1015"));
        let pairs = pairs(&bytes);
        let (_, first) = object(&pairs, "ELLIPSE", 0).unwrap();
        assert_eq!(value(first, 100), Some("AcDbEntity"));
        assert_eq!(value(first, 40), Some("0.25"));
        let parsed = dxf::Drawing::load(&mut std::io::Cursor::new(&bytes)).expect("a valid DXF");
        let found: Vec<dxf::entities::Ellipse> = parsed
            .entities()
            .filter_map(|e| match &e.specific {
                dxf::entities::EntityType::Ellipse(el) => Some(el.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(found.len(), 2);
        assert_eq!((found[0].center.x, found[0].center.y), (1.0, 2.0));
        assert_eq!((found[0].major_axis.x, found[0].major_axis.y), (3.0, 4.0));
        assert_eq!(found[0].minor_axis_ratio, 0.25);
        assert_eq!(
            (found[1].start_parameter, found[1].end_parameter),
            (arc.start, arc.end)
        );
        let back = crate::dxf::read::read(&bytes).unwrap().document;
        let got: Vec<Entity> = back.model_objects().map(|o| o.entity.clone()).collect();
        assert_eq!(got, [Entity::Ellipse(full), Entity::Ellipse(arc)]);
        // R12 has no ellipse; it is refused rather than approximated.
        assert!(matches!(
            super::super::write::write_r12(&doc),
            Err(FileError::Unsupported(_))
        ));
    }

    #[test]
    fn splines_need_r2000_and_come_back_exactly() {
        let p = Point::new;
        let mut doc = Document::default();
        let fitted = Spline::from_fit_points(
            vec![p(0.0, 0.0), p(2.0, 3.0), p(5.0, 1.0), p(7.5, 2.25)],
            od_core::KnotParameterization::Chord,
        )
        .unwrap();
        let rational = Spline::new_nurbs(
            2,
            vec![p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            Some(vec![1.0, 0.5f64.sqrt(), 1.0]),
            false,
            false,
        )
        .unwrap();
        let line =
            Spline::from_control_points(1, vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)]).unwrap();
        for s in [&fitted, &rational, &line] {
            doc.add(Entity::Spline(Box::new(s.clone())));
        }
        let encoded = super::super::write::write(&doc).unwrap();
        assert!(encoded.notes[0].contains("so that splines are kept"));
        let bytes = encoded.bytes;
        assert!(String::from_utf8_lossy(&bytes).contains("AC1015"));
        let pairs = pairs(&bytes);
        let (at, first) = object(&pairs, "SPLINE", 0).unwrap();
        assert_eq!(value(first, 70), Some("8"), "planar");
        let (at, second) = object(&pairs, "SPLINE", at + 1).unwrap();
        assert_eq!(value(second, 70), Some("12"), "planar and rational");
        let (_, third) = object(&pairs, "SPLINE", at + 1).unwrap();
        assert_eq!(value(third, 70), Some("24"), "planar and linear");
        let parsed = dxf::Drawing::load(&mut std::io::Cursor::new(&bytes)).expect("a valid DXF");
        let found: Vec<dxf::entities::Spline> = parsed
            .entities()
            .filter_map(|e| match &e.specific {
                dxf::entities::EntityType::Spline(s) => Some(s.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(found.len(), 3);
        for (theirs, ours) in found.iter().zip([&fitted, &rational, &line]) {
            assert_eq!(theirs.degree_of_curve, i32::from(ours.degree));
            assert_eq!(theirs.knot_values, ours.knots);
            let points: Vec<Point> = theirs.control_points.iter().map(|q| p(q.x, q.y)).collect();
            assert_eq!(points, ours.control_points);
            assert_eq!(
                theirs.weight_values,
                ours.weights.clone().unwrap_or_default()
            );
        }
        assert_eq!(found[0].fit_points.len(), 4);
        // Our reader gets every value back; DXF has no place for how the
        // knots were spaced.
        let back = crate::dxf::read::read(&bytes).unwrap().document;
        let got: Vec<Entity> = back.model_objects().map(|o| o.entity.clone()).collect();
        let mut expected = fitted.clone();
        if let Some(fit) = &mut expected.fit {
            fit.parameterization = None;
        }
        assert_eq!(
            got,
            [
                Entity::Spline(Box::new(expected)),
                Entity::Spline(Box::new(rational)),
                Entity::Spline(Box::new(line))
            ]
        );
        // R12 has no spline; it is refused rather than approximated.
        assert!(matches!(
            super::super::write::write_r12(&doc),
            Err(FileError::Unsupported(_))
        ));
    }

    #[test]
    fn a_drawing_without_layouts_still_writes_r12() {
        let bytes = super::super::write::write(&Document::default())
            .unwrap()
            .bytes;
        assert!(String::from_utf8_lossy(&bytes).contains("AC1009"));
    }
}
