//! The editor on screen: the draft painted over the drawing in its own
//! place, the caret and selection, pointer editing, the input-method
//! position and a small toolbar. The draft is drawn from the same layout the
//! drawing uses, so what is edited is what will be placed.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Shape, Stroke};
use od_canvas::{paint_text, Camera, CanvasStyle};
use od_core::aci::aci_rgb;
use od_core::text::{TextColor, TextLayout};
use od_core::MText;
use od_ui::Theme;

use super::input::{handle_events, Outcome};
use super::state::MTextEditor;

/// The caret is on for this long, then off for as long again.
const BLINK_SECONDS: f64 = 0.5;
/// Space around the text inside the editor's box, in text heights.
const PADDING: f64 = 0.3;

/// The corners of the padded box round the text, on screen.
fn box_corners(
    text: &MText,
    layout: &TextLayout,
    height: f64,
    camera: &Camera,
    canvas: Rect,
) -> Vec<Pos2> {
    let pad = PADDING * text.nominal_height;
    // A word wider than the wrapping width runs past it, and stays editable.
    let (w, h) = (layout.frame_width.max(layout.actual_width), height);
    [
        (-pad, pad),
        (w + pad, pad),
        (w + pad, -h - pad),
        (-pad, -h - pad),
    ]
    .iter()
    .map(|&(x, y)| camera.world_to_screen(text.world_point(layout, x, y), canvas))
    .collect()
}

