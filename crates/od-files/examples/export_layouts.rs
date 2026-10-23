#![allow(clippy::print_stdout, clippy::unwrap_used)]
//! Writes a DXF (R2000) of a drawing with layouts, viewports and sheet
//! objects, for checking it with other programs (see
//! `tests/fixtures/check_layouts.py`).
//!
//!     cargo run -p od-files --example export_layouts -- out.dxf

use od_core::{
    Attr, Attributes, Color, Document, Entity, Layer, LayerId, Layout, Lineweight, Point, Viewport,
};

fn main() {
    let out = std::env::args().nth(1).expect("output file");
    let p = Point::new;
    let mut doc = Document::default();
    let walls = doc.allocate_layer_id();
    doc.push_layer(Layer {
        color: Color::Rgb(200, 40, 40),
        lineweight: Lineweight(50),
        ..Layer::new(walls, "Walls")
    });
    doc.add(Entity::Line {
        start: p(0.0, 0.0),
        end: p(12000.0, 6000.0),
    });
    doc.add_with(
        Entity::Circle {
            center: p(6000.0, 3000.0),
            radius: 2000.0,
        },
        Attributes::on_layer(walls),
    );
    doc.add(Entity::Arc {
        center: p(2000.0, 2000.0),
        radius: 1500.0,
        start: 0.3,
        end: 4.0,
    });
    doc.add(Entity::Rectangle {
        min: p(100.0, 100.0),
        max: p(900.0, 500.0),
    });
    doc.add_with(
        Entity::Polyline {
            points: vec![p(0.0, 0.0), p(500.0, 0.0), p(500.0, 500.0)],
            closed: false,
            bulges: vec![0.5, 0.0, 0.0],
            widths: vec![(10.0, 10.0), (0.0, 0.0), (0.0, 0.0)],
        },
        Attributes {
            layer: LayerId::DEFAULT,
            color: Attr::Set(Color::Rgb(10, 120, 200)),
            linetype: Attr::Set("Dashed".into()),
            lineweight: Attr::Set(Lineweight(70)),
            ..Attributes::on_layer(LayerId::DEFAULT)
        },
    );

    let mut a3 = doc.new_layout("Plan", 420.0, 297.0);
    a3.viewports[0].scale = 0.02;
    a3.viewports[0].locked = true;
    a3.viewports[0].set_frozen(&[walls], true);
    let second = Viewport {
        id: od_core::ViewportId(a3.viewports[0].id.0 + 100),
        center: p(380.0, 40.0),
        width: 60.0,
        height: 50.0,
        view_center: p(500.0, 300.0),
        scale: 0.1,
        locked: false,
        on: false,
        frozen: Vec::new(),
    };
    a3.viewports.push(second);
    let a3_id = a3.id;
    let custom = Layout {
        id: od_core::LayoutId(a3_id.0 + 1),
        name: "Detail".into(),
        width: 200.0,
        height: 150.0,
        viewports: vec![Viewport::framing(
            od_core::ViewportId(900),
            p(100.0, 75.0),
            180.0,
            130.0,
            None,
        )],
    };
    let custom_id = custom.id;
    doc.set_layouts(vec![a3, custom]);
    doc.set_active_layout(Some(a3_id));
    doc.add(Entity::Rectangle {
        min: p(5.0, 5.0),
        max: p(415.0, 292.0),
    });
    doc.add(Entity::Line {
        start: p(5.0, 20.0),
        end: p(415.0, 20.0),
    });
    doc.set_active_layout(Some(custom_id));
    doc.add(Entity::Circle {
        center: p(100.0, 75.0),
        radius: 30.0,
    });
    doc.set_active_layout(None);
    let notes = od_files::save(std::path::Path::new(&out), &doc).unwrap();
    println!("notes: {notes:?}");
}
