#![allow(clippy::print_stdout, clippy::unwrap_used)]
//! Writes a small DXF with every kind of thing the writer handles, for
//! checking it with other programs (see `tests/fixtures/check_export.py`).
//!
//!     cargo run -p od-files --example export_sample -- out.dxf

use od_core::{Attr, Attributes, Color, Document, Entity, Layer, LayerId, Lineweight, Point};

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let p = Point::new;
    let mut document = Document::default();
    let walls = document.allocate_layer_id();
    document.push_layer(Layer {
        color: Color::Rgb(255, 0, 0),
        linetype: "Dashed".into(),
        lineweight: Lineweight(50),
        locked: true,
        ..Layer::new(walls, "Walls")
    });
    let off = document.allocate_layer_id();
    document.push_layer(Layer {
        on: false,
        color: Color::Rgb(12, 200, 99),
        ..Layer::new(off, "Caf\u{e9} off")
    });
    document.add_with(
        Entity::Line {
            start: p(0.0, 0.0),
            end: p(10.0, 5.0),
        },
        Attributes::on_layer(walls),
    );
    document.add(Entity::Circle {
        center: p(5.0, 5.0),
        radius: 2.5,
    });
    document.add(Entity::Rectangle {
        min: p(0.0, 0.0),
        max: p(4.0, 3.0),
    });
    document.add_with(
        Entity::Polyline {
            points: vec![p(0.0, 0.0), p(2.0, 1.0), p(3.0, 4.0)],
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        },
        Attributes {
            layer: LayerId::DEFAULT,
            color: Attr::Set(Color::Rgb(0, 0, 255)),
            linetype: Attr::Set("Center".into()),
            lineweight: Attr::Set(Lineweight(35)),
            ..Attributes::on_layer(LayerId::DEFAULT)
        },
    );
    let notes = od_files::save(std::path::Path::new(&path), &document).unwrap();
    for note in notes {
        println!("note: {note}");
    }
}