/// Show the editor over the canvas `canvas` and handle the frame's input.
/// Returns how it ended, if it did.
pub fn show(
    ui: &mut egui::Ui,
    canvas: Rect,
    camera: &Camera,
    style: &CanvasStyle,
    theme: &Theme,
    editor: &mut MTextEditor,
    styles: &[od_core::TextStyle],
) -> Option<Outcome> {
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);

    // Keyboard and pointer first, so what is painted is the new state.
    let pad = PADDING * editor.text().nominal_height;
    let on_grip = super::box_handles::interact(&ctx, editor, camera, canvas, pad, now);
    let bounds = {
        let (text, layout) = (editor.text(), editor.layout());
        Rect::from_points(&box_corners(
            text,
            layout,
            editor.box_height(),
            camera,
            canvas,
        ))
    };
    let id = egui::Id::new("mtext_editor");
    let response = ui.interact(bounds, id, Sense::click_and_drag());
    // A number field in the toolbar or dialog keeps the keyboard while it is
    // being typed in; the text takes it back when that ends.
    let typing_elsewhere = ctx.wants_keyboard_input() && !response.has_focus();
    if !response.has_focus() && !typing_elsewhere {
        response.request_focus();
    }
    ctx.memory_mut(|m| {
        m.set_focus_lock_filter(
            id,
            egui::EventFilter {
                tab: true,
                horizontal_arrows: true,
                vertical_arrows: true,
                escape: true,
            },
        );
    });
    let pressed = ctx.input(|i| i.pointer.primary_pressed());
    let shift = ctx.input(|i| i.modifiers.shift);
    if response.hovered() && !on_grip {
        ctx.set_cursor_icon(egui::CursorIcon::Text);
    }
    let pointer = ctx.pointer_interact_pos().filter(|_| !on_grip);
    if let Some(pos) = pointer {
        let (x, y) = {
            let (text, layout) = (editor.text(), editor.layout());
            text.layout_point(layout, camera.screen_to_world(pos, canvas))
        };
        if response.triple_clicked() {
            editor.select_line_at(x, y);
        } else if response.double_clicked() {
            editor.select_word_at(x, y);
        } else if pressed && bounds.contains(pos) {
            editor.place_at(x, y, shift);
            editor.active_since = now;
        } else if response.dragged() {
            editor.place_at(x, y, true);
        }
    }
    let events = if typing_elsewhere {
        Vec::new()
    } else {
        ctx.input(|i| i.events.clone())
    };
    let handled = handle_events(editor, &events, now);
    if let Some(copied) = handled.copied {
        ctx.copy_text(copied);
    }
    if handled.touched {
        editor.active_since = now;
    }
    let mut outcome = handled.outcome;

    // The ruler works on the paragraph before the draft is painted.
    let ruler = {
        let (text, layout) = (editor.text().clone(), editor.layout().clone());
        let to = |x: f64, y: f64| camera.world_to_screen(text.world_point(&layout, x, y), canvas);
        super::ruler::show(
            ui,
            theme,
            editor,
            &to,
            pad,
            (-pad, layout.frame_width.max(layout.actual_width) + pad),
            now,
        )
    };

    // Paint the draft from the state after the input.
    let text = editor.text().clone();
    let layout = editor.layout().clone();
    let to_screen = |x: f64, y: f64| -> Pos2 {
        camera.world_to_screen(text.world_point(&layout, x, y), canvas)
    };
    let fg = style.entity;
    let bg = style.background.lerp_to_gamma(fg, 0.1);
    let painter = ui.painter_at(canvas);
    painter.add(Shape::convex_polygon(
        box_corners(&text, &layout, editor.box_height(), camera, canvas),
        bg,
        Stroke::new(1.0_f32, style.selected),
    ));
    super::box_handles::paint(&painter, editor, camera, canvas, pad, style.selected);
    let (from, to) = editor.selection();
    let highlight = style.selected.gamma_multiply(0.45);
    for r in layout.selection_rects(from, to) {
        let quad = [
            (r.x0, r.top),
            (r.x1, r.top),
            (r.x1, r.bottom),
            (r.x0, r.bottom),
        ]
        .iter()
        .map(|&(x, y)| to_screen(x, y))
        .collect();
        painter.add(Shape::convex_polygon(quad, highlight, Stroke::NONE));
    }
    let color = |c: TextColor| match c {
        TextColor::Entity | TextColor::Aci(7) => fg,
        TextColor::Aci(n) => {
            let (r, g, b) = aci_rgb(u8::try_from(n).unwrap_or(7));
            Color32::from_rgb(r, g, b)
        }
        TextColor::Rgb(v) => Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8),
    };
    paint_text(&painter, camera, canvas, &text, &layout, &color);

    // The caret, and where an input method should put its window.
    let caret = layout
        .caret_rect(editor.caret())
        .map(|(x, bottom, top)| (to_screen(x, bottom), to_screen(x, top)));
    if let Some((a, b)) = caret {
        let solid = now - editor.active_since < BLINK_SECONDS;
        let on = solid || ((now / BLINK_SECONDS) as u64).is_multiple_of(2);
        if on && !editor.has_selection() {
            painter.line_segment([a, b], Stroke::new(1.5_f32, fg));
        }
        let cursor = Rect::from_two_pos(a, b).expand2(egui::vec2(1.0, 0.0));
        ctx.output_mut(|o| {
            o.ime = Some(egui::output::IMEOutput {
                rect: bounds,
                cursor_rect: cursor,
            });
        });
        if let Some(composing) = &editor.composing {
            painter.text(
                b + egui::vec2(2.0, 0.0),
                egui::Align2::LEFT_TOP,
                composing,
                egui::FontId::proportional(14.0),
                fg,
            );
        }
    }
    ctx.request_repaint_after(std::time::Duration::from_millis(250));

    let (chosen, tools) =
        super::toolbar::show(&ctx, theme, editor, styles, bounds.union(ruler), canvas);
    outcome = outcome.or(chosen);
    let dialog = super::paragraph_dialog::show(&ctx, theme, editor, tools, now);

    // A click on the drawing outside the box and the toolbar accepts.
    if pressed {
        if let Some(pos) = ctx.pointer_interact_pos() {
            if canvas.contains(pos)
                && !on_grip
                && !bounds.contains(pos)
                && !tools.contains(pos)
                && !ruler.contains(pos)
                && !dialog.is_some_and(|d| d.contains(pos))
                && over_drawing(&ctx, pos)
            {
                outcome = outcome.or(Some(Outcome::Accept));
            }
        }
    }
    outcome
}

/// Nothing but the drawing is under `pos`: not a menu, a dialog or another
/// window drawn over it.
fn over_drawing(ctx: &egui::Context, pos: Pos2) -> bool {
    ctx.layer_id_at(pos)
        .is_none_or(|layer| layer.order == egui::Order::Background)
}
