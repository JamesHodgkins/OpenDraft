//! Exercise the real egui routing and floating widgets across frames.
use eframe::egui::{self, Event, Key, Modifiers, Pos2, RawInput, Rect, Vec2};
use od_core::Point;

use super::{dynamic_input::field_id, OpenDraftApp};
use crate::{dock, dynamic_input::Field, state::AppState};

mod command_line_focus;
mod ctrl;
mod dim_styles;
mod linetypes;
mod mleader_styles;
mod placement;
mod text_styles;
mod units;

fn app() -> OpenDraftApp {
    OpenDraftApp {
        theme: Default::default(),
        settings_draft: None,
        settings_tab: 0,
        units_draft: None,
        units_direction: None,
        linetype_draft: None,
        text_style_draft: None,
        text_style_new: None,
        dim_style_draft: None,
        dim_manager: None,
        mleader_windows: Default::default(),
        registry: crate::commands::build_registry(),
        shortcuts: Vec::new(),
        state: AppState::new(),
        ribbon: crate::ribbon_def::default_ribbon(),
        ribbon_state: Default::default(),
        dock: dock::default_layout(),
        window_title: String::new(),
        last_autosave: std::time::Instant::now(),
        io_waker_set: false,
        f12_pressed_at: None,
        dynamic_suppressed: false,
        ctrl_alone: false,
        ctrl_down: false,
        dynamic_keys: Vec::new(),
        automation: None,
        automation_forced: false,
        home_recent: None,
    }
}
fn frame(app: &mut OpenDraftApp, ctx: &egui::Context, events: Vec<Event>, time: f64) {
    sized_frame(app, ctx, events, time, Vec2::new(1280.0, 800.0));
}
fn sized_frame(
    app: &mut OpenDraftApp,
    ctx: &egui::Context,
    events: Vec<Event>,
    time: f64,
    size: Vec2,
) {
    if time == 0.0 {
        app.theme.apply(ctx);
    }
    let mut raw = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
        events,
        time: Some(time),
        ..Default::default()
    };
    app.capture_dynamic_keys(ctx, &mut raw);
    let _ = ctx.run(raw, |ctx| {
        app.state.pump_command();
        app.route_typing_to_command_line(ctx);
        app.handle_shortcuts(ctx);
        app.panels(ctx);
    });
}

fn click(pos: Pos2) -> Vec<Event> {
    let mut events = vec![Event::PointerMoved(pos)];
    events.extend([true, false].map(|pressed| Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }));
    events
}

fn key(key: Key) -> Vec<Event> {
    [true, false]
        .map(|pressed| Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        })
        .to_vec()
}

fn line(app: &mut OpenDraftApp, ctx: &egui::Context) {
    app.state.start_command("Line", crate::draw::draw_line);
    app.state.settle_command();
    frame(
        app,
        ctx,
        vec![Event::PointerMoved(Pos2::new(300.0, 300.0))],
        0.0,
    );
    frame(app, ctx, Vec::new(), 0.01);
}

