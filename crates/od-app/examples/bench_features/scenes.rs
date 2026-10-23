//! The drawings the benchmark draws and saves.

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui::Color32;
use od_canvas::CanvasStyle;
use od_core::{
    Attr, Attributes, BlockDef, BlockEntity, DimStyle, Dimension, Document, Entity, Insert,
    LayerId, Point,
};
use od_files::{DxfFormat, FileFormat};

/// Tiny deterministic generator so runs are comparable.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as f64 / (1u64 << 31) as f64
    }
}

/// `n` lines, circles, arcs, rectangles and polylines over a 10000 x 6000 sheet.
pub fn generate(n: usize) -> Vec<Entity> {
    let mut rng = Lcg(42);
    (0..n)
        .map(|i| {
            let (x, y) = (rng.next() * 10_000.0, rng.next() * 6_000.0);
            let size = 20.0 + rng.next() * 180.0;
            match i % 5 {
                0 => Entity::Line {
                    start: Point::new(x, y),
                    end: Point::new(x + size, y + rng.next() * size - size / 2.0),
                },
                1 => Entity::Circle {
                    center: Point::new(x, y),
                    radius: size / 2.0,
                },
                2 => Entity::Arc {
                    center: Point::new(x, y),
                    radius: size / 2.0,
                    start: 0.0,
                    end: 4.0,
                },
                3 => Entity::Rectangle {
                    min: Point::new(x, y),
                    max: Point::new(x + size, y + size * 0.6),
                },
                _ => Entity::Polyline {
                    points: vec![
                        Point::new(x, y),
                        Point::new(x + size, y),
                        Point::new(x + size, y + size / 2.0),
                        Point::new(x, y + size / 2.0),
                    ],
                    closed: false,
                    bulges: vec![0.0, 0.5, 0.0, 0.0],
                    widths: Vec::new(),
                },
            }
        })
        .collect()
}

pub fn style() -> CanvasStyle {
    CanvasStyle {
        background: Color32::from_rgb(0x25, 0x25, 0x25),
        grid_minor: Color32::from_white_alpha(18),
        grid_major: Color32::from_white_alpha(36),
        axis_x: Color32::GRAY,
        axis_y: Color32::GRAY,
        entity: Color32::WHITE,
        hover: Color32::from_rgba_unmultiplied(20, 115, 230, 60),
        selected: Color32::LIGHT_BLUE,
        selected_glow: Color32::from_rgba_unmultiplied(20, 115, 230, 120),
        snap: Color32::YELLOW,
        window: Color32::LIGHT_BLUE,
        crossing: Color32::LIGHT_GREEN,
    }
}

/// `document` with every object set to the linetype `name` and `LTSCALE` as given.
pub fn dashed(document: &Document, name: &str, scale: f64) -> Document {
    let mut out = Document::default();
    out.set_linetypes(document.linetypes().clone());
    out.set_linetype_scale(scale);
    for object in document.objects() {
        let attrs = Attributes {
            linetype: Attr::Set(name.to_owned()),
            ..Attributes::on_layer(LayerId::DEFAULT)
        };
        out.add_with(object.entity.clone(), attrs);
    }
    out
}

pub fn dimensions(n: usize) -> Document {
    let mut rng = Lcg(5);
    let style = DimStyle::default();
    Document::from_entities((0..n).map(|_| {
        let (x, y) = (rng.next() * 10_000.0, rng.next() * 6_000.0);
        let len = 60.0 + rng.next() * 200.0;
        let a = Point::new(x, y);
        let b = Point::new(x + len, y + rng.next() * 40.0);
        Entity::Dimension(Box::new(Dimension::aligned(
            a,
            b,
            Point::new(x, y + 30.0),
            &style,
        )))
    }))
}

/// `n` inserts of a block of twelve lines and circles.
pub fn inserts(n: usize) -> Document {
    let mut doc = Document::default();
    let entities = (0..12)
        .map(|i| BlockEntity {
            entity: if i % 2 == 0 {
                Entity::Line {
                    start: Point::new(f64::from(i), 0.0),
                    end: Point::new(f64::from(i) * 2.0, 10.0),
                }
            } else {
                Entity::Circle {
                    center: Point::new(f64::from(i), 5.0),
                    radius: 3.0,
                }
            },
            attrs: Attributes::on_layer(LayerId::DEFAULT),
        })
        .collect();
    let block = Arc::new(BlockDef {
        id: doc.allocate_block_id(),
        name: "Fitting".into(),
        base: Point::new(0.0, 0.0),
        description: String::new(),
        entities,
    });
    doc.push_block(block.clone());
    let mut rng = Lcg(9);
    for _ in 0..n {
        let at = Point::new(rng.next() * 10_000.0, rng.next() * 6_000.0);
        let mut insert = Insert::new(block.clone(), at);
        insert.scale_x = 4.0;
        insert.scale_y = 4.0;
        doc.add(Entity::Insert(Box::new(insert)));
    }
    doc
}

/// The hatches of `HatchTest.dxf` copied `copies` times across the sheet.
pub fn hatches(copies: usize) -> Option<Document> {
    let loaded = read_file("HatchTest.dxf")?;
    let hatches: Vec<Entity> = loaded
        .objects()
        .iter()
        .filter(|o| matches!(o.entity, Entity::Hatch(_)))
        .map(|o| o.entity.clone())
        .collect();
    let mut doc = Document::default();
    let mut rng = Lcg(3);
    for i in 0..copies {
        let (dx, dy) = (rng.next() * 1_500.0, rng.next() * 900.0);
        doc.add(hatches[i % hatches.len()].translated(dx, dy));
    }
    Some(doc)
}

pub fn spec_path(file: &str) -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "private_material",
        file,
    ]
    .iter()
    .collect()
}

pub fn read_file(file: &str) -> Option<Document> {
    let bytes = std::fs::read(spec_path(file)).ok()?;
    Some(DxfFormat.read(&bytes).ok()?.document)
}
