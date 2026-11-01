//! Snapping to the segments of the polyline being drawn, through the whole
//! frame path (prompt, canvas, click), as the app runs it.

use super::*;
use od_core::Entity;

/// The points the polyline command has placed so far.
fn placed(app: &OpenDraftApp) -> Vec<Point> {
    let ask = app.state.waiting().expect("PLINE is waiting");
    match ask.snap_geometry.first() {
        Some(Entity::Polyline { points, .. }) => points.clone(),
        _ => Vec::new(),
    }
}

/// A real click: the press and the release in frames of their own.
fn press(app: &mut OpenDraftApp, ctx: &egui::Context, at: Pos2, t: f64) {
    for (n, pressed) in [true, false].into_iter().enumerate() {
        let event = Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        frame(app, ctx, vec![event], t + n as f64 * 0.01);
    }
}

/// A polyline started with its first three points at screen positions.
fn started(app: &mut OpenDraftApp, ctx: &egui::Context, screen: [Pos2; 3]) -> Rect {
    app.state
        .start_command("Pline", crate::polyline_cmd::draw_polyline);
    app.state.settle_command();
    frame(app, ctx, vec![Event::PointerMoved(screen[0])], 0.0);
    frame(app, ctx, Vec::new(), 0.01);
    let canvas = app.state.docs[0].dynamic_input.canvas.expect("canvas");
    let mut t = 0.02;
    for at in screen {
        let world = app.state.docs[0].view.camera.screen_to_world(at, canvas);
        app.state.answer_point(world);
        app.state.settle_command();
        frame(app, ctx, vec![Event::PointerMoved(at)], t);
        t += 0.01;
    }
    canvas
}

#[test]
fn the_midpoint_of_a_segment_drawn_so_far_is_snapped_to() {
    let (mut app, ctx) = (app(), egui::Context::default());
    let (a, b, c) = (
        Pos2::new(300.0, 300.0),
        Pos2::new(500.0, 300.0),
        Pos2::new(500.0, 450.0),
    );
    let canvas = started(&mut app, &ctx, [a, b, c]);
    let points = placed(&app);
    assert_eq!(points.len(), 3, "{points:?}");
    let mid = Point::new((points[0].x + points[1].x) / 2.0, points[0].y);
    let mid_screen = app.state.docs[0].view.camera.world_to_screen(mid, canvas);
    // A few pixels off the midpoint of the first segment.
    let near = mid_screen + Vec2::new(3.0, 3.0);
    frame(&mut app, &ctx, vec![Event::PointerMoved(near)], 1.0);
    press(&mut app, &ctx, near, 1.1);
    app.state.settle_command();
    frame(&mut app, &ctx, Vec::new(), 1.2);
    let points = placed(&app);
    assert_eq!(points.len(), 4, "{points:?}");
    let q = points[3];
    assert!(
        (q.x - mid.x).abs() < 1e-6 && (q.y - mid.y).abs() < 1e-6,
        "placed {q:?}, midpoint {mid:?}"
    );
}

#[test]
fn the_start_and_inner_vertices_are_snapped_to() {
    for target in [0, 1] {
        let (mut app, ctx) = (app(), egui::Context::default());
        let (a, b, c) = (
            Pos2::new(300.0, 300.0),
            Pos2::new(500.0, 300.0),
            Pos2::new(500.0, 450.0),
        );
        let canvas = started(&mut app, &ctx, [a, b, c]);
        let vertex = placed(&app)[target];
        let at = app.state.docs[0]
            .view
            .camera
            .world_to_screen(vertex, canvas);
        let near = at + Vec2::new(-3.0, 2.0);
        frame(&mut app, &ctx, vec![Event::PointerMoved(near)], 1.0);
        press(&mut app, &ctx, near, 1.1);
        app.state.settle_command();
        frame(&mut app, &ctx, Vec::new(), 1.2);
        let points = placed(&app);
        assert_eq!(points.len(), 4, "{points:?}");
        assert_eq!(points[3], vertex, "vertex {target}");
    }
}
