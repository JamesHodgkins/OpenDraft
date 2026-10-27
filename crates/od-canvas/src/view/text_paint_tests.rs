//! Painting only the lines that reach the screen.

use std::collections::HashSet;

use egui::epaint::Primitive;
use egui::{Color32, Context, LayerId, Pos2, RawInput, Rect, Vec2};
use od_core::{MText, Point, TextStyle, TextStyleId};

use super::text_paint::paint_text;
use crate::camera::Camera;

/// Vertices submitted for `text` through `camera` into `rect`, as the exact
/// bits of each position.
fn painted(text: &MText, camera: &Camera, rect: Rect) -> Vec<[u32; 2]> {
    let layout = text.layout(&TextStyle::standard());
    let ctx = Context::default();
    let out = ctx.run(
        RawInput {
            screen_rect: Some(rect),
            ..RawInput::default()
        },
        |ctx| {
            let painter = ctx.layer_painter(LayerId::background());
            paint_text(&painter, camera, rect, text, &layout, &|_| Color32::WHITE);
        },
    );
    ctx.tessellate(out.shapes, 1.0)
        .iter()
        .flat_map(|m| match &m.primitive {
            Primitive::Mesh(mesh) => mesh.vertices.iter().map(|v| v.pos).collect(),
            Primitive::Callback(_) => Vec::new(),
        })
        .map(|p| [p.x.to_bits(), p.y.to_bits()])
        .collect()
}

fn long_text() -> MText {
    let contents = (0..60).map(|n| format!("line {n}")).collect::<Vec<_>>();
    MText::new(
        Point::new(0.0, 0.0),
        2.5,
        contents.join(r"\P"),
        TextStyleId::STANDARD,
    )
}

#[test]
fn zoomed_in_on_a_long_text_only_the_visible_lines_are_built() {
    let text = long_text();
    let big = Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 4200.0));
    let small = Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 80.0));
    // 40 px text, the camera on the top lines. The same camera into a tall
    // viewport shows far more of the text than into a short one.
    let camera = Camera {
        center: Point::new(30.0, -8.0),
        zoom: 16.0,
    };
    let all = painted(&text, &camera, big);
    let few = painted(&text, &camera, small);
    assert!(!few.is_empty());
    assert!(
        few.len() * 3 < all.len(),
        "{} against {}",
        few.len(),
        all.len()
    );
}

#[test]
fn what_reaches_the_screen_is_painted_exactly_as_without_the_shortcut() {
    // Every vertex of a triangle that is inside the short viewport when
    // everything is built must also be built when only what shows is.
    let text = long_text();
    let tall = Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 4000.0));
    let view = Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 120.0));
    let camera = Camera {
        center: Point::new(30.0, -3.0),
        zoom: 16.0,
    };
    // The same world point must land at the same pixel in both viewports.
    let shift = Vec2::new(0.0, (4000.0 - 120.0) / 2.0);
    let reference = painted(&text, &camera, tall);
    let culled: HashSet<[u32; 2]> = painted(&text, &camera, view)
        .into_iter()
        .map(|p| [p[0], p[1]])
        .collect();
    assert!(!culled.is_empty());
    // Positions differ between the two viewports by `shift`, so compare in
    // the short viewport's pixels.
    let inside = |p: Pos2| view.contains(p);
    let missing = reference
        .iter()
        .map(|b| Pos2::new(f32::from_bits(b[0]), f32::from_bits(b[1])) - shift)
        .filter(|p| inside(*p))
        .filter(|p| {
            !culled.iter().any(|c| {
                let q = Pos2::new(f32::from_bits(c[0]), f32::from_bits(c[1]));
                (q.x - p.x).abs() < 0.01 && (q.y - p.y).abs() < 0.01
            })
        })
        .count();
    assert_eq!(missing, 0);
}
