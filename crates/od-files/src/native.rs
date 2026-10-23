//! The native format: a JSON file, extension `.odx`.
//!
//! The file has its own schema (the `*Dto` types here), not the in-memory
//! types serialised as they happen to be, so refactoring the app cannot
//! silently change what is on disk. Rules for changing it:
//!
//! - Adding an optional field does not need a new version: old readers ignore
//!   it and files without it still read the same.
//! - A new kind of entity DOES bump [`CURRENT_VERSION`]. A reader never skips
//!   an object it does not understand, because saving the file again would
//!   delete that object from the user's drawing. Instead it refuses the file
//!   as saved by a newer version.
//! - Anything else bumps [`CURRENT_VERSION`] too.
//!
//! Only the current version is read: until the first release there are no
//! files to keep working, so an older one is refused rather than converted.
//! From the first release on, a conversion for each older version goes in
//! `read`.
//!
//! A file is read in one pass, bounded by [`MAX_BYTES`], and checked with
//! [`Document::validate`] before it is handed over.

use od_core::{Attr, Color, Document};
use serde::{Deserialize, Serialize};

use crate::{Encoded, FileError, FileFormat, Loaded};

mod block;
mod dimension;
mod dimension_style;
mod entity;
mod hatch;
mod hatch_fill;
mod layout;
mod leader;
mod linetype;
mod load;
mod mleader;
mod mleader_style;
mod new_style;
mod point_style;
pub(crate) mod spline;
mod text;
mod units;

use block::BlockDto;
use dimension::{sparse_style, sparse_table};
use entity::EntityDto;
use layout::LayoutDto;
use text::TextStyleDto;
use units::UnitsDto;

/// The file format version this build writes and reads. Versions before the
/// first release are not kept: see the module notes.
pub const CURRENT_VERSION: u32 = 3;
/// The largest `.odx` file that is opened. Reading holds the whole file and the
/// drawing built from it in memory at once, so a file past this is refused
/// before it is read.
const MAX_BYTES: u64 = 512 << 20;
pub const NATIVE_EXTENSION: &str = "odx";
/// Marks a file as one of ours, whatever its extension.
const MAGIC: &str = "opendraft-drawing";

pub struct NativeFormat;

impl FileFormat for NativeFormat {
    fn name(&self) -> &'static str {
        "OpenDraft drawing"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[NATIVE_EXTENSION]
    }

    fn check_size(&self, len: u64) -> Result<(), FileError> {
        if len > MAX_BYTES {
            return Err(FileError::Unsupported(format!(
                "this drawing is over the {} MB limit",
                MAX_BYTES >> 20
            )));
        }
        Ok(())
    }

    fn read(&self, bytes: &[u8]) -> Result<Loaded, FileError> {
        let file: FileDto = match serde_json::from_slice(bytes) {
            Ok(file) => file,
            Err(e) => return Err(unreadable(bytes, &e)),
        };
        if file.format != MAGIC {
            return Err(FileError::Unreadable("not an OpenDraft drawing".into()));
        }
        check_version(file.version)?;
        let document = file.drawing.into_document()?;
        document
            .validate()
            .map_err(|e| FileError::Invalid(e.to_string()))?;
        Ok(Loaded {
            document,
            warnings: Vec::new(),
            imported: false,
        })
    }

    fn write(&self, document: &Document) -> Result<Encoded, FileError> {
        document
            .validate()
            .map_err(|e| FileError::Invalid(format!("the drawing is inconsistent: {e}")))?;
        let file = FileDto {
            format: MAGIC.to_owned(),
            version: CURRENT_VERSION,
            saved_by: concat!("OpenDraft ", env!("CARGO_PKG_VERSION")).to_owned(),
            drawing: DrawingDto::from_document(document)?,
        };
        let mut bytes =
            serde_json::to_vec_pretty(&file).map_err(|e| FileError::Invalid(e.to_string()))?;
        bytes.push(b'\n');
        Ok(Encoded {
            bytes,
            notes: Vec::new(),
        })
    }
}

/// Just the start of a file, read only to say why the whole of it would not.
#[derive(Deserialize)]
struct Header {
    format: String,
    version: u32,
}

fn check_version(found: u32) -> Result<(), FileError> {
    match found.cmp(&CURRENT_VERSION) {
        std::cmp::Ordering::Equal => Ok(()),
        std::cmp::Ordering::Greater => Err(FileError::TooNew {
            found,
            supported: CURRENT_VERSION,
        }),
        std::cmp::Ordering::Less => Err(FileError::TooOld {
            found,
            supported: CURRENT_VERSION,
        }),
    }
}

/// The error for bytes that did not read as a drawing. A file from another
/// version has content this one cannot parse, so its header says what is
/// wrong before the parse error does. (Only here, after a failure: a good
/// file is read once.)
fn unreadable(bytes: &[u8], parse: &serde_json::Error) -> FileError {
    match serde_json::from_slice::<Header>(bytes) {
        Ok(header) if header.format != MAGIC => {
            FileError::Unreadable("not an OpenDraft drawing".into())
        }
        Ok(header) => match check_version(header.version) {
            Err(error) => error,
            Ok(()) => FileError::Unreadable(parse.to_string()),
        },
        Err(_) => FileError::Unreadable(parse.to_string()),
    }
}

#[derive(Serialize, Deserialize)]
struct FileDto {
    format: String,
    version: u32,
    #[serde(default)]
    saved_by: String,
    drawing: DrawingDto,
}

