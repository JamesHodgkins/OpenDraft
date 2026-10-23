#![allow(clippy::print_stdout, clippy::unwrap_used)]
//! Writes a small DXF with one of each kind of dimension, for checking it
//! with ezdxf (see `tests/fixtures/check_dimensions.py`).
//!
//!     cargo run -p od-files --example export_dimensions -- out.dxf

use od_core::{AngleFormat, DimStyle, DimText, Dimension, Document, Entity, Point};

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let p = Point::new;
    let style = DimStyle::default();
    let mut document = Document::default();
    let mut linear = Dimension::linear(p(0.0, 0.0), p(30.0, 10.0), p(15.0, 20.0), 0.0, &style);
    linear.text = DimText::Custom("<> TYP".to_owned());
    let mut tall = style.clone();
    tall.text_height = 4.0;
    let mut aligned = Dimension::aligned(p(0.0, 0.0), p(30.0, 10.0), p(5.0, -8.0), &tall);
    aligned.text_pos = Some(p(40.0, -12.0));
    let mut slanted = Dimension::linear(p(40.0, 0.0), p(70.0, 0.0), p(55.0, 10.0), 0.0, &style);
    if let od_core::DimKind::Linear { oblique, .. } = &mut slanted.kind {
        *oblique = Some(1.2);
    }
    let radians = DimStyle {
        angle_format: AngleFormat::DecimalDegrees,
        angle_precision: 1,
        ..style.clone()
    };
    for entity in [
        Entity::Dimension(Box::new(linear)),
        Entity::Dimension(Box::new(aligned)),
        Entity::Dimension(Box::new(slanted)),
        Entity::Dimension(Box::new(Dimension::radius(
            p(100.0, 0.0),
            p(110.0, 0.0),
            &style,
        ))),
        Entity::Dimension(Box::new(Dimension::diameter(
            p(100.0, 30.0),
            p(100.0, 40.0),
            &style,
        ))),
        Entity::Dimension(Box::new(Dimension::angular(
            p(0.0, 50.0),
            p(20.0, 50.0),
            p(0.0, 70.0),
            p(14.0, 64.0),
            false,
            &radians,
        ))),
        Entity::Dimension(Box::new(Dimension::angular(
            p(50.0, 50.0),
            p(70.0, 50.0),
            p(70.0, 70.0),
            p(65.0, 53.0),
            true,
            &style,
        ))),
        Entity::Dimension(Box::new(Dimension::ordinate(
            p(30.0, 80.0),
            p(30.0, 95.0),
            true,
            &style,
        ))),
        Entity::Dimension(Box::new(Dimension::arc_length(
            p(0.0, 0.0),
            p(5.0, 0.0),
            p(0.0, 5.0),
            p(7.0, 7.0),
            &style,
        ))),
    ] {
        document.add(entity);
    }
    let notes = od_files::save(std::path::Path::new(&path), &document).unwrap();
    for note in notes {
        println!("note: {note}");
    }
}
