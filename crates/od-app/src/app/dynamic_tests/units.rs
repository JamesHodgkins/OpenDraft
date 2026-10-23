use od_core::{AngleFormat, LengthFormat, Units};

use super::*;

fn architectural_grads() -> Units {
    Units {
        length: LengthFormat::Architectural,
        length_precision: 2,
        angle: AngleFormat::Gradians,
        ..Units::default()
    }
}

#[test]
fn fields_take_and_show_the_drawings_units() {
    let (mut app, ctx) = (app(), egui::Context::default());
    app.state.set_units(architectural_grads());
    line(&mut app, &ctx);
    app.state.answer_dynamic("0,0").unwrap();
    app.state.settle_command();
    frame(
        &mut app,
        &ctx,
        vec![Event::PointerMoved(Pos2::new(450.0, 300.0))],
        0.05,
    );
    frame(&mut app, &ctx, vec![Event::Text("1'".into())], 0.06);
    assert_eq!(app.state.docs[0].dynamic_input.draft, "1'");
    frame(&mut app, &ctx, key(Key::Tab), 0.5);
    assert_eq!(
        app.state.docs[0].dynamic_input.locks.get(&Field::Distance),
        Some(&12.0),
        "a foot is twelve drawing units"
    );
    frame(&mut app, &ctx, vec![Event::Text("100".into())], 0.51);
    frame(&mut app, &ctx, key(Key::Enter), 0.52);
    app.state.settle_command();
    let end = app.state.docs[0].view.last_point().unwrap();
    assert!(end.x.abs() < 1e-9 && (end.y - 12.0).abs() < 1e-9, "{end:?}");
}

#[test]
fn a_locked_angle_is_sent_in_degrees_whatever_the_format() {
    let mut app = app();
    app.state.set_units(architectural_grads());
    app.state.start_command("Rotate", |ed| {
        let angle = ed.get_angle("Specify angle")?;
        ed.say(format!("{angle:.4}"));
        Ok(())
    });
    app.state.settle_command();
    let ask = app.state.waiting().unwrap().clone();
    let units = app.state.units();
    let input = &mut app.state.docs[0].dynamic_input;
    input.begin(Field::Angle);
    input.locks.insert(Field::Angle, 90.0);
    let text = input
        .submission(&ask, &app.state.settings.dynamic_input, &units)
        .unwrap();
    app.state.answer_dynamic(&text).unwrap();
    app.state.settle_command();
    assert_eq!(app.state.cmdline.log.last().unwrap(), "1.5708");
}

#[test]
fn the_units_dialog_opens_and_escape_cancels_it() {
    let (mut app, ctx) = (app(), egui::Context::default());
    app.theme.apply(&ctx);
    let before = app.state.units();
    app.state.show_units = true;
    let run = |app: &mut OpenDraftApp, events, time| {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            events,
            time: Some(time),
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| app.units_window(ctx));
    };
    run(&mut app, Vec::new(), 0.0);
    assert!(app.units_draft.is_some(), "the dialog works on a draft");
    app.units_draft.as_mut().unwrap().length = LengthFormat::Fractional;
    run(&mut app, key(Key::Escape), 0.1);
    assert!(!app.state.show_units);
    assert!(app.units_draft.is_none());
    assert_eq!(app.state.units(), before, "Cancel changes nothing");
}

#[test]
fn direction_control_restores_the_angle_on_cancel() {
    let (mut app, ctx) = (app(), egui::Context::default());
    app.theme.apply(&ctx);
    app.state.show_units = true;
    let run = |app: &mut OpenDraftApp, events, time| {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            events,
            time: Some(time),
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| app.units_window(ctx));
    };
    run(&mut app, Vec::new(), 0.0);
    app.units_draft.as_mut().unwrap().base_angle = 1.0;
    app.units_direction = Some(0.25);
    run(&mut app, Vec::new(), 0.1);
    run(&mut app, key(Key::Escape), 0.2);
    assert!(
        app.units_direction.is_none(),
        "only Direction Control closed"
    );
    assert!(app.state.show_units, "the main dialog stays open");
    assert_eq!(app.units_draft.as_ref().unwrap().base_angle, 0.25);
}
