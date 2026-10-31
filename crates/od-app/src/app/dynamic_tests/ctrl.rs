//! Ctrl pressed and let go on its own, while an end grip is being edited,
//! switches it between Stretch and Lengthen.

use od_canvas::GripHit;
use od_core::{Entity, Point};

use super::*;

/// One frame with Ctrl held or not, and the given events.
fn ctrl_frame(app: &mut OpenDraftApp, ctx: &egui::Context, down: bool, events: Vec<Event>, t: f64) {
    let raw = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
        modifiers: Modifiers {
            ctrl: down,
            command: down,
            ..Modifiers::NONE
        },
        events,
        time: Some(t),
        ..Default::default()
    };
    let _ = ctx.run(raw, |ctx| {
        app.state.pump_command();
        app.route_typing_to_command_line(ctx);
    });
    app.state.settle_command();
}

fn editing_an_end_grip(app: &mut OpenDraftApp) {
    app.state.add_entity(Entity::Line {
        start: Point::new(0.0, 0.0),
        end: Point::new(10.0, 0.0),
    });
    let id = app.state.session().document().objects()[0].id;
    app.state.select_ids([id], false);
    app.state.start_grip_edit(vec![GripHit { id, index: 1 }]);
    app.state.settle_command();
}

fn asking(app: &mut OpenDraftApp) -> String {
    app.state.cmd_prompt().text.to_string()
}

/// Dynamic input at work, as in the real app: it handles every frame while a
/// point is asked for, which once starved the Ctrl watch.
#[test]
fn a_tap_of_ctrl_cycles_while_dynamic_input_is_showing() {
    let (mut app, ctx) = (app(), egui::Context::default());
    editing_an_end_grip(&mut app);
    let input = &mut app.state.docs[0].dynamic_input;
    input.canvas = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)));
    input.cursor = Some(Point::new(5.0, 5.0));
    ctrl_frame(&mut app, &ctx, true, Vec::new(), 0.1);
    ctrl_frame(&mut app, &ctx, false, Vec::new(), 0.2);
    assert!(asking(&mut app).starts_with("Specify new end point"));
}

#[test]
fn a_tap_of_ctrl_cycles_but_ctrl_with_a_key_does_not() {
    let (mut app, ctx) = (app(), egui::Context::default());
    editing_an_end_grip(&mut app);
    assert!(asking(&mut app).starts_with("Specify stretch point"));

    ctrl_frame(&mut app, &ctx, true, Vec::new(), 0.1);
    assert!(
        asking(&mut app).starts_with("Specify stretch point"),
        "nothing until it is let go"
    );
    ctrl_frame(&mut app, &ctx, false, Vec::new(), 0.2);
    assert!(asking(&mut app).starts_with("Specify new end point"));

    // Ctrl+Z is an undo shortcut, not a tap.
    let z = Event::Key {
        key: Key::Z,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::CTRL,
    };
    ctrl_frame(&mut app, &ctx, true, Vec::new(), 0.3);
    ctrl_frame(&mut app, &ctx, true, vec![z], 0.4);
    ctrl_frame(&mut app, &ctx, false, Vec::new(), 0.5);
    assert!(asking(&mut app).starts_with("Specify new end point"));

    // A tap again goes back.
    ctrl_frame(&mut app, &ctx, true, Vec::new(), 0.6);
    ctrl_frame(&mut app, &ctx, false, Vec::new(), 0.7);
    assert!(asking(&mut app).starts_with("Specify stretch point"));
}
