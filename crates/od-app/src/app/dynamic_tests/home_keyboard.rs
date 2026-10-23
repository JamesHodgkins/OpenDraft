use std::path::PathBuf;

use super::*;

fn on_home() -> (OpenDraftApp, egui::Context) {
    let mut app = app();
    app.state.start_on_home();
    app.state.recent.add(&PathBuf::from("first.odx"));
    app.state.recent.add(&PathBuf::from("second.odx"));
    (app, egui::Context::default())
}

fn press(app: &mut OpenDraftApp, ctx: &egui::Context, k: Key, time: &mut f64) {
    *time += 0.1;
    frame(app, ctx, key(k), *time);
    *time += 0.1;
    frame(app, ctx, Vec::new(), *time);
}

/// Home, then Tab: New drawing, Open..., then for each recent drawing its row,
/// Pin and More.
#[test]
fn tab_reaches_each_recent_drawing_then_its_pin_and_menu() {
    let (mut app, ctx) = on_home();
    let mut time = 0.0;
    frame(&mut app, &ctx, Vec::new(), time);
    let mut order = Vec::new();
    for _ in 0..8 {
        press(&mut app, &ctx, Key::Tab, &mut time);
        order.push(ctx.memory(|m| m.focused()));
    }
    assert!(order.iter().all(Option::is_some));
    let distinct: std::collections::HashSet<_> = order.iter().collect();
    assert_eq!(distinct.len(), 8, "eight stops, none repeated: {order:?}");
}

/// The Tab stop whose Enter pins a drawing, for each drawing.
fn pin_stops() -> Vec<(usize, PathBuf)> {
    let mut found = Vec::new();
    for stops in 1..=12 {
        let (mut app, ctx) = on_home();
        let mut time = 0.0;
        frame(&mut app, &ctx, Vec::new(), time);
        for _ in 0..stops {
            press(&mut app, &ctx, Key::Tab, &mut time);
        }
        press(&mut app, &ctx, Key::Enter, &mut time);
        let pinned: Vec<_> = app
            .state
            .recent
            .items()
            .iter()
            .filter(|i| i.pinned)
            .collect();
        if let [one] = pinned[..] {
            found.push((stops, one.path.clone()));
        }
    }
    found
}

#[test]
fn each_drawing_has_its_pin_three_stops_after_the_last_and_enter_works_there() {
    let stops = pin_stops();
    assert_eq!(stops.len(), 2, "{stops:?}");
    // Newest first: second.odx, then first.odx; row, Pin, More for each.
    assert_eq!(stops[0].1, PathBuf::from("second.odx"));
    assert_eq!(stops[1].1, PathBuf::from("first.odx"));
    assert_eq!(stops[1].0, stops[0].0 + 3);
}

#[test]
fn focus_stays_with_a_drawing_when_pinning_moves_it() {
    let stops = pin_stops();
    let (mut app, ctx) = on_home();
    let mut time = 0.0;
    frame(&mut app, &ctx, Vec::new(), time);
    for _ in 0..stops[1].0 {
        press(&mut app, &ctx, Key::Tab, &mut time);
    }
    let focused = ctx.memory(|m| m.focused());
    time += 0.1;
    frame(&mut app, &ctx, key(Key::Enter), time);
    time += 0.1;
    frame(&mut app, &ctx, Vec::new(), time);
    assert_eq!(app.state.recent.items()[0].path, PathBuf::from("first.odx"));
    assert_eq!(ctx.memory(|m| m.focused()), focused, "focus did not move");
    // Enter again acts on the same drawing, now unpinning it.
    press(&mut app, &ctx, Key::Enter, &mut time);
    assert_eq!(app.state.recent.pinned_count(), 0);
}

/// How many Tab stops Home has, found by tabbing until focus comes round.
fn tab_cycle(app: &mut OpenDraftApp) -> usize {
    let ctx = egui::Context::default();
    let mut time = 0.0;
    frame(app, &ctx, Vec::new(), time);
    let mut seen = Vec::new();
    for _ in 0..40 {
        press(app, &ctx, Key::Tab, &mut time);
        let focused = ctx.memory(|m| m.focused());
        if let Some(at) = seen.iter().position(|f| *f == focused) {
            return seen.len() - at;
        }
        seen.push(focused);
    }
    panic!("focus never came round");
}

fn plain_home(dismissed: bool) -> OpenDraftApp {
    let mut app = app();
    app.state.start_on_home();
    if dismissed {
        app.state
            .recent
            .dismiss_whats_new(env!("CARGO_PKG_VERSION"));
    }
    app
}

#[test]
fn the_whats_new_note_is_two_tab_stops_that_dismiss_it_and_it_stays_dismissed() {
    // It has a note for this version (see `whats_new.rs`); if a release leaves
    // it out on purpose, there is nothing to test.
    if crate::whats_new::current().is_none() {
        return;
    }
    let with = tab_cycle(&mut plain_home(false));
    let without = tab_cycle(&mut plain_home(true));
    assert_eq!(with, without + 2, "View release notes and Dismiss");
    // Enter on the last of them dismisses it; nothing else does.
    let mut dismissing = Vec::new();
    for stops in 1..=with {
        let mut app = plain_home(false);
        let ctx = egui::Context::default();
        let mut time = 0.0;
        frame(&mut app, &ctx, Vec::new(), time);
        for _ in 0..stops {
            press(&mut app, &ctx, Key::Tab, &mut time);
        }
        press(&mut app, &ctx, Key::Enter, &mut time);
        if app.state.recent.whats_new_seen(env!("CARGO_PKG_VERSION")) {
            dismissing.push(stops);
        }
    }
    assert_eq!(dismissing.len(), 1, "{dismissing:?}");
}
