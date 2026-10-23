//! The Multileader Style Manager and the Modify Multileader Style dialog.

use super::*;
use od_core::mleader::{MLeaderLineType, MLeaderStyle, MLeaderStyleId};

fn frame(app: &mut OpenDraftApp, ctx: &egui::Context, events: Vec<Event>, time: f64) {
    let raw = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 900.0))),
        events,
        time: Some(time),
        ..Default::default()
    };
    let _ = ctx.run(raw, |ctx| {
        app.mleader_manager_window(ctx);
        app.mleader_style_window(ctx);
    });
}

fn notes() -> MLeaderStyle {
    MLeaderStyle {
        name: "Notes".to_owned(),
        arrow_size: 4.0,
        ..MLeaderStyle::standard()
    }
}

#[test]
fn the_command_and_its_name_open_the_manager() {
    let mut app = app();
    let id = app.state.cmd_submit("MLEADERSTYLE").expect("alias");
    let _ = app.registry.execute(id, &mut app.state);
    assert!(app.state.mleader_dialogs.manager);
}

#[test]
fn every_tab_draws_and_escape_drops_the_draft_without_changing_the_style() {
    let (mut app, ctx) = (app(), egui::Context::default());
    app.theme.apply(&ctx);
    app.state.mleader_dialogs.style = true;
    frame(&mut app, &ctx, Vec::new(), 0.0);
    for tab in 0..3 {
        if let Some(draft) = app.mleader_windows.draft.as_mut() {
            draft.tab = tab;
            draft.style.arrow_size = 9.0;
            draft.style.leader_type = MLeaderLineType::Spline;
            draft.style.content.frame = true;
        }
        frame(&mut app, &ctx, Vec::new(), 0.05 * (tab as f64 + 1.0));
    }
    assert!(app.mleader_windows.draft.is_some());
    frame(&mut app, &ctx, key(Key::Escape), 1.0);
    assert!(!app.state.mleader_dialogs.style);
    frame(&mut app, &ctx, Vec::new(), 1.1);
    assert!(app.mleader_windows.draft.is_none());
    assert_eq!(app.state.mleader_style(), MLeaderStyle::standard());
}

#[test]
fn the_manager_draws_with_several_styles_and_escape_closes_it() {
    let (mut app, ctx) = (app(), egui::Context::default());
    app.theme.apply(&ctx);
    app.state.add_mleader_style(notes()).unwrap();
    app.state.mleader_dialogs.manager = true;
    for i in 0..3 {
        frame(&mut app, &ctx, Vec::new(), 0.05 * f64::from(i));
    }
    assert!(app.mleader_windows.manager.is_some());
    frame(&mut app, &ctx, key(Key::Escape), 1.0);
    assert!(!app.state.mleader_dialogs.manager);
}

#[test]
fn modify_opens_for_the_selected_style_and_a_new_style_starts_from_a_copy() {
    let (mut app, ctx) = (app(), egui::Context::default());
    app.theme.apply(&ctx);
    app.state.add_mleader_style(notes()).unwrap();
    app.state.mleader_dialogs.edit =
        crate::mleader_style_table::MLeaderEdit::Named(MLeaderStyleId(1));
    app.state.mleader_dialogs.style = true;
    frame(&mut app, &ctx, Vec::new(), 0.0);
    let draft = app.mleader_windows.draft.as_ref().expect("a draft");
    assert_eq!(draft.style.name, "Notes");
    assert_eq!(draft.style.arrow_size, 4.0);
}