#[derive(Serialize, Deserialize)]
struct DrawingDto {
    /// Counters, so ids of deleted objects and layers are never reused.
    next_object_id: u64,
    next_layer_id: u64,
    current_layer: u64,
    #[serde(default)]
    units: UnitsDto,
    /// The current dimension style's differences from the stock one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dim_style: Option<serde_json::Value>,
    /// The named dimension styles, each as its differences from the stock
    /// style; left out when `Standard` is all there is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dim_styles: Option<Vec<serde_json::Value>>,
    layers: Vec<LayerDto>,
    /// In draw order.
    objects: Vec<ObjectDto>,
    /// Sheets in tab order; counters keep deleted ids retired.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    layouts: Vec<LayoutDto>,
    #[serde(default)]
    next_layout_id: u64,
    #[serde(default)]
    next_viewport_id: u64,
    /// Left out for the stock table (just `Standard`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    text_styles: Vec<TextStyleDto>,
    #[serde(default)]
    current_text_style: u64,
    #[serde(default)]
    next_text_style_id: u64,
    /// The multileader styles; left out for the stock table.
    #[serde(flatten)]
    mleader_styles: mleader_style::TableDto,
    /// Block definitions in table order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    blocks: Vec<BlockDto>,
    #[serde(default)]
    next_block_id: u64,
    /// The linetype table and `LTSCALE`; each is left out at its stock value.
    #[serde(flatten)]
    linetypes: linetype::TableDto,
    #[serde(flatten)]
    new_style: new_style::NewStyleDto,
    /// `PDMODE` and `PDSIZE`; each is left out at its default.
    #[serde(flatten)]
    point_style: point_style::PointStyleDto,
}

#[derive(Serialize, Deserialize)]
struct LayerDto {
    id: u64,
    name: String,
    color: ColorDto,
    linetype: String,
    /// Hundredths of a millimetre.
    lineweight: u16,
    #[serde(default = "yes")]
    on: bool,
    #[serde(default)]
    frozen: bool,
    #[serde(default)]
    locked: bool,
}

