use super::*;
use crate::dynamic_input::cursor_clearance;

fn check_clearance(app: &OpenDraftApp) {
    let doc = &app.state.docs[0];
    let input = &doc.dynamic_input;
    let canvas = input.canvas.unwrap();
    let cursor = doc
        .view
        .camera
        .world_to_screen(input.cursor.unwrap(), canvas);
    let clearance = cursor_clearance(cursor);
    assert!(!input.hit_rects.is_empty());
    for (i, rect) in input.hit_rects.iter().enumerate() {
        assert!(canvas.contains_rect(*rect), "outside canvas: {rect:?}");
        assert!(
            !rect.intersects(clearance),
            "field {rect:?} obstructs cursor {cursor:?}"
        );
        for other in &input.hit_rects[i + 1..] {
            assert!(!rect.intersects(*other), "{rect:?} overlaps {other:?}");
        }
    }
}

#[test]
fn short_line_fields_leave_crosshair_clear_at_edges_and_scales() {
    for size in [Vec2::new(640.0, 400.0), Vec2::new(1280.0, 800.0)] {
        for scale in [1.0, 1.25, 1.5] {
            let (mut app, ctx) = (app(), egui::Context::default());
            app.state.settings.snap.enabled = false;
            line(&mut app, &ctx);
            ctx.set_zoom_factor(scale);
            sized_frame(&mut app, &ctx, Vec::new(), 0.02, size);
            let canvas = app.state.docs[0].dynamic_input.canvas.unwrap();
            let cursors = [
                canvas.center(),
                canvas.left_top() + Vec2::splat(2.0),
                canvas.right_top() + Vec2::new(-2.0, 2.0),
                canvas.right_bottom() - Vec2::splat(2.0),
                canvas.left_bottom() + Vec2::new(2.0, -2.0),
            ];
            for cursor in cursors {
                for length in [0.0, 8.0, 40.0, 100.0] {
                    for angle in [0.0_f32, 51.3402, 90.0, 180.0, 270.0] {
                        let direction = Vec2::angled(-angle.to_radians());
                        let base = app.state.docs[0]
                            .view
                            .camera
                            .screen_to_world(cursor - direction * length, canvas);
                        app.state.cancel_command();
                        app.state.start_command("Line", crate::draw::draw_line);
                        app.state.settle_command();
                        app.state.answer_point(base);
                        app.state.settle_command();
                        app.state.docs[0].dynamic_input.clear();
                        sized_frame(
                            &mut app,
                            &ctx,
                            vec![Event::PointerMoved(cursor)],
                            0.03,
                            size,
                        );
                        assert_eq!(app.state.docs[0].dynamic_input.hit_rects.len(), 3);
                        check_clearance(&app);
                    }
                }
            }
        }
    }
}

#[test]
fn pointer_fields_menu_and_error_leave_crosshair_clear() {
    let (mut app, ctx) = (app(), egui::Context::default());
    app.state.settings.snap.enabled = false;
    line(&mut app, &ctx);
    check_clearance(&app);
    frame(&mut app, &ctx, vec![Event::Text("bad".into())], 0.02);
    frame(&mut app, &ctx, key(Key::Enter), 0.03);
    assert!(app.state.docs[0].dynamic_input.error.is_some());
    assert_eq!(app.state.docs[0].dynamic_input.hit_rects.len(), 4);
    check_clearance(&app);
    frame(&mut app, &ctx, key(Key::Escape), 0.04);
    app.state.answer_point(Point::new(0.0, 0.0));
    app.state.settle_command();
    frame(&mut app, &ctx, key(Key::ArrowDown), 0.05);
    assert!(app.state.docs[0].dynamic_input.menu);
    assert_eq!(app.state.docs[0].dynamic_input.hit_rects.len(), 4);
    check_clearance(&app);
}
