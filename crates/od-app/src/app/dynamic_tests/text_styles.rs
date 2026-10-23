//! The Text Style dialog.

use super::*;

fn run(app: &mut OpenDraftApp, ctx: &egui::Context, events: Vec<Event>, time: f64) {
    let raw = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
        events,
        time: Some(time),
        ..Default::default()
    };
    let _ = ctx.run(raw, |ctx| app.text_style_window(ctx));
}

#[test]
fn the_dialog_works_on_a_draft_and_escape_drops_it() {
    let (mut app, ctx) = (app(), egui::Context::default());
    app.theme.apply(&ctx);
    app.state.show_text_styles = true;
    run(&mut app, &ctx, Vec::new(), 0.0);
    assert!(app.text_style_draft.is_some());
    // Edits stay in the draft until Apply.
    if let Some(draft) = app.text_style_draft.as_mut() {
        draft.add("Notes").unwrap();
    }
    run(&mut app, &ctx, Vec::new(), 0.05);
    assert_eq!(app.state.session().document().text_styles().len(), 1);
    run(&mut app, &ctx, key(Key::Escape), 0.1);
    assert!(!app.state.show_text_styles);
    run(&mut app, &ctx, Vec::new(), 0.2);
    assert!(app.text_style_draft.is_none());
    assert_eq!(app.state.session().document().text_styles().len(), 1);
}

#[test]
fn a_style_with_every_kind_of_font_and_value_draws_without_trouble() {
    let (mut app, ctx) = (app(), egui::Context::default());
    app.theme.apply(&ctx);
    app.state.show_text_styles = true;
    run(&mut app, &ctx, Vec::new(), 0.0);
    for (i, name) in ["romans.shx", "Arial", "DejaVu Sans"].iter().enumerate() {
        if let Some(draft) = app.text_style_draft.as_mut() {
            draft.add(&format!("s{i}")).unwrap();
            if let Some(style) = draft.selected_style() {
                style.font = od_core::FontDescriptor::from_name(name, i == 2, false);
                style.width_factor = 0.0;
                style.oblique_radians = 5.0;
            }
        }
        run(&mut app, &ctx, Vec::new(), 0.1 * (i as f64 + 1.0));
    }
    assert!(app.text_style_draft.is_some());
}

#[test]
fn the_command_and_its_aliases_open_it() {
    let mut app = app();
    for alias in ["STYLE", "ST", "ddstyle"] {
        app.state.show_text_styles = false;
        let id = app.state.cmd_submit(alias).expect(alias);
        let _ = app.registry.execute(id, &mut app.state);
        assert!(app.state.show_text_styles, "{alias}");
    }
}

#[test]
fn an_od_roman_style_keeps_its_font_and_weight_through_the_dialog() {
    let (mut app, ctx) = (app(), egui::Context::default());
    app.theme.apply(&ctx);
    app.state.show_text_styles = true;
    run(&mut app, &ctx, Vec::new(), 0.0);
    if let Some(draft) = app.text_style_draft.as_mut() {
        draft.add("Drafting").unwrap();
        if let Some(style) = draft.selected_style() {
            style.font = od_core::FontDescriptor::from_name("OD Roman", true, false);
            style.stroke_weight = 0.18;
        }
    }
    for t in 1..4 {
        run(&mut app, &ctx, Vec::new(), 0.1 * f64::from(t));
    }
    let Some(draft) = app.text_style_draft.as_mut() else {
        panic!("the dialog closed");
    };
    let style = draft.selected_style().unwrap();
    assert_eq!(
        style.font,
        od_core::FontDescriptor::OdRoman {
            bold: true,
            italic: false
        }
    );
    assert_eq!(style.stroke_weight, 0.18);
    assert!(style.is_valid());
}
