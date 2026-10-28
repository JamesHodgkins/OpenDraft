//! The native format: a JSON file, extension `.odx`.
//!
//! The file has its own schema (the `*Dto` types here), not the in-memory
//! types serialised as they happen to be, so refactoring the app cannot
//! silently change what is on disk.
//!
//! # Development and stable
//!
//! The format has two phases, told apart by [`CURRENT_VERSION`]:
//!
//! - **Development (version 0, the value now).** The format is still being
//!   shaped and nothing promises that a file keeps opening. Any change to the
//!   schema (new entity kinds, renamed or removed fields, a new layout) is made
//!   in place and does NOT bump the version. A file saved by an earlier
//!   development build may fail to open; the error then names the build that
//!   saved it (`saved_by`). Nothing converts development files.
//! - **Stable (version 1 and up).** Once the format is declared stable,
//!   [`CURRENT_VERSION`] is set to 1, once, as a deliberate release step, never
//!   as a side effect of a feature. From then on files must keep opening:
//!   - Adding an optional field does not need a new version: old readers ignore
//!     it and files without it still read the same.
//!   - A new kind of entity DOES bump [`CURRENT_VERSION`]. A reader never skips
//!     an object it does not understand, because saving the file again would
//!     delete that object from the user's drawing. Instead it refuses the file
//!     as another version (`FileError::WrongVersion`).
//!   - Any other change that an older reader would misread bumps
//!     [`CURRENT_VERSION`] too, and `read` gains a conversion for the version
//!     it replaces.
//!
//! `read` judges the header (`format`, `version`, `saved_by`) before the body,
//! so a file of another version is refused by name ("saved by OpenDraft 0.1.3
//! in file format 3") instead of failing to parse. A development build reads
//! only version 0. There is no conversion code until there is something to
//! convert: the first stable bump adds the first conversion, in the same
//! change, ahead of the body parse.
//!
//! The file is bounded by [`MAX_BYTES`] and checked with
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
mod names;
mod new_style;
mod point_style;
pub(crate) mod spline;
mod text;
mod units;

use block::BlockDto;
use dimension_style::{DimStylesDto, StylePool};
use entity::EntityDto;
use layout::LayoutDto;
use text::TextStyleDto;
use units::UnitsDto;

/// The file format version this build writes and reads. 0 is the development
/// format, which changes without a bump; 1 is the first stable format. See the
/// module notes before changing this.
pub const CURRENT_VERSION: u32 = 0;
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
        let header: Header =
            serde_json::from_slice(bytes).map_err(|e| FileError::Unreadable(e.to_string()))?;
        if header.format != MAGIC {
            return Err(FileError::Unreadable("not an OpenDraft drawing".into()));
        }
        if header.version != CURRENT_VERSION {
            return Err(FileError::WrongVersion {
                found: header.version,
                supported: CURRENT_VERSION,
                saved_by: header.saved_by,
            });
        }
        let file: FileDto = serde_json::from_slice(bytes).map_err(|e| unreadable(&header, &e))?;
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

/// Just the start of a file, read first so the version is judged before the
/// body is (an older or newer body would not parse as this one's).
#[derive(Deserialize)]
struct Header {
    format: String,
    version: u32,
    #[serde(default)]
    saved_by: String,
}