fn yes() -> bool {
    true
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ColorDto {
    Foreground,
    Rgb([u8; 3]),
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum AttrDto<T> {
    #[default]
    ByLayer,
    Set(T),
}

#[derive(Serialize, Deserialize)]
struct ObjectDto {
    id: u64,
    layer: u64,
    #[serde(default)]
    color: AttrDto<ColorDto>,
    #[serde(default)]
    linetype: AttrDto<String>,
    #[serde(default)]
    lineweight: AttrDto<u16>,
    #[serde(flatten)]
    scale: linetype::ScaleDto,
    /// The layout whose sheet it is on; absent for model space.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    layout: Option<u64>,
    entity: EntityDto,
}

impl From<Color> for ColorDto {
    fn from(color: Color) -> Self {
        match color {
            Color::Foreground => ColorDto::Foreground,
            Color::Rgb(r, g, b) => ColorDto::Rgb([r, g, b]),
        }
    }
}

impl From<ColorDto> for Color {
    fn from(color: ColorDto) -> Self {
        match color {
            ColorDto::Foreground => Color::Foreground,
            ColorDto::Rgb([r, g, b]) => Color::Rgb(r, g, b),
        }
    }
}

fn attr_to_dto<T, U>(attr: &Attr<T>, convert: impl Fn(&T) -> U) -> AttrDto<U> {
    match attr {
        Attr::ByLayer => AttrDto::ByLayer,
        Attr::Set(value) => AttrDto::Set(convert(value)),
    }
}

fn attr_from_dto<T, U>(attr: AttrDto<T>, convert: impl Fn(T) -> U) -> Attr<U> {
    match attr {
        AttrDto::ByLayer => Attr::ByLayer,
        AttrDto::Set(value) => Attr::Set(convert(value)),
    }
}

impl DrawingDto {
    fn from_document(document: &Document) -> Result<Self, FileError> {
        let layers = document
            .layers()
            .iter()
            .map(|l| LayerDto {
                id: l.id.0,
                name: l.name.clone(),
                color: l.color.into(),
                linetype: l.linetype.clone(),
                lineweight: l.lineweight.0,
                on: l.on,
                frozen: l.frozen,
                locked: l.locked,
            })
            .collect();
        let objects = document
            .objects()
            .iter()
            .map(|o| {
                Ok(ObjectDto {
                    id: o.id.0,
                    layer: o.attrs.layer.0,
                    color: attr_to_dto(&o.attrs.color, |c| (*c).into()),
                    linetype: attr_to_dto(&o.attrs.linetype, String::clone),
                    lineweight: attr_to_dto(&o.attrs.lineweight, |w| w.0),
                    scale: linetype::ScaleDto::of(&o.attrs),
                    layout: o.layout.map(|l| l.0),
                    entity: EntityDto::from_entity(&o.entity)?,
                })
            })
            .collect::<Result<_, FileError>>()?;
        let (next_object_id, next_layer_id) = document.next_ids();
        let (next_layout_id, next_viewport_id) = document.next_layout_ids();
        let stock_styles = text::is_stock(document);
        Ok(Self {
            next_object_id,
            next_layer_id,
            current_layer: document.current_layer().0,
            units: UnitsDto::from(document.units()),
            dim_style: sparse_style(document.dim_style()),
            dim_styles: sparse_table(document.dim_styles()),
            layers,
            objects,
            layouts: document
                .layouts()
                .iter()
                .map(LayoutDto::from_layout)
                .collect(),
            next_layout_id,
            next_viewport_id,
            text_styles: if stock_styles {
                Vec::new()
            } else {
                document
                    .text_styles()
                    .iter()
                    .map(TextStyleDto::from_style)
                    .collect()
            },
            current_text_style: document.current_text_style().0,
            next_text_style_id: document.next_text_style_id(),
            mleader_styles: mleader_style::TableDto::of(document),
            blocks: document
                .blocks()
                .iter()
                .map(|b| BlockDto::from_block(b))
                .collect::<Result<_, FileError>>()?,
            next_block_id: document.next_block_id(),
            linetypes: linetype::TableDto::of(document),
            new_style: new_style::NewStyleDto::of(document),
            point_style: point_style::PointStyleDto::of(document),
        })
    }
}

#[cfg(test)]
mod tests {
    use od_core::{
        AngleFormat, Attributes, Ellipse, Entity, EntityId, InsertUnits, KnotParameterization,
        Layer, LayerId, LengthFormat, Lineweight, Point, Spline, SplineFitData, TextStyleId, Units,
    };

    use super::*;

    pub(super) fn read(text: &str) -> Result<Document, FileError> {
        NativeFormat.read(text.as_bytes()).map(|l| l.document)
    }

    pub(super) fn roundtrip(document: &Document) -> Document {
        let bytes = NativeFormat.write(document).unwrap().bytes;
        NativeFormat.read(&bytes).unwrap().document
    }

    /// A drawing using every kind of entity and attribute.
    fn rich() -> Document {
        let mut document = Document::default();
        let walls = document.allocate_layer_id();
        document.push_layer(Layer {
            id: walls,
            name: "Walls".into(),
            color: Color::Rgb(200, 30, 30),
            linetype: "Dashed".into(),
            lineweight: Lineweight(50),
            on: true,
            frozen: false,
            locked: true,
        });
        let hidden = document.allocate_layer_id();
        document.push_layer(Layer {
            on: false,
            frozen: true,
            ..Layer::new(hidden, "Hidden")
        });
        document.set_current_layer(walls);
        document.set_units(Units {
            length: LengthFormat::Architectural,
            length_precision: 5,
            angle: AngleFormat::Surveyor,
            angle_precision: 2,
            clockwise: true,
            base_angle: 1.5,
            insert: InsertUnits::from_code(4).unwrap(),
        });
        let p = Point::new;
        document.add(Entity::Line {
            start: p(0.0, 0.0),
            end: p(10.5, -3.25),
        });
        document.add(Entity::Circle {
            center: p(1.0, 2.0),
            radius: 0.5,
        });
        document.add(Entity::Rectangle {
            min: p(-1.0, -1.0),
            max: p(4.0, 2.0),
        });
        document.add(Entity::Arc {
            center: p(2.0, 3.0),
            radius: 1.5,
            start: 0.25,
            end: 4.0,
        });
        document.add(Entity::Ellipse(
            Ellipse::full(p(5.0, 5.0), p(3.0, 4.0), 0.25).unwrap(),
        ));
        document.add(Entity::Ellipse(
            Ellipse::new(p(-5.0, 1.0), p(0.0, -2.0), 1e-6, 5.5, 1.0).unwrap(),
        ));
        let fitted = vec![p(0.0, 0.0), p(2.0, 3.0), p(5.0, 1.0)];
        let fitted = Spline::from_fit_points(fitted, KnotParameterization::SqrtChord).unwrap();
        document.add(Entity::Spline(Box::new(fitted)));
        let mut rational = Spline::new_nurbs(
            2,
            vec![p(1.0, 0.0), p(1.0, 1.0), p(0.0, 1.0)],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            Some(vec![1.0, 0.5f64.sqrt(), 1.0]),
            false,
            false,
        )
        .unwrap();
        rational.knot_tolerance = 1e-9;
        rational.fit = Some(SplineFitData {
            points: vec![p(1.0, 0.0), p(0.0, 1.0)],
            tolerance: 0.25,
            start_tangent: Some(p(0.0, 1.0)),
            end_tangent: Some(p(-1.0, 0.0)),
            parameterization: None,
        });
        document.add(Entity::Spline(Box::new(rational)));
        let loop_ = Spline::new_nurbs(
            1,
            vec![p(0.0, 0.0), p(1.0, 0.0), p(0.0, 1.0), p(0.0, 0.0)],
            vec![0.0, 0.0, 1.0, 2.0, 3.0, 3.0],
            None,
            true,
            true,
        )
        .unwrap();
        document.add(Entity::Spline(Box::new(loop_)));
        document.add_with(
            Entity::Polyline {
                points: vec![p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)],
                closed: true,
                bulges: vec![0.0, -0.5, 0.25],
                widths: Vec::new(),
            },
            Attributes {
                layer: LayerId::DEFAULT,
                color: Attr::Set(Color::Rgb(1, 2, 3)),
                linetype: Attr::Set("Center".into()),
                lineweight: Attr::Set(Lineweight(100)),
                linetype_scale: 2.5,
                continuous_generation: true,
            },
        );
        document
    }

    #[test]
    fn everything_survives_a_round_trip() {
        let document = rich();
        assert_eq!(roundtrip(&document), document);
    }

    #[test]
    fn a_default_drawing_round_trips() {
        let document = Document::default();
        assert_eq!(roundtrip(&document), document);
    }

    #[test]
    fn ids_of_deleted_objects_are_not_reused_after_loading() {
        let mut document = Document::default();
        let first = document.add(Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: 1.0,
        });
        let second = document.add(Entity::Circle {
            center: Point::new(5.0, 0.0),
            radius: 1.0,
        });
        document.remove(second);
        let layer = document.allocate_layer_id();
        document.push_layer(Layer::new(layer, "Gone"));
        document.remove_layer(layer);

        let mut loaded = roundtrip(&document);
        assert!(loaded.contains(first) && !loaded.contains(second));
        assert!(loaded.allocate_id() > second, "id {second:?} stays retired");
        assert!(loaded.allocate_layer_id() > layer);
    }

    #[test]
    fn the_file_is_readable_text_with_a_header() {
        let text = String::from_utf8(NativeFormat.write(&rich()).unwrap().bytes).unwrap();
        assert!(text.contains("\"format\": \"opendraft-drawing\""));
        assert!(text.contains(&format!("\"version\": {CURRENT_VERSION}")));
        assert!(text.contains("\"type\": \"polyline\""));
        assert!(text.contains("\"type\": \"arc\""));
        assert!(text.contains("\"name\": \"Walls\""));
    }

    pub(super) const MINIMAL: &str = r#"{
        "format": "opendraft-drawing", "version": 3,
        "drawing": {
            "next_object_id": 0, "next_layer_id": 1, "current_layer": 0,
            "layers": [ { "id": 0, "name": "0", "color": "foreground",
                          "linetype": "Continuous", "lineweight": 25 } ],
            "objects": []
        }
    }"#;

    #[test]
    fn a_minimal_hand_written_file_reads_with_defaults() {
        let document = read(MINIMAL).unwrap();
        let layer = document.layer(LayerId::DEFAULT).unwrap();
        assert!(layer.on && !layer.frozen && !layer.locked);
    }

    #[test]
    fn polyline_arcs_round_trip_and_straight_polylines_write_no_bulges() {
        let mut document = Document::default();
        document.add(Entity::Polyline {
            points: vec![Point::new(0.0, 0.0), Point::new(2.0, 0.0)],
            closed: false,
            bulges: vec![1.0, 0.0],
            widths: Vec::new(),
        });
        document.add(Entity::Polyline {
            points: vec![Point::new(0.0, 0.0), Point::new(2.0, 0.0)],
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        });
        assert_eq!(roundtrip(&document), document);
        let text = String::from_utf8(NativeFormat.write(&document).unwrap().bytes).unwrap();
        assert_eq!(text.matches("\"bulges\"").count(), 1, "only the curved one");
        // A bulge list of the wrong length, or not a number, is refused.
        let polyline = |extra: &str| {
            MINIMAL.replace(
                "\"objects\": []",
                &format!(
                    r#""objects": [ {{ "id": 0, "layer": 0, "entity": {{ "type": "polyline",
                        "points": [[0, 0], [1, 0], [1, 1]], {extra} }} }} ]"#
                ),
            )
        };
        assert!(read(&polyline(r#""bulges": [0, 0.5, 0]"#)).is_ok());
        assert!(matches!(
            read(&polyline(r#""bulges": [0.5]"#)),
            Err(FileError::Invalid(_))
        ));
        assert!(
            read(&polyline(r#""closed": true"#)).is_ok(),
            "no bulges is straight"
        );
    }

    #[test]
    fn polyline_widths_round_trip_and_hairlines_write_none() {
        let mut document = Document::default();
        document.add(Entity::Polyline {
            points: vec![Point::new(0.0, 0.0), Point::new(2.0, 0.0)],
            closed: false,
            bulges: Vec::new(),
            widths: vec![(0.5, 1.0), (0.0, 0.0)],
        });
        document.add(Entity::Polyline {
            points: vec![Point::new(0.0, 0.0), Point::new(2.0, 0.0)],
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        });
        assert_eq!(roundtrip(&document), document);
        let text = String::from_utf8(NativeFormat.write(&document).unwrap().bytes).unwrap();
        assert_eq!(text.matches("\"widths\"").count(), 1, "only the wide one");
        let polyline = |extra: &str| {
            MINIMAL.replace(
                "\"objects\": []",
                &format!(
                    r#""objects": [ {{ "id": 0, "layer": 0, "entity": {{ "type": "polyline",
                        "points": [[0, 0], [1, 0]], {extra} }} }} ]"#
                ),
            )
        };
        assert!(read(&polyline(r#""widths": [[1, 2], [0, 0]]"#)).is_ok());
        for bad in [r#""widths": [[1, 2]]"#, r#""widths": [[-1, 2], [0, 0]]"#] {
            assert!(
                matches!(read(&polyline(bad)), Err(FileError::Invalid(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn a_minimal_file_opens_and_a_bad_arc_is_refused() {
        assert!(read(MINIMAL).is_ok());
        let arc = |radius: &str| {
            MINIMAL.replace(
                "\"objects\": []",
                &format!(
                    r#""objects": [ {{ "id": 0, "layer": 0, "entity": {{ "type": "arc",
                        "center": [0, 0], "radius": {radius}, "start": 0, "end": 1 }} }} ]"#
                ),
            )
        };
        assert!(read(&arc("2")).is_ok());
        assert!(matches!(read(&arc("-2")), Err(FileError::Invalid(_))));
        assert!(matches!(read(&arc("0")), Err(FileError::Invalid(_))));
    }

    #[test]
    fn ellipses_keep_every_value_and_bad_ones_are_refused() {
        let ellipse = |fields: &str| {
            MINIMAL.replace(
                "\"objects\": []",
                &format!(
                    r#""objects": [ {{ "id": 0, "layer": 0, "entity": {{ "type": "ellipse",
                        {fields} }} }} ]"#
                ),
            )
        };
        let good = r#""center": [1, 2], "major_axis": [3, 0], "radius_ratio": 0.5,
                      "start": 0.5, "end": 2"#;
        let document = read(&ellipse(good)).unwrap();
        let Entity::Ellipse(e) = &document.objects()[0].entity else {
            panic!("an ellipse")
        };
        assert_eq!((e.radius_ratio, e.start, e.end), (0.5, 0.5, 2.0));
        for bad in [
            good.replace("0.5,", "1.5,"),
            good.replace("[3, 0]", "[0, 0]"),
            good.replace("\"end\": 2", "\"end\": 0.5"),
            good.replace(", \"end\": 2", ""),
        ] {
            assert!(read(&ellipse(&bad)).is_err(), "{bad}");
        }
        let text = String::from_utf8(NativeFormat.write(&rich()).unwrap().bytes).unwrap();
        assert!(text.contains("\"type\": \"ellipse\""));
    }

    #[test]
    fn unknown_fields_are_ignored_so_additions_stay_readable() {
        let text = MINIMAL.replace("\"objects\": []", "\"objects\": [], \"views\": [1, 2]");
        assert!(read(&text).is_ok());
    }

    #[test]
    fn attributes_default_to_by_layer() {
        let text = MINIMAL.replace(
            "\"objects\": []",
            r#""objects": [ { "id": 7, "layer": 0,
                "entity": { "type": "line", "start": [0, 0], "end": [1, 1] } } ]"#,
        );
        let document = read(&text).unwrap();
        let object = &document.objects()[0];
        assert_eq!(object.id, EntityId(7));
        assert_eq!(object.attrs, Attributes::on_layer(LayerId::DEFAULT));
        let mut document = document;
        assert!(document.allocate_id() > EntityId(7));
    }

    #[test]
    fn files_that_are_not_drawings_are_refused() {
        for text in [
            "",
            "not json",
            "{}",
            "[1,2]",
            r#"{"format":"other","version":1}"#,
        ] {
            assert!(
                matches!(read(text), Err(FileError::Unreadable(_))),
                "{text:?}"
            );
        }
    }

    #[test]
    fn a_newer_file_is_reported_as_newer_even_with_unknown_content() {
        let text = r#"{ "format": "opendraft-drawing", "version": 99,
                        "drawing": { "something": "new" } }"#;
        match read(text) {
            Err(FileError::TooNew { found, supported }) => {
                assert_eq!((found, supported), (99, CURRENT_VERSION));
            }
            other => panic!("expected TooNew, got {other:?}"),
        }
    }

    #[test]
    fn an_older_file_is_refused_as_older_even_with_unknown_content() {
        let text = r#"{ "format": "opendraft-drawing", "version": 1,
                        "drawing": { "something": "old" } }"#;
        assert!(matches!(
            read(text),
            Err(FileError::TooOld {
                found: 1,
                supported: CURRENT_VERSION
            })
        ));
        // Also when it would have read: only the current version does.
        let older = MINIMAL.replace("\"version\": 3", "\"version\": 1");
        assert!(matches!(read(&older), Err(FileError::TooOld { .. })));
    }

    #[test]
    fn a_file_over_the_size_limit_is_refused_before_it_is_read() {
        assert!(NativeFormat.check_size(MAX_BYTES).is_ok());
        assert!(matches!(
            NativeFormat.check_size(MAX_BYTES + 1),
            Err(FileError::Unsupported(_))
        ));
    }

    #[test]
    fn the_largest_id_is_refused_not_wrapped_or_panicked_on() {
        let max = u64::MAX;
        let layer = |id: u64| {
            MINIMAL.replace(
                "\"layers\": [",
                &format!(
                    r#""layers": [ {{ "id": {id}, "name": "Last", "color": "foreground",
                        "linetype": "Continuous", "lineweight": 25 }},"#
                ),
            )
        };
        assert!(read(&layer(5)).is_ok());
        assert!(matches!(read(&layer(max)), Err(FileError::Invalid(_))));
        let object = |id: u64| {
            MINIMAL.replace(
                "\"objects\": []",
                &format!(
                    r#""objects": [ {{ "id": {id}, "layer": 0, "entity": {{ "type": "line",
                        "start": [0, 0], "end": [1, 1] }} }} ]"#
                ),
            )
        };
        assert!(read(&object(7)).is_ok());
        assert!(matches!(read(&object(max)), Err(FileError::Invalid(_))));
    }

    fn invalid_with(objects: &str, layers: &str) -> FileError {
        let text = format!(
            r#"{{ "format": "opendraft-drawing", "version": 3,
              "drawing": {{ "next_object_id": 0, "next_layer_id": 1, "current_layer": 0,
                "layers": [{layers}], "objects": [{objects}] }} }}"#
        );
        read(&text).unwrap_err()
    }

    const LAYER0: &str = r#"{ "id": 0, "name": "0", "color": "foreground",
                              "linetype": "Continuous", "lineweight": 25 }"#;

    #[test]
    fn damaged_content_is_reported_not_loaded() {
        let line = |layer: u64, id: u64| {
            format!(
                r#"{{ "id": {id}, "layer": {layer},
                  "entity": {{ "type": "line", "start": [0,0], "end": [1,1] }} }}"#
            )
        };
        // An object on a layer that is not there.
        assert!(matches!(
            invalid_with(&line(5, 1), LAYER0),
            FileError::Invalid(_)
        ));
        // The same object id twice.
        let twice = format!("{}, {}", line(0, 1), line(0, 1));
        assert!(matches!(
            invalid_with(&twice, LAYER0),
            FileError::Invalid(_)
        ));
        // No layer 0.
        let layer1 = LAYER0.replace("\"id\": 0", "\"id\": 1");
        assert!(matches!(invalid_with("", &layer1), FileError::Invalid(_)));
        // The same layer twice.
        let layers = format!("{LAYER0}, {LAYER0}");
        assert!(matches!(invalid_with("", &layers), FileError::Invalid(_)));
        // A circle with no radius, a one-point polyline.
        let circle = r#"{ "id": 1, "layer": 0,
            "entity": { "type": "circle", "center": [0,0], "radius": 0 } }"#;
        assert!(matches!(
            invalid_with(circle, LAYER0),
            FileError::Invalid(_)
        ));
        let polyline = r#"{ "id": 1, "layer": 0,
            "entity": { "type": "polyline", "points": [[0,0]] } }"#;
        assert!(matches!(
            invalid_with(polyline, LAYER0),
            FileError::Invalid(_)
        ));
    }

    #[test]
    fn an_unknown_entity_kind_is_refused_never_skipped() {
        let text = MINIMAL.replace(
            "\"objects\": []",
            r#""objects": [ { "id": 1, "layer": 0, "entity": { "type": "hatch" } } ]"#,
        );
        assert!(matches!(read(&text), Err(FileError::Unreadable(_))));
    }

    #[test]
    fn a_missing_current_layer_falls_back_to_layer_zero() {
        let text = MINIMAL.replace("\"current_layer\": 0", "\"current_layer\": 42");
        assert_eq!(read(&text).unwrap().current_layer(), LayerId::DEFAULT);
    }

    #[test]
    fn layouts_survive_a_round_trip_and_keep_ids_retired() {
        let mut document = rich();
        let vp = document.allocate_viewport_id();
        let gone = document.allocate_layout_id();
        let layout = od_core::Layout {
            id: document.allocate_layout_id(),
            name: "Sheet 1".into(),
            width: 420.0,
            height: 297.0,
            viewports: vec![od_core::Viewport {
                locked: true,
                scale: 0.02,
                ..od_core::Viewport::framing(vp, Point::new(210.0, 148.0), 380.0, 250.0, None)
            }],
        };
        document.set_layouts(vec![layout]);
        let mut loaded = roundtrip(&document);
        assert_eq!(loaded, document);
        assert!(loaded.allocate_layout_id() > gone);
        assert!(loaded.allocate_viewport_id() > vp);
    }

    #[test]
    fn layers_frozen_in_a_viewport_round_trip() {
        let mut document = Document::default();
        let vp = document.allocate_viewport_id();
        let mut viewport = od_core::Viewport::framing(vp, Point::new(10.0, 10.0), 50.0, 50.0, None);
        viewport.set_frozen(&[LayerId(0), LayerId(4)], true);
        let layout = od_core::Layout {
            id: document.allocate_layout_id(),
            name: "Sheet".into(),
            width: 297.0,
            height: 210.0,
            viewports: vec![viewport],
        };
        document.set_layouts(vec![layout]);
        let loaded = roundtrip(&document);
        assert_eq!(
            loaded.layouts()[0].viewports[0].frozen,
            [LayerId(0), LayerId(4)]
        );
        let text = String::from_utf8(NativeFormat.write(&document).unwrap().bytes).unwrap();
        assert!(text.contains("\"frozen\""));
        let plain = String::from_utf8(NativeFormat.write(&rich()).unwrap().bytes).unwrap();
        assert!(
            !plain.contains("\"frozen\": ["),
            "none written when none are frozen"
        );
    }

    #[test]
    fn objects_on_a_sheet_round_trip_and_need_their_layout() {
        let mut document = Document::default();
        let model = document.add(Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: 1.0,
        });
        let layout = od_core::Layout {
            id: document.allocate_layout_id(),
            name: "Sheet".into(),
            width: 297.0,
            height: 210.0,
            viewports: Vec::new(),
        };
        let id = layout.id;
        document.set_layouts(vec![layout]);
        document.set_active_layout(Some(id));
        let sheet = document.add(Entity::Line {
            start: Point::new(5.0, 5.0),
            end: Point::new(50.0, 5.0),
        });
        document.set_active_layout(None);
        let loaded = roundtrip(&document);
        assert_eq!(loaded.object(sheet).unwrap().layout, Some(id));
        assert_eq!(loaded.object(model).unwrap().layout, None);
        let text = String::from_utf8(NativeFormat.write(&document).unwrap().bytes).unwrap();
        assert_eq!(
            text.matches("\"layout\":").count(),
            1,
            "only the sheet object"
        );

        let orphan = r#""objects": [ { "id": 1, "layer": 0, "layout": 7,
            "entity": { "type": "line", "start": [0, 0], "end": [1, 1] } } ]"#;
        let bad = MINIMAL.replace("\"objects\": []", orphan);
        assert!(matches!(read(&bad), Err(FileError::Invalid(_))));
    }

    #[test]
    fn a_file_without_layouts_has_none_and_bad_layouts_are_refused() {
        assert!(read(MINIMAL).unwrap().layouts().is_empty());
        let with = |layouts: &str| {
            MINIMAL.replace(
                "\"objects\": []",
                &format!("\"objects\": [], \"layouts\": [{layouts}]"),
            )
        };
        let good = r#"{ "id": 1, "name": "A", "width": 297, "height": 210, "viewports": [
            { "id": 1, "center": [10, 10], "width": 5, "height": 5,
              "view_center": [0, 0], "scale": 0.5 } ] }"#;
        assert!(read(&with(good)).is_ok());
        assert!(matches!(
            read(&with(&format!("{good}, {good}"))),
            Err(FileError::Invalid(_))
        ));
        let flat = good.replace("\"scale\": 0.5", "\"scale\": 0");
        assert!(matches!(read(&with(&flat)), Err(FileError::Invalid(_))));
        let no_sheet = good.replace("\"width\": 297", "\"width\": 0");
        assert!(matches!(read(&with(&no_sheet)), Err(FileError::Invalid(_))));
    }

    #[test]
    fn units_survive_a_round_trip() {
        let document = rich();
        assert_eq!(roundtrip(&document).units(), document.units());
    }

    #[test]
    fn a_file_without_units_reads_as_decimal() {
        assert_eq!(*read(MINIMAL).unwrap().units(), Units::default());
    }

    #[test]
    fn units_that_do_not_exist_are_refused() {
        let text = MINIMAL.replace(
            "\"current_layer\": 0",
            "\"current_layer\": 0, \"units\": { \"length\": 9 }",
        );
        assert!(matches!(read(&text), Err(FileError::Invalid(_))));
    }

    /// A drawing with a second style and text that uses every optional field.
    fn with_text() -> Document {
        use od_core::mtext::{
            BackgroundFill, ColumnKind, EntityLineSpacing, LineSpacingStyle, MTextAttachment,
            MTextBackground, MTextColumns, MTextFlow,
        };
        use od_core::{FontDescriptor, MText, TextStyle};

        let mut document = Document::default();
        let notes = document.allocate_text_style_id();
        document.push_text_style(TextStyle {
            font: FontDescriptor::TrueType {
                family: "Arial".into(),
                bold: true,
                italic: false,
            },
            width_factor: 0.8,
            oblique_radians: 0.2,
            fixed_height: Some(3.5),
            ..TextStyle::new(notes, "Notes")
        });
        let roman = document.allocate_text_style_id();
        document.push_text_style(TextStyle {
            font: FontDescriptor::OdRoman {
                bold: true,
                italic: true,
            },
            stroke_weight: 0.18,
            ..TextStyle::new(roman, "Roman")
        });
        let legacy = document.allocate_text_style_id();
        document.push_text_style(TextStyle {
            font: FontDescriptor::Shx {
                file_name: "txt.shx".into(),
            },
            big_font: Some("bigfont.shx".into()),
            backward: true,
            ..TextStyle::new(legacy, "Legacy")
        });
        document.set_current_text_style(notes);
        let plain = MText::new(Point::new(1.0, 2.0), 2.5, "Hello", TextStyleId::STANDARD);
        document.add(Entity::MText(Box::new(plain)));
        let rich = MText {
            direction: Point::new(0.0, 2.0),
            reference_width: 40.0,
            attachment: MTextAttachment::MiddleRight,
            flow: MTextFlow::ByStyle,
            style: notes,
            contents: "A\\P\\fArial|b1;B\\H2x;\\pxi-3,l3;C \\U+00E9 %<\\AcVar Date>%".into(),
            line_spacing: EntityLineSpacing {
                style: LineSpacingStyle::Exact,
                factor: 1.5,
            },
            background: Some(MTextBackground {
                fill: BackgroundFill::Color(Color::Rgb(9, 8, 7)),
                scale: 1.5,
            }),
            columns: MTextColumns {
                kind: ColumnKind::Dynamic,
                count: 3,
                reversed: true,
                auto_height: true,
                width: 12.0,
                gutter: 2.0,
                heights: vec![30.0, 31.0],
            },
            preserved: od_core::mtext::PreservedMTextFields {
                unimplemented_441: Some(7),
                background_extra_bits: 16,
            },
            ..MText::new(Point::new(-5.0, 6.0), 3.0, "", legacy)
        };
        document.add(Entity::MText(Box::new(rich)));
        document
    }

    #[test]
    fn text_styles_and_multiline_text_survive_a_round_trip_exactly() {
        let document = with_text();
        document.validate().unwrap();
        assert_eq!(roundtrip(&document), document);
        let text = String::from_utf8(NativeFormat.write(&document).unwrap().bytes).unwrap();
        assert!(text.contains("\"type\": \"mtext\""));
        assert!(
            text.contains(r"\\pxi-3,l3;"),
            "contents are written verbatim"
        );
    }

    #[test]
    fn a_drawing_without_text_writes_no_style_table_and_reads_back_with_standard() {
        let text = String::from_utf8(NativeFormat.write(&rich()).unwrap().bytes).unwrap();
        assert!(!text.contains("text_styles") && !text.contains("mtext"));
        let loaded = read(MINIMAL).unwrap();
        assert_eq!(loaded.text_styles().len(), 1);
        assert_eq!(loaded.current_text_style(), TextStyleId::STANDARD);
        assert_eq!(loaded.next_text_style_id(), 1);
    }

    #[test]
    fn ids_of_text_styles_are_not_reused_after_loading() {
        let mut document = with_text();
        let gone = document.allocate_text_style_id();
        let mut loaded = roundtrip(&document);
        assert!(loaded.allocate_text_style_id() > gone);
    }

    fn mtext_file(entity: &str) -> String {
        MINIMAL.replace(
            "\"objects\": []",
            &format!(
                r#""objects": [ {{ "id": 1, "layer": 0, "entity": {{ "type": "mtext",
                    "insertion": [0, 0], "height": 2.5, "contents": "Hi" {entity} }} }} ]"#
            ),
        )
    }

    #[test]
    fn damaged_text_is_reported_not_loaded() {
        assert!(read(&mtext_file("")).is_ok(), "the minimal text reads");
        for bad in [
            r#", "attachment": 0"#,
            r#", "attachment": 10"#,
            r#", "flow": 2"#,
            r#", "style": 4"#,
            r#", "direction": [0, 0]"#,
            r#", "width": -1"#,
            r#", "line_spacing": { "style": 3, "factor": 1 }"#,
            r#", "line_spacing": { "style": 1, "factor": 9 }"#,
            r#", "columns": { "kind": "wavy", "count": 2 }"#,
            r#", "columns": { "kind": "static", "count": 0 }"#,
            r#", "background": { "fill": "window", "scale": 0.5 }"#,
        ] {
            assert!(
                matches!(read(&mtext_file(bad)), Err(FileError::Invalid(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn text_with_no_height_is_refused() {
        let flat = mtext_file("").replace("\"height\": 2.5", "\"height\": 0");
        assert!(matches!(read(&flat), Err(FileError::Invalid(_))));
    }

    #[test]
    fn a_damaged_style_table_is_refused() {
        let table = |styles: &str| {
            MINIMAL.replace(
                "\"objects\": []",
                &format!("\"objects\": [], \"text_styles\": [{styles}]"),
            )
        };
        let standard = r#"{ "id": 0, "name": "Standard", "font": { "type": "bundled_sans" } }"#;
        let notes = r#"{ "id": 1, "name": "Notes", "font": { "type": "bundled_sans" } }"#;
        assert!(read(&table(standard)).is_ok());
        assert!(read(&table(&format!("{standard}, {notes}"))).is_ok());
        for bad in [
            notes.to_owned(),
            format!("{standard}, {standard}"),
            format!("{standard}, {}", notes.replace("Notes", "STANDARD")),
            format!("{standard}, {}", notes.replace("\"Notes\"", "\"\"")),
        ] {
            assert!(
                matches!(read(&table(&bad)), Err(FileError::Invalid(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn writing_refuses_what_could_not_be_read_back() {
        let mut document = Document::default();
        document.add(Entity::Line {
            start: Point::new(f64::NAN, 0.0),
            end: Point::new(1.0, 1.0),
        });
        assert!(matches!(
            NativeFormat.write(&document),
            Err(FileError::Invalid(_))
        ));
    }

    #[test]
    fn a_stock_linetype_table_is_left_out_and_old_files_read_with_it() {
        let text = String::from_utf8(NativeFormat.write(&rich()).unwrap().bytes).unwrap();
        assert!(!text.contains("linetypes"));
        assert!(!text.contains("next_linetype_id"));
        assert!(read(MINIMAL).unwrap().linetypes().is_stock());
    }

    #[test]
    fn linetypes_survive_a_round_trip_with_ids_and_counter() {
        use od_core::{LinetypeId, StrokeElement};
        let mut document = rich();
        let mut table = document.linetypes().clone();
        let mine = table
            .add(
                "Mine",
                "my line",
                vec![
                    StrokeElement::Dash(3.5),
                    StrokeElement::Gap(1.25),
                    StrokeElement::Dot,
                    StrokeElement::Gap(1.25),
                ],
            )
            .unwrap();
        table.load_stock("ACAD_ISO02W100").unwrap();
        let gone = table
            .add("Gone", "", vec![StrokeElement::Dash(1.0)])
            .unwrap();
        table.remove(gone).unwrap();
        document.set_linetypes(table);

        let loaded = roundtrip(&document);
        assert_eq!(loaded.linetypes(), document.linetypes());
        assert!(loaded.linetypes().get(mine).is_some());
        assert!(loaded.linetypes().get(LinetypeId::BYBLOCK).is_some());
        let mut loaded = loaded;
        let mut table = loaded.linetypes().clone();
        let next = table
            .add("Next", "", vec![StrokeElement::Dash(1.0)])
            .unwrap();
        assert!(next.0 > gone.0, "a deleted id is not handed out again");
        loaded.set_linetypes(table);
    }

    #[test]
    fn a_linetype_table_missing_a_special_record_is_refused() {
        use od_core::StrokeElement;
        let mut document = Document::default();
        let mut table = document.linetypes().clone();
        table
            .add("Mine", "", vec![StrokeElement::Dash(1.0)])
            .unwrap();
        document.set_linetypes(table);
        let bytes = NativeFormat.write(&document).unwrap().bytes;
        let mut json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let records = json["drawing"]["linetypes"].as_array_mut().unwrap();
        records.retain(|r| r["name"] != "BYLAYER");
        let text = serde_json::to_string(&json).unwrap();
        assert!(matches!(read(&text), Err(FileError::Invalid(_))));
    }
}