#[test]
fn coordinate_expression_and_tab_lock_survive_cursor_motion() {
    let (mut app, ctx) = (app(), egui::Context::default());
    line(&mut app, &ctx);
    frame(&mut app, &ctx, vec![Event::Text("100,200".into())], 0.02);
    frame(&mut app, &ctx, Vec::new(), 0.03);
    assert_eq!(app.state.docs[0].dynamic_input.draft, "100,200");
    assert!(
        ctx.memory(|m| m.has_focus(field_id(app.state.docs[0].uid, Field::Coordinate))),
        "focused {:?}, active {:?}, editing {}",
        ctx.memory(|m| m.focused()),
        app.state.docs[0].dynamic_input.active,
        app.state.docs[0].dynamic_input.editing
    );
    frame(&mut app, &ctx, key(Key::Enter), 0.04);
    app.state.settle_command();
    assert_eq!(
        app.state.docs[0].view.last_point(),
        Some(Point::new(100.0, 200.0))
    );
    frame(
        &mut app,
        &ctx,
        vec![Event::PointerMoved(Pos2::new(450.0, 300.0))],
        0.05,
    );
    frame(&mut app, &ctx, vec![Event::Text("10".into())], 0.06);
    for n in 0..30 {
        frame(
            &mut app,
            &ctx,
            vec![Event::PointerMoved(Pos2::new(450.0 + n as f32, 300.0))],
            0.07 + n as f64 * 0.01,
        );
        assert_eq!(app.state.docs[0].dynamic_input.draft, "10");
        assert!(ctx.memory(|m| m.has_focus(field_id(app.state.docs[0].uid, Field::Distance))));
    }
    frame(&mut app, &ctx, key(Key::Tab), 0.5);
    assert_eq!(app.state.docs[0].dynamic_input.active, Some(Field::Angle));
    assert_eq!(
        app.state.docs[0].dynamic_input.locks.get(&Field::Distance),
        Some(&10.0)
    );
    frame(&mut app, &ctx, vec![Event::Text("90".into())], 0.51);
    frame(&mut app, &ctx, key(Key::Enter), 0.52);
    app.state.settle_command();
    assert_eq!(app.state.session().document().len(), 1);
    let end = app.state.docs[0].view.last_point().unwrap();
    assert!((end.x - 100.0).abs() < 1e-9 && (end.y - 210.0).abs() < 1e-9);
}

#[test]
fn command_line_focus_clears_dynamic_draft_and_owns_typing() {
    let (mut app, ctx) = (app(), egui::Context::default());
    line(&mut app, &ctx);
    frame(&mut app, &ctx, vec![Event::Text("12".into())], 0.02);
    ctx.memory_mut(|m| m.request_focus(egui::Id::new("command_line_input")));
    frame(&mut app, &ctx, vec![Event::Text("34".into())], 0.03);
    assert!(app.state.docs[0].dynamic_input.draft.is_empty());
    assert_eq!(app.state.cmdline.input, "34");
    assert!(app.state.session().document().is_empty());
}

#[test]
fn options_preserve_draft_and_fields_and_choose_through_command_path() {
    let (mut app, ctx) = (app(), egui::Context::default());
    app.state
        .start_command("Circle", crate::circle::draw_circle);
    app.state.settle_command();
    app.state.answer_dynamic("0,0").unwrap();
    app.state.settle_command();
    frame(
        &mut app,
        &ctx,
        vec![Event::PointerMoved(Pos2::new(300.0, 300.0))],
        0.0,
    );
    frame(&mut app, &ctx, vec![Event::Text("10".into())], 0.01);
    // The dimension follows the typed radius, so let its boxes settle first.
    frame(&mut app, &ctx, Vec::new(), 0.015);
    let before = app.state.docs[0].dynamic_input.hit_rects.clone();
    frame(&mut app, &ctx, key(Key::ArrowDown), 0.02);
    assert!(app.state.docs[0].dynamic_input.menu);
    assert_eq!(app.state.docs[0].dynamic_input.draft, "10");
    assert_eq!(
        &app.state.docs[0].dynamic_input.hit_rects[..before.len()],
        &before
    );
    frame(&mut app, &ctx, key(Key::Enter), 0.03);
    app.state.settle_command();
    assert!(
        app.state.cmd_prompt().plain().contains("diameter"),
        "prompt {:?}, log {:?}",
        app.state.cmd_prompt(),
        app.state.cmdline.log
    );
    assert!(app.state.cmdline.log.last().unwrap().ends_with("Diameter"));
}