/// The error for a body of the current version that did not parse. While the
/// format is in development (version 0) that usually means another build
/// wrote it.
fn unreadable(header: &Header, parse: &serde_json::Error) -> FileError {
    if header.version != 0 {
        return FileError::Unreadable(parse.to_string());
    }
    let by = if header.saved_by.is_empty() {
        "an unknown build"
    } else {
        &header.saved_by
    };
    FileError::Unreadable(format!(
        "{parse}; version 0 is the development format, which changes between builds, and this file was saved by {by}"
    ))
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
    units: UnitsDto,
    /// The dimension styles, in full: see `dimension_style.rs`.
    #[serde(flatten)]
    dim_styles: DimStylesDto,
    layers: Vec<LayerDto>,
    /// In draw order.
    objects: Vec<ObjectDto>,
    /// Sheets in tab order; counters keep deleted ids retired.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    layouts: Vec<LayoutDto>,
    next_layout_id: u64,
    next_viewport_id: u64,
    text_styles: Vec<TextStyleDto>,
    current_text_style: u64,
    next_text_style_id: u64,
    /// The multileader styles.
    #[serde(flatten)]
    mleader_styles: mleader_style::TableDto,
    /// Block definitions in table order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    blocks: Vec<BlockDto>,
    next_block_id: u64,
    /// The linetype table and `LTSCALE`.
    #[serde(flatten)]
    linetypes: linetype::TableDto,
    #[serde(flatten)]
    new_style: new_style::NewStyleDto,
    /// `PDMODE` and `PDSIZE`.
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
    on: bool,
    frozen: bool,
    locked: bool,
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
    color: AttrDto<ColorDto>,
    linetype: AttrDto<String>,
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
        let mut styles = StylePool::new();
        let named_styles = DimStylesDto::start(document, &mut styles);
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
                    entity: EntityDto::from_entity(&o.entity, &mut styles)?,
                })
            })
            .collect::<Result<_, FileError>>()?;
        let (next_object_id, next_layer_id) = document.next_ids();
        let (next_layout_id, next_viewport_id) = document.next_layout_ids();
        let blocks = document
            .blocks()
            .iter()
            .map(|b| BlockDto::from_block(b, &mut styles))
            .collect::<Result<_, FileError>>()?;
        Ok(Self {
            next_object_id,
            next_layer_id,
            current_layer: document.current_layer().0,
            units: UnitsDto::from(document.units()),
            dim_styles: DimStylesDto::of(document, &styles, named_styles)?,
            layers,
            objects,
            layouts: document
                .layouts()
                .iter()
                .map(LayoutDto::from_layout)
                .collect(),
            next_layout_id,
            next_viewport_id,
            text_styles: document
                .text_styles()
                .iter()
                .map(TextStyleDto::from_style)
                .collect(),
            current_text_style: document.current_text_style().0,
            next_text_style_id: document.next_text_style_id(),
            mleader_styles: mleader_style::TableDto::of(document),
            blocks,
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

    /// The tables a drawing must spell out (styles, linetypes, point style,
    /// units, counters).
    const TABLE_KEYS: [&str; 18] = [
        "dim_styles",
        "named_dim_styles",
        "current_dim_style",
        "text_styles",
        "current_text_style",
        "next_text_style_id",
        "mleader_styles",
        "current_mleader_style",
        "next_mleader_style_id",
        "linetypes",
        "next_linetype_id",
        "linetype_scale",
        "point_mode",
        "point_size",
        "units",
        "next_layout_id",
        "next_viewport_id",
        "next_block_id",
    ];

    fn fill(value: &mut serde_json::Value, defaults: &[(&str, serde_json::Value)]) {
        if let Some(map) = value.as_object_mut() {
            for (key, default) in defaults {
                map.entry(*key).or_insert_with(|| default.clone());
            }
        }
    }

    /// `text` with what a hand-written file leaves out added to its drawing:
    /// the stock tables, layer flags, by-layer attributes. The reader itself
    /// requires them all.
    pub(super) fn with_dim_styles(text: &str) -> String {
        use serde_json::json;
        let Ok(mut file) = serde_json::from_str::<serde_json::Value>(text) else {
            return text.to_owned();
        };
        let stock: serde_json::Value =
            serde_json::from_slice(&NativeFormat.write(&Document::default()).unwrap().bytes)
                .unwrap();
        let attrs = [
            ("color", json!("by_layer")),
            ("linetype", json!("by_layer")),
            ("lineweight", json!("by_layer")),
        ];
        let Some(drawing) = file.get_mut("drawing").and_then(|d| d.as_object_mut()) else {
            return file.to_string();
        };
        for key in TABLE_KEYS {
            drawing
                .entry(key)
                .or_insert_with(|| stock["drawing"][key].clone());
        }
        let each = |value: Option<&mut serde_json::Value>,
                    f: &mut dyn FnMut(&mut serde_json::Value)| {
            if let Some(items) = value.and_then(|v| v.as_array_mut()) {
                items.iter_mut().for_each(f);
            }
        };
        each(drawing.get_mut("layers"), &mut |layer| {
            fill(
                layer,
                &[
                    ("on", json!(true)),
                    ("frozen", json!(false)),
                    ("locked", json!(false)),
                ],
            );
        });
        each(drawing.get_mut("objects"), &mut |object| {
            fill(object, &attrs)
        });
        each(drawing.get_mut("blocks"), &mut |block| {
            each(block.get_mut("objects"), &mut |object| fill(object, &attrs));
        });
        each(drawing.get_mut("layouts"), &mut |layout| {
            each(layout.get_mut("viewports"), &mut |viewport| {
                fill(viewport, &[("on", json!(true)), ("locked", json!(false))]);
            });
        });
        file.to_string()
    }

    pub(super) fn read(text: &str) -> Result<Document, FileError> {
        NativeFormat
            .read(with_dim_styles(text).as_bytes())
            .map(|l| l.document)
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
        "format": "opendraft-drawing", "version": 0,
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
    fn a_file_of_another_version_is_refused_whatever_its_content() {
        let text = r#"{ "format": "opendraft-drawing", "version": 99,
                        "drawing": { "something": "new" } }"#;
        match read(text) {
            Err(FileError::WrongVersion {
                found, supported, ..
            }) => assert_eq!((found, supported), (99, CURRENT_VERSION)),
            other => panic!("expected WrongVersion, got {other:?}"),
        }
    }

    #[test]
    fn a_pre_reset_file_is_refused_and_names_the_build_that_saved_it() {
        // Declaring the format stable means setting CURRENT_VERSION to 1 as a
        // release step: follow "Declaring .odx stable" in CONTRIBUTING.md,
        // then change this assertion.
        assert_eq!(
            CURRENT_VERSION, 0,
            "development format: see the module notes"
        );
        let old = MINIMAL.replace(
            "\"version\": 0",
            "\"version\": 3, \"saved_by\": \"OpenDraft 0.1.3\"",
        );
        let error = read(&old).unwrap_err();
        assert!(matches!(
            error,
            FileError::WrongVersion {
                found: 3,
                supported: 0,
                ..
            }
        ));
        assert!(error.to_string().contains("OpenDraft 0.1.3"), "{error}");
    }

    #[test]
    fn a_development_file_that_does_not_parse_names_the_build_that_saved_it() {
        let text = r#"{ "format": "opendraft-drawing", "version": 0,
                        "saved_by": "OpenDraft 0.0.9",
                        "drawing": { "something": "from another build" } }"#;
        match read(text) {
            Err(FileError::Unreadable(why)) => {
                assert!(why.contains("development format"), "{why}");
                assert!(why.contains("OpenDraft 0.0.9"), "{why}");
            }
            other => panic!("expected Unreadable, got {other:?}"),
        }
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
            r#"{{ "format": "opendraft-drawing", "version": 0,
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
    fn a_missing_current_layer_is_refused() {
        let text = MINIMAL.replace("\"current_layer\": 0", "\"current_layer\": 42");
        assert!(matches!(read(&text), Err(FileError::Invalid(_))));
        assert_eq!(read(MINIMAL).unwrap().current_layer(), LayerId::DEFAULT);
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
    fn a_file_without_units_is_refused() {
        assert!(NativeFormat.read(MINIMAL.as_bytes()).is_err());
        assert_eq!(*read(MINIMAL).unwrap().units(), Units::default());
    }

    #[test]
    fn units_that_do_not_exist_are_refused() {
        let mut file: serde_json::Value = serde_json::from_str(&with_dim_styles(MINIMAL)).unwrap();
        file["drawing"]["units"]["length"] = "nine".into();
        assert!(read(&file.to_string()).is_err());
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
            reference_height: 30.0,
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
    fn a_drawing_without_text_still_writes_its_style_table_in_full() {
        let text = String::from_utf8(NativeFormat.write(&rich()).unwrap().bytes).unwrap();
        assert!(text.contains("\"text_styles\"") && !text.contains("\"mtext\""));
        assert!(text.contains("\"width_factor\"") && text.contains("\"stroke_weight\""));
        assert!(NativeFormat.read(MINIMAL.as_bytes()).is_err());
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
                    "insertion": [0, 0], "height": 2.5, "contents": "Hi",
                    "attachment": "top_left", "flow": "left_to_right" {entity} }} }} ]"#
            ),
        )
    }

    #[test]
    fn damaged_text_is_reported_not_loaded() {
        assert!(read(&mtext_file("")).is_ok(), "the minimal text reads");
        // A choice that does not exist cannot be written in the file at all.
        for unknown in [r#""attachment": "top_left""#, r#""flow": "left_to_right""#] {
            let nonsense = mtext_file("").replace(unknown, &unknown.replace('_', "-"));
            assert!(read(&nonsense).is_err(), "{unknown}");
        }
        for bad in [
            r#", "style": 4"#,
            r#", "direction": [0, 0]"#,
            r#", "width": -1"#,
            r#", "line_spacing": { "style": "at_least", "factor": 9 }"#,
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
                &format!(
                    "\"objects\": [], \"text_styles\": [{styles}], \"current_text_style\": 0, \"next_text_style_id\": 2"
                ),
            )
        };
        let style = |id: u32, name: &str| {
            format!(
                r#"{{ "id": {id}, "name": "{name}", "font": {{ "type": "bundled_sans" }},
                    "width_factor": 1.0, "oblique": 0.0, "backward": false,
                    "upside_down": false, "vertical": false, "stroke_weight": 0.1 }}"#
            )
        };
        let standard = style(0, "Standard");
        let notes = style(1, "Notes");
        let standard = standard.as_str();
        let notes = notes.as_str();
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
    fn a_stock_linetype_table_is_written_in_full_and_a_file_without_one_is_refused() {
        let text = String::from_utf8(NativeFormat.write(&rich()).unwrap().bytes).unwrap();
        assert!(text.contains("\"linetypes\"") && text.contains("\"next_linetype_id\""));
        assert!(NativeFormat.read(MINIMAL.as_bytes()).is_err());
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
