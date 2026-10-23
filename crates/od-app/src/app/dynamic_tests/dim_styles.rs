//! The Dimension Style dialog.

use super::*;

fn run(app: &mut OpenDraftApp, ctx: &egui::Context, events: Vec<Event>, time: f64) {
    let raw = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
        events,
        time: Some(time),
        ..Default::default()
    };
    let _ = ctx.run(raw, |ctx| app.dim_style_window(ctx));
}

#[test]
fn every_tab_draws_and_escape_drops_the_draft_without_changing_the_style() {
    let (mut app, ctx) = (app(), egui::Context::default());
    app.theme.apply(&ctx);
    app.state.show_dim_style = true;
    run(&mut app, &ctx, Vec::new(), 0.0);
    for tab in 0..7 {
        if let Some(draft) = app.dim_style_draft.as_mut() {
            draft.tab = tab;
            draft.style.text_height = 5.0;
        }
        run(&mut app, &ctx, Vec::new(), 0.05 * (tab as f64 + 1.0));
    }
    assert!(app.dim_style_draft.is_some());
    run(&mut app, &ctx, key(Key::Escape), 1.0);
    assert!(!app.state.show_dim_style);
    run(&mut app, &ctx, Vec::new(), 1.1);
    assert!(app.dim_style_draft.is_none());
    assert_eq!(app.state.dim_style().text_height, 2.5);
}

#[test]
fn the_command_and_its_aliases_open_it() {
    let mut app = app();
    for alias in ["DIMSTYLE", "D", "dst", "DDIM"] {
        app.state.show_dim_manager = false;
        let id = app.state.cmd_submit(alias).expect(alias);
        let _ = app.registry.execute(id, &mut app.state);
        assert!(app.state.show_dim_manager, "{alias}");
    }
}

fn metric() -> od_core::DimStyle {
    od_core::DimStyle {
        name: "Metric".to_owned(),
        text_height: 3.5,
        ..od_core::DimStyle::default()
    }
}

#[test]
fn the_manager_draws_with_several_styles_and_escape_closes_it() {
    let (mut app, ctx) = (app(), egui::Context::default());
    app.theme.apply(&ctx);
    app.state.add_dim_style(metric()).unwrap();
    app.state.show_dim_manager = true;
    for i in 0..3 {
        let raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            time: Some(0.05 * f64::from(i)),
            ..Default::default()
        };
        let _ = ctx.run(raw, |ctx| app.dim_manager_window(ctx));
    }
    assert!(app.dim_manager.is_some());
    let raw = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
        events: key(Key::Escape),
        time: Some(1.0),
        ..Default::default()
    };
    let _ = ctx.run(raw, |ctx| app.dim_manager_window(ctx));
    assert!(!app.state.show_dim_manager);
}

#[test]
fn the_modify_dialog_edits_the_style_it_was_asked_for() {
    use crate::dim_style_table::DimEdit;
    let (mut app, ctx) = (app(), egui::Context::default());
    app.theme.apply(&ctx);
    app.state.add_dim_style(metric()).unwrap();
    app.state.dim_edit = DimEdit::Named("Metric".to_owned());
    app.state.show_dim_style = true;
    run(&mut app, &ctx, Vec::new(), 0.0);
    let draft = app.dim_style_draft.as_ref().unwrap();
    assert_eq!(draft.style.name, "Metric");
    assert_eq!(draft.style.text_height, 3.5);
    run(&mut app, &ctx, key(Key::Escape), 0.5);
    assert_eq!(app.state.dim_edit, DimEdit::Current, "back to the default");
}
