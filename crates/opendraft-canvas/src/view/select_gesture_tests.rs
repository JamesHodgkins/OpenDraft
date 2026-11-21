//! The selection styles: a click on nothing and a drag, by style.
//!
//! The default camera shows (150, 100) at the middle of the 600 x 400
//! screen, two screen points to a unit; the one circle sits there with
//! radius 10 (20 points on screen).

use std::collections::BTreeSet;

use egui::{pos2, Context, Event, Key, Modifiers, Pos2, RawInput};
use opendraft_core::{Document, Entity, EntityId, Point};

use super::input_tests::{button, style, SCREEN};
use super::select_gesture::{LassoKind, Pending};
use super::{CanvasOutput, GridSettings, SelectionStyle, View};
use crate::select::Region;
use crate::snap::SnapModes;

const MIDDLE: Pos2 = pos2(300.0, 200.0);

fn circle() -> Document {
    Document::from_entities([Entity::Circle {
        center: Point::new(150.0, 100.0),
        radius: 10.0,
    }])
}

fn key(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

/// A frame in which the app first gives the canvas its selection keys.
fn frame(view: &mut View, ctx: &Context, doc: &Document, events: Vec<Event>) -> CanvasOutput {
    let mut result = None;
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(SCREEN),
            events,
            ..RawInput::default()
        },
        |ctx| {
            view.take_selection_keys(ctx);
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    result = Some(view.show(
                        ui,
                        &style(),
                        &GridSettings {
                            visible: false,
                            spacing: 10.0,
                        },
                        doc,
                        &BTreeSet::new(),
                        &SnapModes::default(),
                    ));
                });
        },
    );
    result.expect("the canvas was shown")
}

fn click(view: &mut View, ctx: &Context, doc: &Document, at: Pos2) -> CanvasOutput {
    frame(view, ctx, doc, vec![Event::PointerMoved(at)]);
    frame(view, ctx, doc, vec![button(at, true)]);
    frame(view, ctx, doc, vec![button(at, false)])
}

/// Press at the first point, move through the rest and release at the last.
fn drag(view: &mut View, ctx: &Context, doc: &Document, path: &[Pos2]) -> CanvasOutput {
    frame(view, ctx, doc, vec![Event::PointerMoved(path[0])]);
    frame(view, ctx, doc, vec![button(path[0], true)]);
    for at in &path[1..] {
        frame(view, ctx, doc, vec![Event::PointerMoved(*at)]);
    }
    let last = path[path.len() - 1];
    frame(view, ctx, doc, vec![button(last, false)])
}

fn lasso_style() -> View {
    let mut view = View::default();
    view.set_selection_style(SelectionStyle::WindowLasso);
    view
}

/// A lasso around the middle: off to the right first, or to the left.
fn around(rightwards: bool) -> Vec<Pos2> {
    let s = if rightwards { 1.0 } else { -1.0 };
    vec![
        pos2(300.0, 140.0),
        pos2(300.0 + 60.0 * s, 150.0),
        pos2(300.0 + 60.0 * s, 250.0),
        pos2(300.0 - 60.0 * s, 250.0),
        pos2(300.0 - 60.0 * s, 150.0),
        pos2(295.0, 141.0),
    ]
}

#[test]
fn click_clears_reports_a_click_on_nothing_and_drags_a_box() {
    let (ctx, doc, mut view) = (Context::default(), circle(), View::default());
    let out = click(&mut view, &ctx, &doc, pos2(100.0, 100.0));
    assert_eq!(
        out.click.map(|c| c.entity),
        Some(None),
        "for the app to clear"
    );
    assert!(!view.selection_pending());
    let out = drag(
        &mut view,
        &ctx,
        &doc,
        &[pos2(250.0, 150.0), pos2(350.0, 250.0)],
    );
    assert!(matches!(
        out.select.map(|s| s.region),
        Some(Region::Window(_))
    ));
}

#[test]
fn lasso_style_a_click_on_nothing_starts_a_window_the_next_click_closes() {
    let (ctx, doc, mut view) = (Context::default(), circle(), lasso_style());
    let out = click(&mut view, &ctx, &doc, pos2(250.0, 150.0));
    assert!(out.click.is_none(), "nothing to clear");
    assert!(view.selection_pending());
    let out = click(&mut view, &ctx, &doc, pos2(350.0, 250.0));
    let region = out.select.expect("the window closed").region;
    assert!(matches!(region, Region::Window(_)), "drawn to the right");
    assert!(out.click.is_none());
    assert!(!view.selection_pending());
    assert_eq!(crate::select_in(&doc, &region), [EntityId(0)]);

    // To the left it is a crossing window, which takes what it touches.
    click(&mut view, &ctx, &doc, pos2(305.0, 150.0));
    let out = click(&mut view, &ctx, &doc, pos2(200.0, 250.0));
    let region = out.select.expect("the window closed").region;
    assert!(matches!(region, Region::Crossing(_)));
    assert_eq!(crate::select_in(&doc, &region), [EntityId(0)]);
}

