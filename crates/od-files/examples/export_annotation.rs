#![allow(clippy::print_stdout, clippy::unwrap_used)]
//! Writes a DXF (R2000) of what the other export examples leave out:
//! multileaders, legacy leaders, formatted multiline text, hatches (user
//! defined, named, solid) and ellipses, for opening in AutoCAD.
//!
//!     cargo run -p od-files --example export_annotation -- out.dxf

use od_core::hatch::HatchFill;
use od_core::hatch::HatchPattern;
use od_core::hatch::{builtin, loops_from_entities, PatternType};
use od_core::{
    Document, Ellipse, Entity, HatchEntity, Leader, MLeader, MLeaderStyle, MText, Point,
    TextStyleId,
};

fn main() {
    let path = std::env::args().nth(1).unwrap();
    let p = Point::new;
    let mut document = Document::default();

    let standard = MLeaderStyle::standard();
    for (y, text) in [(0.0, "Standard note"), (20.0, "Two\\Plines")] {
        let m = MLeader::build(&standard, vec![p(0.0, y), p(12.0, y + 6.0)], text).unwrap();
        document.add(Entity::MLeader(Box::new(m)));
    }
    let three = MLeader::build(
        &standard,
        vec![p(0.0, 40.0), p(8.0, 44.0), p(16.0, 41.0)],
        "Bent",
    )
    .unwrap();
    document.add(Entity::MLeader(Box::new(three)));

    document.add(Entity::Leader(Box::new(Leader::new(
        vec![p(40.0, 0.0), p(48.0, 6.0), p(56.0, 6.0)],
        2.5,
    ))));

    let rich = "{\\Fsimplex;Plain}\\P\\LUnderlined\\l and \\Kstruck\\k\\P\\H1.5x;Taller\\H0.66x; back\\P\\S1/2; stack and \\S1^2; tolerance";
    document.add(Entity::MText(Box::new(MText::new(
        p(40.0, 20.0),
        2.5,
        rich,
        TextStyleId::STANDARD,
    ))));

    let square = Entity::Rectangle {
        min: p(80.0, 0.0),
        max: p(100.0, 20.0),
    };
    let circle = Entity::Circle {
        center: p(120.0, 10.0),
        radius: 10.0,
    };
    let solid = Entity::Rectangle {
        min: p(140.0, 0.0),
        max: p(160.0, 20.0),
    };
    document.add(square.clone());
    document.add(circle.clone());
    document.add(solid.clone());
    let make = |entity: &Entity, fill: HatchFill| {
        let (loops, _) = loops_from_entities(std::slice::from_ref(entity), 1e-6).unwrap();
        Entity::Hatch(Box::new(HatchEntity::new(loops, fill)))
    };
    let user =
        HatchPattern::user_defined(2.0, std::f64::consts::FRAC_PI_4, true, p(0.0, 0.0)).unwrap();
    document.add(make(&square, HatchFill::Pattern(user)));
    let def = builtin().find("ANSI31").unwrap();
    let named =
        HatchPattern::from_definition(def, PatternType::Predefined, 1.0, 0.0, p(0.0, 0.0)).unwrap();
    document.add(make(&circle, HatchFill::Pattern(named)));
    document.add(make(&solid, HatchFill::Solid));

    document.add(Entity::Ellipse(
        Ellipse::new(
            p(180.0, 10.0),
            p(12.0, 0.0),
            0.5,
            0.0,
            std::f64::consts::TAU,
        )
        .unwrap(),
    ));
    document.add(Entity::Ellipse(
        Ellipse::new(p(210.0, 10.0), p(8.0, 6.0), 0.4, 0.5, 4.0).unwrap(),
    ));

    let notes = od_files::save(std::path::Path::new(&path), &document).unwrap();
    for note in notes {
        println!("note: {note}");
    }
}
