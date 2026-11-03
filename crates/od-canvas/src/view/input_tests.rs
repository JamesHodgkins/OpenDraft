//! Pointer ownership between the canvas and UI overlays drawn over it.

use std::collections::BTreeSet;

use egui::{pos2, Color32, Context, Event, PointerButton, RawInput, Rect};
use od_core::Document;

use super::{CanvasOutput, CanvasStyle, GridSettings, View};
use crate::snap::SnapModes;

const SCREEN: Rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(600.0, 400.0));

fn style() -> CanvasStyle {
    CanvasStyle {
        background: Color32::BLACK,
        grid_minor: Color32::DARK_GRAY,
        grid_major: Color32::GRAY,
        axis_x: Color32::RED,
        axis_y: Color32::GREEN,
        entity: Color32::WHITE,
        hover: Color32::LIGHT_BLUE,
        selected: Color32::BLUE,
        selected_glow: Color32::BLUE,
        snap: Color32::YELLOW,
        window: Color32::BLUE,
        crossing: Color32::GREEN,
    }
}

fn frame(view: &mut View, ctx: &Context, events: Vec<Event>) -> CanvasOutput {
    let mut result = None;
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(SCREEN),
            events,
            ..RawInput::default()
        },
        |ctx| {
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
                        &Document::default(),
                        &BTreeSet::new(),
                        &SnapModes::default(),
                    ));
                });
        },
    );
    result.expect("the canvas was shown")
}

/// A frame with a dialog-like window standing over the middle of the canvas.
fn frame_under_window(view: &mut View, ctx: &Context, events: Vec<Event>) -> CanvasOutput {
    let mut result = None;
    let _ = ctx.run(
        RawInput {
            screen_rect: Some(SCREEN),
            events,
            ..RawInput::default()
        },
        |ctx| {
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
                        &Document::default(),
                        &BTreeSet::new(),
                        &SnapModes::default(),
                    ));
                });
            egui::Window::new("dialog")
                .fixed_pos(pos2(250.0, 150.0))
                .fixed_size(egui::vec2(100.0, 100.0))
                .show(ctx, |ui| {
                    ui.label("Plot");
                });
        },
    );
    result.expect("the canvas was shown")
}

#[test]
fn the_canvas_has_no_cursor_under_a_window_above_it() {
    let ctx = Context::default();
    let mut view = View::default();
    // The window is known to egui from the frame after it first appears.
    frame_under_window(&mut view, &ctx, vec![]);
    frame_under_window(&mut view, &ctx, vec![]);

    let beside = frame_under_window(&mut view, &ctx, vec![Event::PointerMoved(pos2(50.0, 50.0))]);
    assert!(beside.cursor.is_some(), "the open canvas still has one");
    let under = frame_under_window(
        &mut view,
        &ctx,
        vec![Event::PointerMoved(pos2(300.0, 200.0))],
    );
    assert!(under.cursor.is_none(), "the dialog owns the pointer there");
}

fn button(at: egui::Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn a_drag_that_begins_in_an_input_exclusion_does_not_select() {
    let ctx = Context::default();
    let mut view = View::default();
    view.set_input_exclusions(vec![SCREEN]);
    let (from, to) = (pos2(100.0, 100.0), pos2(300.0, 250.0));

    frame(&mut view, &ctx, vec![Event::PointerMoved(from)]);
    frame(&mut view, &ctx, vec![button(from, true)]);
    frame(&mut view, &ctx, vec![Event::PointerMoved(to)]);
    assert!(
        view.drag.is_none(),
        "the canvas did not take the overlay's drag"
    );
    let released = frame(&mut view, &ctx, vec![button(to, false)]);
    assert!(released.select.is_none());
}

#[test]
fn an_unblocked_canvas_drag_still_selects() {
    let ctx = Context::default();
    let mut view = View::default();
    let (from, to) = (pos2(100.0, 100.0), pos2(300.0, 250.0));

    frame(&mut view, &ctx, vec![Event::PointerMoved(from)]);
    frame(&mut view, &ctx, vec![button(from, true)]);
    frame(&mut view, &ctx, vec![Event::PointerMoved(to)]);
    assert!(view.drag.is_some());
    let released = frame(&mut view, &ctx, vec![button(to, false)]);
    assert!(released.select.is_some());
}
