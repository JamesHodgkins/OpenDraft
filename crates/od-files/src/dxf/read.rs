//! Reading ASCII DXF of any version. Lines, circles, arcs, ellipses, splines, hatches, polylines
//! (plain or lightweight) and blocks with their references come in, with their
//! layers and attributes; other objects are skipped and counted, so the user
//! is told what was left out.

use std::collections::{BTreeMap, HashMap};

use od_core::{
    Attr, Attributes, Color, Document, Entity, Layer, LayerId, LinetypeId, Lineweight, Point,
};

use crate::{FileError, Loaded};
use od_core::aci::aci_rgb;

mod attribute;
mod basic;
mod blocks;
mod dimension;
mod ellipse;
mod hatch;
mod hatch_sources;
#[cfg(test)]
mod hatch_tests;
mod infinite;
mod layouts;
mod leader;
mod ltype;
mod mleader;
mod polyline;
mod spline;
mod text;

/// One group: a code and its value.
#[derive(Clone)]
struct Group {
    code: i32,
    value: String,
}

/// A record: a group with code 0 (its name) and the groups that follow it.
#[derive(Clone)]
struct Record {
    name: String,
    groups: Vec<Group>,
}

impl Record {
    fn text(&self, code: i32) -> Option<&str> {
        self.groups
            .iter()
            .find(|g| g.code == code)
            .map(|g| g.value.as_str())
    }

    fn int(&self, code: i32) -> Option<i64> {
        self.text(code)?.parse().ok()
    }

    fn number(&self, code: i32) -> Option<f64> {
        self.text(code)?
            .parse()
            .ok()
            .filter(|v: &f64| v.is_finite())
    }
}

