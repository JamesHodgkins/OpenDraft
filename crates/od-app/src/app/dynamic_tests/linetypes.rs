//! The Linetype Manager dialog.

use od_core::{LinetypeId, StrokeElement};

use super::*;

fn run(app: &mut OpenDraftApp, ctx: &egui::Context, events: Vec<Event>, time: f64) {
    let raw = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
        events,
        time: Some(time),
        ..Default::default()
    };
    let _ = ctx.run(raw, |ctx| app.linetype_window(ctx));
}

#[test]
fn the_manager_works_on_a_draft_and_escape_drops_it() {
    let (mut app, ctx) = (app(), egui::Context::default());
    app.theme.apply(&ctx);
    app.state.show_linetypes = true;
    run(&mut app, &ctx, Vec::new(), 0.0);
    assert!(app.linetype_draft.is_some(), "the dialog works on a draft");
    run(&mut app, &ctx, key(Key::Escape), 0.1);
    assert!(!app.state.show_linetypes);
    assert!(app.linetype_draft.is_none());
    assert!(app.state.session().document().linetypes().is_stock());
}

#[test]
fn the_command_and_its_aliases_open_it() {
    let mut app = app();
    for alias in ["LINETYPE", "LT", "ltype", "DDLTYPE"] {
        app.state.show_linetypes = false;
        let id = app.state.cmd_submit(alias).expect(alias);
        let _ = app.registry.execute(id, &mut app.state);
        assert!(app.state.show_linetypes, "{alias}");
    }
}

#[test]
fn ok_commits_the_table_and_scale_as_one_undo_step() {
    let mut app = app();
    let mut table = app.state.session().document().linetypes().clone();
    table.load_stock("HIDDEN2").unwrap();
    table
        .add(
            "Mine",
            "",
            vec![StrokeElement::Dash(3.0), StrokeElement::Gap(1.0)],
        )
        .unwrap();
    app.state.set_linetypes(table, 2.0);
    let doc = &app.state.session().document();
    assert!(doc.linetypes().named("hidden2").is_some() && doc.linetype_scale() == 2.0);
    app.state.undo();
    let doc = &app.state.session().document();
    assert!(doc.linetypes().is_stock() && doc.linetype_scale() == 1.0);
}

#[test]
fn nothing_is_recorded_when_the_manager_changes_nothing() {
    let mut app = app();
    let table = app.state.session().document().linetypes().clone();
    app.state.set_linetypes(table, 1.0);
    assert!(!app.state.session().history().can_undo());
}

#[test]
fn a_linetype_in_use_is_not_for_deleting_but_a_free_one_is() {
    let mut app = app();
    app.state.add_entity(od_core::Entity::Line {
        start: od_core::Point::new(0.0, 0.0),
        end: od_core::Point::new(1.0, 0.0),
    });
    app.state.select_all();
    app.state.set_attrs(
        &crate::layers::AttrEdit::Linetype(od_core::Attr::Set(LinetypeId::DASHED)),
        None,
    );
    let doc = &app.state.session().document();
    assert!(doc.linetype_in_use(LinetypeId::DASHED));
    assert!(!doc.linetype_in_use(LinetypeId::HIDDEN));
    let hidden = doc.linetypes().named("HIDDEN").unwrap().id;
    assert!(!hidden.is_special() && !LinetypeId::CONTINUOUS.0.eq(&hidden.0));
}
