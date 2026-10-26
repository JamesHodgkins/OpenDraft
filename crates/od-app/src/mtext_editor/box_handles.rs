//! The grips on the editor's box: the right edge sets the wrapping width, the
//! bottom edge the box height, the corner both. The height is the editor's
//! own (the box never shrinks below its text), kept while the text changes.

use eframe::egui::{self, Color32, Pos2, Rect, Stroke};
use od_canvas::Camera;

use super::state::MTextEditor;

/// How near a grip a press must be, and the size drawn, in pixels.
const GRAB_PX: f32 = 8.0;
const DRAWN_PX: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Handle {
    Width,
    Height,
    Both,
}

/// The box's padding and size in local units: `(pad, width, height)`.
pub fn extent(editor: &MTextEditor, pad: f64) -> (f64, f64, f64) {
    let layout = editor.layout();
    (
        pad,
        layout.frame_width.max(layout.actual_width),
        editor.box_height(),
    )
}

/// Where each grip sits on screen.
fn spots(editor: &MTextEditor, camera: &Camera, canvas: Rect, pad: f64) -> [(Handle, Pos2); 3] {
    let (pad, w, h) = extent(editor, pad);
    let (text, layout) = (editor.text(), editor.layout());
    let at = |x: f64, y: f64| camera.world_to_screen(text.world_point(layout, x, y), canvas);
    [
        (Handle::Width, at(w + pad, -h / 2.0)),
        (Handle::Height, at(w / 2.0, -h - pad)),
        (Handle::Both, at(w + pad, -h - pad)),
    ]
}

/// Take a press on a grip and follow its drag. Returns whether the pointer is
/// on a grip or dragging one, so the box below it does not take the click.
pub fn interact(
    ctx: &egui::Context,
    editor: &mut MTextEditor,
    camera: &Camera,
    canvas: Rect,
    pad: f64,
    now: f64,
) -> bool {
    let id = egui::Id::new("mtext_box_grab");
    let (pressed, down, pointer) = ctx.input(|i| {
        (
            i.pointer.primary_pressed(),
            i.pointer.primary_down(),
            i.pointer.interact_pos(),
        )
    });
    let mut held: Option<Handle> = ctx.data(|d| d.get_temp(id)).filter(|_| down);
    let near = |editor: &MTextEditor, pos: Pos2| {
        spots(editor, camera, canvas, pad)
            .into_iter()
            .filter(|(_, p)| p.distance(pos) <= GRAB_PX)
            .min_by(|a, b| a.1.distance(pos).total_cmp(&b.1.distance(pos)))
            .map(|(h, _)| h)
    };
    let hover = pointer.and_then(|p| near(editor, p));
    if let (true, Some(h)) = (pressed, hover) {
        held = Some(h);
    }
    if let (Some(h), Some(pos)) = (held, pointer) {
        let (x, y) = {
            let (text, layout) = (editor.text(), editor.layout());
            text.layout_point(layout, camera.screen_to_world(pos, canvas))
        };
        if h != Handle::Height {
            let min = editor.text().nominal_height;
            editor.set_width((x - pad).max(min), true, now);
        }
        if h != Handle::Width {
            editor.set_box_height(-y - pad);
        }
        editor.active_since = now;
    }
    ctx.data_mut(|d| match held {
        Some(h) => d.insert_temp(id, h),
        None => d.remove::<Handle>(id),
    });
    match held.or(hover) {
        Some(Handle::Width) => ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal),
        Some(Handle::Height) => ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical),
        Some(Handle::Both) => ctx.set_cursor_icon(egui::CursorIcon::ResizeNwSe),
        None => {}
    }
    held.is_some() || hover.is_some()
}

pub fn paint(
    painter: &egui::Painter,
    editor: &MTextEditor,
    camera: &Camera,
    canvas: Rect,
    pad: f64,
    colour: Color32,
) {
    for (_, p) in spots(editor, camera, canvas, pad) {
        let square = Rect::from_center_size(p, egui::vec2(2.0 * DRAWN_PX, 2.0 * DRAWN_PX));
        painter.rect_filled(square, 0.0, Color32::WHITE);
        painter.rect_stroke(
            square,
            0.0,
            Stroke::new(1.0_f32, colour),
            egui::StrokeKind::Middle,
        );
    }
}