/// Text from the file, with `\U+XXXX` escapes (how old DXF writes characters
/// outside ASCII) turned into the characters.
fn decode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("\\U+") {
        out.push_str(&rest[..at]);
        let hex = rest.get(at + 3..at + 7);
        match hex
            .and_then(|h| u32::from_str_radix(h, 16).ok())
            .and_then(char::from_u32)
        {
            Some(c) => {
                out.push(c);
                rest = &rest[at + 7..];
            }
            None => {
                out.push_str("\\U+");
                rest = &rest[at + 3..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn records(bytes: &[u8]) -> Result<Vec<Record>, FileError> {
    if bytes.starts_with(b"AutoCAD Binary DXF") {
        return Err(FileError::Unreadable(
            "this is a binary DXF; save it as an ASCII DXF first".into(),
        ));
    }
    // Files before AutoCAD 2007 use a code page, not UTF-8; reading each byte
    // as one character is right for plain text and close for the rest.
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => bytes.iter().map(|&b| char::from(b)).collect(),
    };
    let mut lines = text.lines();
    let mut records: Vec<Record> = Vec::new();
    let mut number = 0;
    while let (Some(code), Some(value)) = (lines.next(), lines.next()) {
        number += 2;
        let code: i32 = code.trim().parse().map_err(|_| {
            FileError::Unreadable(format!("line {}: expected a group code", number - 1))
        })?;
        let value = text::value_of(records.last().map_or("", |r| &r.name), code, value);
        if code == 0 {
            records.push(Record {
                name: value,
                groups: Vec::new(),
            });
        } else if let Some(record) = records.last_mut() {
            record.groups.push(Group { code, value });
        }
        // Groups before the first record (comments, code 999) are ignored.
    }
    Ok(records)
}

/// The colour an AutoCAD index stands for in this app.
fn aci_color(index: i64) -> Color {
    match index {
        1..=255 if index != 7 => {
            let (r, g, b) = aci_rgb(index as u8);
            Color::Rgb(r, g, b)
        }
        _ => Color::Foreground,
    }
}

fn true_color(value: i64) -> Color {
    Color::Rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

#[derive(Default)]
struct Importer {
    document: Document,
    /// Layer ids by lower-cased name.
    layers: HashMap<String, LayerId>,
    skipped: BTreeMap<String, usize>,
    damaged: usize,
    paper_space: usize,
    imported: usize,
    blocks: blocks::BlockReading,
    ltypes: ltype::Reading,
    spline_budget: usize,
    hatch_budget: usize,
    /// The object each handle in the file named, for hatch sources.
    handles: HashMap<String, od_core::EntityId>,
    /// What multileader styles the file named.
    mleaders: mleader::Styles,
    text_styles: text::Styles,
    leaders: leader::Leaders,
    dim_styles: dimension::Styles,
}

impl Importer {
    fn new() -> Self {
        let mut importer = Self::default();
        importer.document.begin_bulk_load();
        // The drawing starts with layer 0; entities on "0" use it.
        importer.layers.insert("0".to_owned(), LayerId::DEFAULT);
        importer.spline_budget = spline::MAX_FILE_SPLINE_SCALARS;
        importer.hatch_budget = hatch::MAX_FILE_HATCH_GROUPS;
        importer
    }

    fn layer_named(&mut self, name: &str) -> LayerId {
        let key = name.to_lowercase();
        if let Some(&id) = self.layers.get(&key) {
            return id;
        }
        let id = self.document.allocate_layer_id();
        self.document.push_layer(Layer::new(id, name));
        self.layers.insert(key, id);
        id
    }

    fn layer_record(&mut self, record: &Record) {
        let name = decode(record.text(2).unwrap_or("0"));
        let flags = record.int(70).unwrap_or(0);
        let index = record.int(62).unwrap_or(7);
        let mut color = match record.int(420) {
            Some(value) => true_color(value),
            None => aci_color(index.abs()),
        };
        if index == 0 {
            color = Color::Foreground;
        }
        let lineweight = match record.int(370) {
            Some(w) if w >= 0 => Lineweight(u16::try_from(w).unwrap_or(u16::MAX)),
            _ => Lineweight::DEFAULT,
        };
        let id = if name == "0" {
            LayerId::DEFAULT
        } else {
            self.layer_named(&name)
        };
        let layer = Layer {
            id,
            name: if id == LayerId::DEFAULT {
                "0".to_owned()
            } else {
                name
            },
            color,
            linetype: self
                .linetype_ref(record.text(6))
                .unwrap_or(LinetypeId::CONTINUOUS),
            lineweight,
            on: index >= 0,
            frozen: flags & 1 != 0,
            locked: flags & 4 != 0,
        };
        self.layers.insert(layer.name.to_lowercase(), id);
        self.document.replace_layer(layer);
    }

    fn attributes(&mut self, record: &Record) -> Attributes {
        let layer = self.layer_named(&decode(record.text(8).unwrap_or("0")));
        let color = match (record.int(420), record.int(62)) {
            (Some(value), _) => Attr::Set(true_color(value)),
            (None, Some(0)) => Attr::ByBlock,
            (None, Some(index)) if (1..=255).contains(&index.abs()) => {
                Attr::Set(aci_color(index.abs()))
            }
            _ => Attr::ByLayer,
        };
        let linetype = if record
            .text(6)
            .is_some_and(|n| n.eq_ignore_ascii_case("byblock"))
        {
            Attr::ByBlock
        } else {
            self.linetype_ref(record.text(6))
                .map_or(Attr::ByLayer, Attr::Set)
        };
        let lineweight = match record.int(370) {
            Some(w) if w >= 0 => Attr::Set(Lineweight(u16::try_from(w).unwrap_or(u16::MAX))),
            Some(-2) => Attr::ByBlock,
            _ => Attr::ByLayer,
        };
        let (linetype_scale, continuous_generation) =
            super::linetypes::read_entity(&record.name, record.number(48), record.int(70));
        Attributes {
            layer,
            color,
            linetype,
            lineweight,
            linetype_scale,
            continuous_generation,
        }
    }

    fn add(&mut self, record: &Record, entity: Entity) {
        self.add_item(record, crate::block_import::Item::Entity(entity));
    }

    /// Reads a drawing object into the drawing, or into the block being
    /// read. An old-style POLYLINE is collected in `open` with its VERTEX
    /// records until its SEQEND.
    fn shape<'a>(
        &mut self,
        record: &'a Record,
        open: &mut Option<(&'a Record, Vec<polyline::Vertex>)>,
    ) {
        match record.name.as_str() {
            _ if self.attribute_record(record) => {}
            "VERTEX" => {
                if let Some((_, vertices)) = open.as_mut() {
                    // Spline control points (flag 16) are not on the curve.
                    let on_curve = record.int(70).unwrap_or(0) & 16 == 0;
                    if let (true, Some(vertex)) = (on_curve, polyline::vertex(record)) {
                        vertices.push(vertex);
                    }
                }
            }
            "SEQEND" => {
                if let Some((header, vertices)) = open.take() {
                    let closed = header.int(70).unwrap_or(0) & 1 != 0;
                    let default = polyline::header_widths(header);
                    self.polyline(header, &vertices, default, closed);
                }
            }
            "LINE" | "CIRCLE" | "ARC" | "POINT" => self.basic(record),
            "XLINE" | "RAY" => self.infinite_line(record),
            "ELLIPSE" => self.ellipse(record),
            "SPLINE" => self.spline(record),
            "HATCH" => self.hatch(record),
            "MTEXT" => self.mtext(record),
            "TEXT" => self.text(record),
            "LEADER" => self.leader(record),
            "MULTILEADER" => self.multileader(record),
            "DIMENSION" => self.dimension(record),
            "INSERT" => self.insert(record),
            "LWPOLYLINE" => {
                let (vertices, constant) = polyline::lw_vertices(record);
                let closed = record.int(70).unwrap_or(0) & 1 != 0;
                self.polyline(record, &vertices, (constant, constant), closed);
            }
            "POLYLINE" => {
                // Meshes and polyface meshes are surfaces, not 2D lines.
                if record.int(70).unwrap_or(0) & (16 | 32 | 64) != 0 {
                    self.skip("POLYLINE (mesh)");
                } else {
                    *open = Some((record, Vec::new()));
                }
            }
            other => self.skip(other),
        }
    }

    fn point(record: &Record, x: i32, y: i32) -> Option<Point> {
        Some(Point::new(record.number(x)?, record.number(y)?))
    }

    fn skip(&mut self, name: &str) {
        *self.skipped.entry(name.to_owned()).or_insert(0) += 1;
    }

    fn warnings(&self) -> Vec<String> {
        let mut warnings = self.ltypes.warnings();
        if !self.skipped.is_empty() {
            let total: usize = self.skipped.values().sum();
            let kinds: Vec<String> = self
                .skipped
                .iter()
                .map(|(k, n)| format!("{n} {k}"))
                .collect();
            warnings.push(format!(
                "Skipped {total} unsupported object{}: {}.",
                if total == 1 { "" } else { "s" },
                kinds.join(", ")
            ));
        }
        if self.paper_space > 0 {
            warnings.push(format!("Skipped {} paper space objects.", self.paper_space));
        }
        if self.damaged > 0 {
            warnings.push(format!("Skipped {} damaged objects.", self.damaged));
        }
        warnings.extend(self.blocks.warnings());
        warnings
    }
}

pub fn read(bytes: &[u8]) -> Result<Loaded, FileError> {
    let records = records(bytes)?;
    if !records.iter().any(|r| r.name == "SECTION") {
        return Err(FileError::Unreadable("not a DXF file".into()));
    }
    let mut importer = Importer::new();
    let mut spaces = layouts::Spaces::scan(&records, &mut importer.document);
    importer.scan_styles(&records);
    let mut section = String::new();
    let mut table = String::new();
    // The block being read in BLOCKS (only a sheet's own is of interest).
    let mut block = String::new();
    // A POLYLINE being collected, with its vertices.
    let mut open_polyline: Option<(&Record, Vec<polyline::Vertex>)> = None;

    for (i, record) in records.iter().enumerate() {
        match record.name.as_str() {
            "SECTION" => {
                section = record.text(2).unwrap_or("").to_owned();
                if section == "BLOCKS" {
                    importer.read_blocks(&records[i + 1..], &spaces);
                }
                if section == "HEADER" {
                    let groups = record.groups.iter().map(|g| (g.code, g.value.as_str()));
                    super::header::apply(groups, &mut importer.document);
                }
            }
            "ENDSEC" => section.clear(),
            "BLOCK" if section == "BLOCKS" => block = record.text(2).unwrap_or("").to_owned(),
            "ENDBLK" => block.clear(),
            "TABLE" => table = record.text(2).unwrap_or("").to_owned(),
            "ENDTAB" => table.clear(),
            "LAYER" | "LTYPE" | "BLOCK_RECORD" => importer.table_record(record, &section, &table),
            _ if section != "ENTITIES"
                && !(section == "BLOCKS" && spaces.is_sheet_block(&block)) => {}
            "VERTEX" | "SEQEND" => importer.shape(record, &mut open_polyline),
            name => {
                // Entities go to model space or to a layout's sheet.
                match spaces.space_of(record, &block) {
                    layouts::Space::Skip => {
                        importer.paper_space += 1;
                        continue;
                    }
                    space => importer.document.set_active_layout(space.layout()),
                }
                match name {
                    "VIEWPORT" => match importer.document.active_layout() {
                        Some(layout) => {
                            spaces.viewport(record, layout, &mut importer.document);
                        }
                        None => importer.skip("VIEWPORT"),
                    },
                    _ => importer.shape(record, &mut open_polyline),
                }
            }
        }
    }
    importer.drop_open_insert();
    importer.resolve_hatch_sources();
    importer.resolve_leaders();
    importer.resolve_linetypes();
    let mut warnings = importer.warnings();
    warnings.extend(spaces.warnings());
    spaces.finish(&mut importer);
    importer.document.finish_bulk_load();
    Ok(Loaded {
        warnings,
        document: importer.document,
        imported: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a DXF text from (code, value) pairs.
    fn dxf(pairs: &[(i32, &str)]) -> Vec<u8> {
        let mut text = String::new();
        for (code, value) in pairs {
            text.push_str(&format!("{code:>3}\r\n{value}\r\n"));
        }
        text.into_bytes()
    }

    /// A file with the given ENTITIES (and optional LAYER table records).
    fn file(layers: &[(i32, &str)], entities: &[(i32, &str)]) -> Vec<u8> {
        let mut pairs: Vec<(i32, &str)> = vec![(0, "SECTION"), (2, "HEADER"), (0, "ENDSEC")];
        if !layers.is_empty() {
            pairs.extend([(0, "SECTION"), (2, "TABLES"), (0, "TABLE"), (2, "LAYER")]);
            pairs.extend_from_slice(layers);
            pairs.extend([(0, "ENDTAB"), (0, "ENDSEC")]);
        }
        pairs.extend([(0, "SECTION"), (2, "ENTITIES")]);
        pairs.extend_from_slice(entities);
        pairs.extend([(0, "ENDSEC"), (0, "EOF")]);
        dxf(&pairs)
    }

    fn load(layers: &[(i32, &str)], entities: &[(i32, &str)]) -> Loaded {
        read(&file(layers, entities)).unwrap()
    }

    #[test]
    fn a_donut_of_two_vertices_and_two_arcs_comes_in_as_a_whole_ring() {
        let loaded = load(
            &[],
            &[
                (0, "LWPOLYLINE"),
                (8, "0"),
                (90, "2"),
                (70, "1"),
                (43, "2"),
                (10, "1"),
                (20, "0"),
                (42, "1"),
                (10, "-1"),
                (20, "0"),
                (42, "1"),
            ],
        );
        let Some(Entity::Polyline { points, bulges, .. }) =
            loaded.document.objects().first().map(|o| o.entity.clone())
        else {
            panic!("a polyline");
        };
        assert_eq!(points.len(), 4);
        let round = od_core::bulge::path_length(&points, &bulges, true);
        assert!((round - std::f64::consts::TAU).abs() < 1e-9, "{round}");
    }

    #[test]
    fn lines_and_circles_come_in_on_their_layers() {
        let loaded = load(
            &[
                (0, "LAYER"),
                (2, "Walls"),
                (70, "0"),
                (62, "1"),
                (6, "DASHED"),
                (370, "50"),
            ],
            &[
                (0, "LINE"),
                (8, "Walls"),
                (10, "0"),
                (20, "0"),
                (30, "0"),
                (11, "3"),
                (21, "4"),
                (31, "0"),
                (0, "CIRCLE"),
                (8, "Walls"),
                (10, "1"),
                (20, "2"),
                (40, "5"),
            ],
        );
        assert!(loaded.imported && loaded.warnings.is_empty());
        let doc = &loaded.document;
        assert_eq!(doc.len(), 2);
        let walls = doc.layers().iter().find(|l| l.name == "Walls").unwrap();
        assert_eq!(walls.color, Color::Rgb(255, 0, 0));
        assert_eq!(walls.linetype, od_core::LinetypeId::DASHED);
        assert_eq!(walls.lineweight, Lineweight(50));
        assert_eq!(doc.objects()[0].attrs.layer, walls.id);
        assert_eq!(
            doc.objects()[0].entity,
            Entity::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(3.0, 4.0)
            }
        );
        assert_eq!(
            doc.objects()[1].entity,
            Entity::Circle {
                center: Point::new(1.0, 2.0),
                radius: 5.0
            }
        );
    }

    #[test]
    fn layer_flags_colours_and_off_layers() {
        let loaded = load(
            &[
                (0, "LAYER"),
                (2, "Frozen"),
                (70, "1"),
                (62, "7"),
                (0, "LAYER"),
                (2, "Locked"),
                (70, "4"),
                (62, "3"),
                (0, "LAYER"),
                (2, "Off"),
                (70, "0"),
                (62, "-5"),
                (0, "LAYER"),
                (2, "True"),
                (70, "0"),
                (62, "7"),
                (420, "16711935"),
            ],
            &[],
        );
        let layer = |name: &str| {
            loaded
                .document
                .layers()
                .iter()
                .find(|l| l.name == name)
                .unwrap()
                .clone()
        };
        assert!(layer("Frozen").frozen && layer("Frozen").color == Color::Foreground);
        assert!(layer("Locked").locked);
        assert!(!layer("Off").on && layer("Off").color == Color::Rgb(0, 0, 255));
        assert_eq!(
            layer("True").color,
            Color::Rgb(255, 0, 255),
            "true colour wins"
        );
    }

    #[test]
    fn entity_attributes_by_layer_or_set() {
        let loaded = load(
            &[],
            &[
                (0, "LINE"),
                (8, "0"),
                (10, "0"),
                (20, "0"),
                (11, "1"),
                (21, "1"),
                (0, "LINE"),
                (8, "0"),
                (62, "5"),
                (6, "CENTER"),
                (370, "35"),
                (10, "0"),
                (20, "0"),
                (11, "1"),
                (21, "1"),
                (0, "LINE"),
                (8, "0"),
                (62, "256"),
                (6, "BYLAYER"),
                (370, "-2"),
                (10, "0"),
                (20, "0"),
                (11, "1"),
                (21, "1"),
            ],
        );
        let objects = loaded.document.objects();
        assert_eq!(objects[0].attrs, Attributes::on_layer(LayerId::DEFAULT));
        assert_eq!(objects[1].attrs.color, Attr::Set(Color::Rgb(0, 0, 255)));
        assert_eq!(
            objects[1].attrs.linetype,
            Attr::Set(od_core::LinetypeId::CENTER)
        );
        assert_eq!(objects[1].attrs.lineweight, Attr::Set(Lineweight(35)));
        // Colour 256 and BYLAYER are by layer; lineweight -2 is by block.
        assert_eq!(objects[2].attrs.color, Attr::ByLayer);
        assert_eq!(objects[2].attrs.linetype, Attr::ByLayer);
        assert_eq!(objects[2].attrs.lineweight, Attr::ByBlock);
    }

    #[test]
    fn a_layer_used_but_not_declared_is_created() {
        let loaded = load(
            &[],
            &[
                (0, "LINE"),
                (8, "Ghost"),
                (10, "0"),
                (20, "0"),
                (11, "1"),
                (21, "1"),
            ],
        );
        let doc = &loaded.document;
        assert_eq!(doc.layers().len(), 2);
        assert_eq!(
            doc.layer(doc.objects()[0].attrs.layer).unwrap().name,
            "Ghost"
        );
    }

    #[test]
    fn layer_names_match_regardless_of_case() {
        let loaded = load(
            &[(0, "LAYER"), (2, "Walls"), (70, "0"), (62, "1")],
            &[
                (0, "LINE"),
                (8, "WALLS"),
                (10, "0"),
                (20, "0"),
                (11, "1"),
                (21, "1"),
            ],
        );
        assert_eq!(loaded.document.layers().len(), 2, "no second Walls layer");
    }

    #[test]
    fn arcs_are_read_as_arcs_with_their_angles_in_radians() {
        let loaded = load(
            &[],
            &[
                (0, "ARC"),
                (8, "0"),
                (10, "0"),
                (20, "0"),
                (40, "10"),
                (50, "0"),
                (51, "90"),
            ],
        );
        let Entity::Arc {
            center,
            radius,
            start,
            end,
        } = loaded.document.objects()[0].entity
        else {
            panic!("an arc should stay an arc");
        };
        assert_eq!((center, radius), (Point::new(0.0, 0.0), 10.0));
        assert!(start.abs() < 1e-12 && (end - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    }

    #[test]
    fn polyline_widths_come_from_vertices_headers_and_the_constant_width() {
        let loaded = load(
            &[],
            &[
                // Lightweight: a constant width of 2, one segment overriding it.
                (0, "LWPOLYLINE"),
                (8, "0"),
                (90, "3"),
                (70, "0"),
                (43, "2"),
                (10, "0"),
                (20, "0"),
                (40, "1"),
                (41, "3"),
                (10, "4"),
                (20, "0"),
                (10, "4"),
                (20, "3"),
                // Old style: header widths are the default, a vertex overrides.
                (0, "POLYLINE"),
                (8, "0"),
                (66, "1"),
                (40, "0.5"),
                (41, "0.5"),
                (70, "0"),
                (0, "VERTEX"),
                (8, "0"),
                (10, "0"),
                (20, "0"),
                (0, "VERTEX"),
                (8, "0"),
                (10, "1"),
                (20, "0"),
                (40, "0"),
                (41, "2"),
                (0, "VERTEX"),
                (8, "0"),
                (10, "1"),
                (20, "1"),
                (0, "SEQEND"),
            ],
        );
        let objects = loaded.document.objects();
        let widths = |i: usize| match &objects[i].entity {
            Entity::Polyline { widths, .. } => widths.clone(),
            other => panic!("{other:?}"),
        };
        // The last vertex of an open polyline starts no segment.
        assert_eq!(widths(0), vec![(1.0, 3.0), (2.0, 2.0), (0.0, 0.0)]);
        assert_eq!(widths(1), vec![(0.5, 0.5), (0.0, 2.0), (0.0, 0.0)]);
    }

    #[test]
    fn lightweight_polylines_with_and_without_bulges() {
        let loaded = load(
            &[],
            &[
                (0, "LWPOLYLINE"),
                (8, "0"),
                (90, "3"),
                (70, "1"),
                (10, "0"),
                (20, "0"),
                (10, "4"),
                (20, "0"),
                (10, "4"),
                (20, "3"),
                (0, "LWPOLYLINE"),
                (8, "0"),
                (90, "2"),
                (70, "0"),
                (10, "0"),
                (20, "0"),
                (10, "1"),
                (20, "1"),
                (0, "LWPOLYLINE"),
                (8, "0"),
                (90, "2"),
                (70, "0"),
                (10, "0"),
                (20, "0"),
                (42, "1"),
                (10, "2"),
                (20, "0"),
            ],
        );
        let objects = loaded.document.objects();
        assert_eq!(
            objects[0].entity,
            Entity::Polyline {
                points: vec![
                    Point::new(0.0, 0.0),
                    Point::new(4.0, 0.0),
                    Point::new(4.0, 3.0)
                ],
                closed: true,
                bulges: Vec::new(),
                widths: Vec::new(),
            }
        );
        assert_eq!(
            objects[1].entity,
            Entity::Line {
                start: Point::new(0.0, 0.0),
                end: Point::new(1.0, 1.0)
            },
            "a two-point open polyline is a line"
        );
        // The bulge of 1 stays a bulge: a half circle, not short straight pieces.
        assert_eq!(
            objects[2].entity,
            Entity::Polyline {
                points: vec![Point::new(0.0, 0.0), Point::new(2.0, 0.0)],
                closed: false,
                bulges: vec![1.0, 0.0],
                widths: Vec::new(),
            }
        );
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    }

    #[test]
    fn old_style_polylines_collect_their_vertices() {
        let loaded = load(
            &[],
            &[
                (0, "POLYLINE"),
                (8, "0"),
                (66, "1"),
                (70, "1"),
                (0, "VERTEX"),
                (8, "0"),
                (10, "0"),
                (20, "0"),
                (0, "VERTEX"),
                (8, "0"),
                (10, "5"),
                (20, "0"),
                (0, "VERTEX"),
                (8, "0"),
                (10, "5"),
                (20, "5"),
                (0, "SEQEND"),
                (8, "0"),
            ],
        );
        assert_eq!(
            loaded.document.objects()[0].entity,
            Entity::Polyline {
                points: vec![
                    Point::new(0.0, 0.0),
                    Point::new(5.0, 0.0),
                    Point::new(5.0, 5.0)
                ],
                closed: true,
                bulges: Vec::new(),
                widths: Vec::new(),
            }
        );
    }

    #[test]
    fn unsupported_things_are_counted_not_loaded() {
        let loaded = load(
            &[],
            &[
                (0, "3DFACE"),
                (8, "0"),
                (10, "0"),
                (20, "0"),
                (1, "hi"),
                (0, "3DFACE"),
                (8, "0"),
                (10, "0"),
                (20, "0"),
                (1, "there"),
                (0, "INSERT"),
                (8, "0"),
                (2, "BLOCK1"),
                (10, "0"),
                (20, "0"),
                (0, "LINE"),
                (8, "0"),
                (10, "0"),
                (20, "0"),
                (11, "1"),
                (21, "1"),
            ],
        );
        assert_eq!(loaded.document.len(), 1);
        assert_eq!(
            loaded.warnings,
            [
                "Skipped 2 unsupported objects: 2 3DFACE.",
                "Skipped 1 block reference to blocks that are not in the file."
            ]
        );
    }

    #[test]
    fn paper_space_and_damaged_objects_are_reported() {
        let loaded = load(
            &[],
            &[
                (0, "LINE"),
                (8, "0"),
                (67, "1"),
                (10, "0"),
                (20, "0"),
                (11, "1"),
                (21, "1"),
                (0, "CIRCLE"),
                (8, "0"),
                (10, "0"),
                (20, "0"),
                (40, "0"),
                (0, "LINE"),
                (8, "0"),
                (10, "oops"),
                (20, "0"),
                (11, "1"),
                (21, "1"),
            ],
        );
        // The paper-space line (an old file's one unnamed paper space) becomes
        // a sheet object on an implicit "Layout1"; the model gets nothing.
        assert_eq!(loaded.document.model_objects().count(), 0);
        let layout = &loaded.document.layouts()[0];
        assert_eq!(layout.name, "Layout1");
        assert_eq!(loaded.document.objects_of_layout(layout.id).len(), 1);
        assert!(loaded.warnings.iter().any(|w| w.contains("2 damaged")));
    }

    #[test]
    fn unicode_escapes_in_names_are_decoded() {
        let loaded = load(
            &[(0, "LAYER"), (2, "Caf\\U+00E9"), (70, "0"), (62, "1")],
            &[],
        );
        assert!(loaded.document.layers().iter().any(|l| l.name == "Café"));
        assert_eq!(decode("a\\U+zzzzb"), "a\\U+zzzzb");
    }

    #[test]
    fn non_dxf_and_binary_files_are_refused() {
        assert!(matches!(read(b"hello"), Err(FileError::Unreadable(_))));
        assert!(matches!(read(b""), Err(FileError::Unreadable(_))));
        assert!(matches!(
            read(b"AutoCAD Binary DXF\r\n\x1a\0"),
            Err(FileError::Unreadable(why)) if why.contains("binary")
        ));
        assert!(matches!(
            read(&dxf(&[(0, "LINE")])),
            Err(FileError::Unreadable(_))
        ));
    }

    #[test]
    fn an_old_code_page_file_still_reads() {
        // Layer "Caf\xe9" in Latin-1: not valid UTF-8.
        let mut bytes = b"  0\r\nSECTION\r\n  2\r\nTABLES\r\n  0\r\nTABLE\r\n  2\r\nLAYER\r\n  0\r\nLAYER\r\n  2\r\nCaf".to_vec();
        bytes.push(0xe9);
        bytes.extend(
            b"\r\n 70\r\n0\r\n 62\r\n1\r\n  0\r\nENDTAB\r\n  0\r\nENDSEC\r\n  0\r\nEOF\r\n",
        );
        let loaded = read(&bytes).unwrap();
        assert!(loaded
            .document
            .layers()
            .iter()
            .any(|l| l.name == "Caf\u{e9}"));
    }
}