#[test]
fn lasso_style_a_click_on_an_object_picks_it() {
    let (ctx, doc, mut view) = (Context::default(), circle(), lasso_style());
    let out = click(&mut view, &ctx, &doc, pos2(320.0, 200.0));
    assert_eq!(out.click.and_then(|c| c.entity), Some(EntityId(0)));
    assert!(!view.selection_pending());
}

#[test]
fn lasso_style_a_drag_is_a_lasso_window_to_the_right_and_crossing_to_the_left() {
    let (ctx, doc, mut view) = (Context::default(), circle(), lasso_style());
    let out = drag(&mut view, &ctx, &doc, &around(true));
    let region = out.select.expect("a lasso").region;
    assert!(matches!(region, Region::Window(ref p) if p.len() >= 5));
    assert_eq!(crate::select_in(&doc, &region), [EntityId(0)]);

    let out = drag(&mut view, &ctx, &doc, &around(false));
    assert!(matches!(
        out.select.map(|s| s.region),
        Some(Region::Crossing(_))
    ));
    assert!(!view.selection_pending());
}

#[test]
fn lasso_style_space_cycles_the_lasso_and_esc_drops_it() {
    let (ctx, doc, mut view) = (Context::default(), circle(), lasso_style());
    let path = [pos2(200.0, 200.0), pos2(250.0, 205.0), pos2(260.0, 210.0)];
    frame(&mut view, &ctx, &doc, vec![Event::PointerMoved(path[0])]);
    frame(&mut view, &ctx, &doc, vec![button(path[0], true)]);
    for at in &path[1..] {
        frame(&mut view, &ctx, &doc, vec![Event::PointerMoved(*at)]);
    }
    let kind = |view: &View| match &view.pending {
        Some(Pending::Lasso { kind, .. }) => *kind,
        _ => None,
    };
    assert_eq!(kind(&view), Some(LassoKind::Window));
    frame(&mut view, &ctx, &doc, vec![key(Key::Space)]);
    frame(&mut view, &ctx, &doc, vec![key(Key::Space)]);
    assert_eq!(kind(&view), Some(LassoKind::Fence));
    // The fence crosses the circle.
    frame(&mut view, &ctx, &doc, vec![Event::PointerMoved(MIDDLE)]);
    let out = frame(&mut view, &ctx, &doc, vec![button(MIDDLE, false)]);
    let region = out.select.expect("a fence").region;
    assert!(matches!(region, Region::Fence(_)));
    assert_eq!(crate::select_in(&doc, &region), [EntityId(0)]);

    // Esc halfway through a lasso: the release selects nothing.
    frame(&mut view, &ctx, &doc, vec![button(path[0], true)]);
    frame(&mut view, &ctx, &doc, vec![Event::PointerMoved(path[1])]);
    assert!(view.selection_pending());
    frame(&mut view, &ctx, &doc, vec![key(Key::Escape)]);
    assert!(!view.selection_pending());
    frame(&mut view, &ctx, &doc, vec![Event::PointerMoved(path[2])]);
    let out = frame(&mut view, &ctx, &doc, vec![button(path[2], false)]);
    assert!(out.select.is_none() && !view.selection_pending());
}

#[test]
fn lasso_style_esc_or_a_command_drops_an_implied_window() {
    let (ctx, doc, mut view) = (Context::default(), circle(), lasso_style());
    click(&mut view, &ctx, &doc, pos2(100.0, 100.0));
    frame(&mut view, &ctx, &doc, vec![key(Key::Escape)]);
    assert!(!view.selection_pending());

    click(&mut view, &ctx, &doc, pos2(100.0, 100.0));
    view.set_point_prompt(Some(crate::PointPrompt::default()));
    frame(&mut view, &ctx, &doc, vec![]);
    assert!(!view.selection_pending(), "a prompt takes the clicks");

    // Switching back to the default style drops one too.
    view.set_point_prompt(None);
    click(&mut view, &ctx, &doc, pos2(100.0, 100.0));
    view.set_selection_style(SelectionStyle::ClickClears);
    assert!(!view.selection_pending());
}
