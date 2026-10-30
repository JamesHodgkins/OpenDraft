use super::*;

#[test]
fn escape_clears_entry_after_widget_focus_is_lost() {
    let (mut app, ctx) = (app(), egui::Context::default());
    line(&mut app, &ctx);
    let command_line = egui::Id::new("command_line_input");
    ctx.memory_mut(|m| m.request_focus(command_line));
    frame(&mut app, &ctx, vec![Event::Text("U".into())], 0.02);
    assert_eq!(app.state.cmdline.input, "U");

    ctx.memory_mut(|m| m.surrender_focus(command_line));
    frame(&mut app, &ctx, key(Key::Escape), 0.03);

    assert!(app.state.cmdline.input.is_empty());
    assert!(
        app.state.command_running(),
        "Escape cleared entry instead of cancelling"
    );
}

#[test]
fn enter_submits_entry_after_widget_focus_is_lost() {
    let (mut app, ctx) = (app(), egui::Context::default());
    frame(&mut app, &ctx, vec![Event::Text("L".into())], 0.0);
    frame(&mut app, &ctx, Vec::new(), 0.01);
    let command_line = egui::Id::new("command_line_input");
    ctx.memory_mut(|m| m.surrender_focus(command_line));

    frame(&mut app, &ctx, key(Key::Enter), 0.02);

    assert!(app.state.command_running());
    assert!(app.state.cmdline.input.is_empty());
}

#[test]
fn up_and_down_browse_the_command_history_when_idle() {
    let (mut app, ctx) = (app(), egui::Context::default());
    for name in ["LINE", "CIRCLE"] {
        app.state.cmdline.note_typed(name);
    }
    frame(&mut app, &ctx, Vec::new(), 0.0);
    frame(&mut app, &ctx, Vec::new(), 0.01);

    frame(&mut app, &ctx, key(Key::ArrowUp), 0.02);
    assert_eq!(app.state.cmdline.input, "CIRCLE");
    frame(&mut app, &ctx, key(Key::ArrowUp), 0.03);
    assert_eq!(app.state.cmdline.input, "LINE");
    frame(&mut app, &ctx, key(Key::ArrowDown), 0.04);
    assert_eq!(app.state.cmdline.input, "CIRCLE");
}