#[test]
fn f12_tap_toggles_and_hold_restores_without_cancelling() {
    let (mut app, ctx) = (app(), egui::Context::default());
    line(&mut app, &ctx);
    frame(&mut app, &ctx, key(Key::F12), 0.02);
    assert!(!app.state.settings.dynamic_input.enabled());
    assert!(app.state.command_running());
    frame(&mut app, &ctx, key(Key::F12), 0.03);
    assert!(app.state.settings.dynamic_input.enabled());
    let down = Event::Key {
        key: Key::F12,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    frame(&mut app, &ctx, vec![down.clone()], 0.04);
    frame(&mut app, &ctx, Vec::new(), 0.5);
    assert!(app.dynamic_suppressed);
    assert!(app.state.settings.dynamic_input.enabled());
    let up = Event::Key {
        key: Key::F12,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    frame(&mut app, &ctx, vec![up], 0.51);
    assert!(!app.dynamic_suppressed && app.state.settings.dynamic_input.enabled());
    assert!(app.state.command_running());
}

#[test]
fn clicking_a_field_freezes_placement_and_selects_its_live_value() {
    let (mut app, ctx) = (app(), egui::Context::default());
    line(&mut app, &ctx);
    let fields = app.state.docs[0].dynamic_input.hit_rects.clone();
    let target = fields[2].center();
    frame(&mut app, &ctx, vec![Event::PointerMoved(target)], 0.02);
    assert_eq!(app.state.docs[0].dynamic_input.hit_rects, fields);
    frame(&mut app, &ctx, click(target), 0.03);
    assert!(app.state.session().document().is_empty());
    assert_eq!(app.state.docs[0].dynamic_input.active, Some(Field::Y));
    frame(&mut app, &ctx, vec![Event::Text("7".into())], 0.04);
    assert_eq!(app.state.docs[0].dynamic_input.draft, "7");
}

#[test]
fn malformed_draft_blocks_click_and_escape_clears_before_cancel() {
    let (mut app, ctx) = (app(), egui::Context::default());
    line(&mut app, &ctx);
    frame(&mut app, &ctx, vec![Event::Text("NaN,3".into())], 0.02);
    let canvas = app.state.docs[0].dynamic_input.canvas.unwrap();
    let pos = canvas.min + Vec2::new(150.0, 176.0);
    frame(&mut app, &ctx, vec![Event::PointerMoved(pos)], 0.03);
    for (n, pressed) in [true, false].iter().enumerate() {
        frame(
            &mut app,
            &ctx,
            vec![Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: *pressed,
                modifiers: Modifiers::NONE,
            }],
            0.04 + n as f64 * 0.01,
        );
    }
    assert!(app.state.session().document().is_empty());
    assert!(
        app.state.docs[0].dynamic_input.error.is_some(),
        "draft {:?} focus {:?} cursor {:?} last {:?} canvas {:?}",
        app.state.docs[0].dynamic_input.draft,
        ctx.memory(|m| m.focused()),
        app.state.cursor,
        app.state.docs[0].view.last_point(),
        app.state.docs[0].dynamic_input.canvas
    );
    assert!(app.state.waiting().unwrap().base.is_none());
    assert!(app.state.docs[0].view.last_point().is_none());
    frame(&mut app, &ctx, key(Key::Escape), 0.06);
    assert!(app.state.docs[0].dynamic_input.draft.is_empty());
    assert!(app.state.command_running());
    frame(&mut app, &ctx, key(Key::Escape), 0.07);
    assert!(!app.state.command_running());
}

#[test]
fn tooltips_stay_inside_the_canvas_without_overlap_at_edges_and_scales() {
    for size in [Vec2::new(640.0, 400.0), Vec2::new(1280.0, 800.0)] {
        for scale in [1.0, 1.25, 1.5] {
            let (mut app, ctx) = (app(), egui::Context::default());
            app.state.settings.snap.enabled = false;
            app.state.start_command("Line", crate::draw::draw_line);
            app.state.settle_command();
            app.theme.apply(&ctx);
            ctx.set_zoom_factor(scale);
            // The first frame after a zoom change lays out on a placeholder
            // screen; the second has the real canvas.
            for time in [0.01, 0.02] {
                sized_frame(
                    &mut app,
                    &ctx,
                    vec![Event::PointerMoved(Pos2::new(100.0, 100.0))],
                    time,
                    size,
                );
            }
            let canvas = app.state.docs[0].dynamic_input.canvas.unwrap();
            let base = app.state.docs[0]
                .view
                .camera
                .screen_to_world(canvas.center(), canvas);
            app.state.answer_point(base);
            app.state.settle_command();
            for (n, corner) in [
                canvas.left_top(),
                canvas.right_top(),
                canvas.right_bottom(),
                canvas.left_bottom(),
            ]
            .iter()
            .enumerate()
            {
                let pos = *corner + (canvas.center() - *corner).normalized() * 2.0;
                sized_frame(
                    &mut app,
                    &ctx,
                    vec![Event::PointerMoved(pos)],
                    0.1 + n as f64 * 0.1,
                    size,
                );
                let rects = &app.state.docs[0].dynamic_input.hit_rects;
                assert_eq!(rects.len(), 3);
                for (i, rect) in rects.iter().enumerate() {
                    assert!(
                        canvas.contains_rect(*rect),
                        "size {size:?} scale {scale} rect {rect:?} canvas {canvas:?}"
                    );
                    for other in &rects[i + 1..] {
                        assert!(
                            !rect.intersects(*other),
                            "size {size:?} scale {scale} rect {rect:?} overlaps {other:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn always_pointer_visibility_does_not_capture_idle_command_entry() {
    let (mut app, ctx) = (app(), egui::Context::default());
    app.state.settings.dynamic_input.pointer_visibility = 2;
    frame(
        &mut app,
        &ctx,
        vec![Event::PointerMoved(Pos2::new(300.0, 300.0))],
        0.0,
    );
    assert_eq!(app.state.docs[0].dynamic_input.hit_rects.len(), 2);
    frame(&mut app, &ctx, vec![Event::Text("L".into())], 0.01);
    assert_eq!(app.state.cmdline.input, "L");
}

#[test]
fn last_character_and_enter_in_one_frame_submit_the_complete_value() {
    let (mut app, ctx) = (app(), egui::Context::default());
    line(&mut app, &ctx);
    let mut events = vec![Event::Text("100,200".into())];
    events.extend(key(Key::Enter));
    frame(&mut app, &ctx, events, 0.02);
    frame(&mut app, &ctx, Vec::new(), 0.03);
    app.state.settle_command();
    assert_eq!(
        app.state.docs[0].view.last_point(),
        Some(Point::new(100.0, 200.0))
    );
    assert!(app.state.command_running());
}

#[test]
fn down_arrow_click_keeps_draft_focus_and_does_not_place_a_point() {
    let (mut app, ctx) = (app(), egui::Context::default());
    line(&mut app, &ctx);
    app.state.answer_dynamic("0,0").unwrap();
    app.state.settle_command();
    frame(&mut app, &ctx, Vec::new(), 0.02);
    frame(&mut app, &ctx, vec![Event::Text("10".into())], 0.03);
    let prompt = app.state.docs[0].dynamic_input.hit_rects[0];
    let target = prompt.right_center() - Vec2::new(12.0, 0.0);
    frame(&mut app, &ctx, vec![Event::PointerMoved(target)], 0.04);
    frame(&mut app, &ctx, click(target), 0.05);
    assert!(app.state.docs[0].dynamic_input.menu);
    assert_eq!(app.state.docs[0].dynamic_input.draft, "10");
    assert!(app.state.session().document().is_empty());
    frame(&mut app, &ctx, key(Key::Escape), 0.06);
    assert!(!app.state.docs[0].dynamic_input.menu);
    assert_eq!(app.state.docs[0].dynamic_input.draft, "10");
}

#[test]
fn escape_clears_text_typed_on_the_command_line_when_idle() {
    for visibility in [0, 2] {
        let (mut app, ctx) = (app(), egui::Context::default());
        app.state.settings.dynamic_input.pointer_visibility = visibility;
        frame(
            &mut app,
            &ctx,
            vec![Event::PointerMoved(Pos2::new(300.0, 300.0))],
            0.0,
        );
        frame(&mut app, &ctx, vec![Event::Text("L".into())], 0.01);
        frame(&mut app, &ctx, Vec::new(), 0.02);
        assert_eq!(app.state.cmdline.input, "L");
        frame(&mut app, &ctx, key(Key::Escape), 0.03);
        assert_eq!(app.state.cmdline.input, "", "visibility {visibility}");
    }
}
