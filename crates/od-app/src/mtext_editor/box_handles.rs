//! The grips on the editor's box: the right edge sets the wrapping width, the
//! bottom edge the box height, the corner both. The height is the editor's
//! own (the box never shrinks below its text), kept while the text changes.

use eframe::egui::{self, Color32, Pos2, Rect, Stroke};
use od_canvas::Camera;

use super::state::MTextEditor;

/// How near an edge or grip a press must be, and the size drawn, in pixels.
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

/// The two resize edges on screen: right first, then bottom. Keeping this in
/// screen space gives a constant-size target at every zoom and makes the hit
/// area follow rotated text correctly.
fn edges(
    editor: &MTextEditor,
    camera: &Camera,
    canvas: Rect,
    pad: f64,
) -> [(Handle, [Pos2; 2]); 2] {
    let (pad, w, h) = extent(editor, pad);
    let (text, layout) = (editor.text(), editor.layout());
    let at = |x: f64, y: f64| camera.world_to_screen(text.world_point(layout, x, y), canvas);
    [
        (Handle::Width, [at(w + pad, pad), at(w + pad, -h - pad)]),
        (Handle::Height, [at(-pad, -h - pad), at(w + pad, -h - pad)]),
    ]
}

fn distance_to_segment(pos: Pos2, [a, b]: [Pos2; 2]) -> f32 {
    let along = b - a;
    let length_sq = along.length_sq();
    if length_sq == 0.0 {
        return pos.distance(a);
    }
    let t = ((pos - a).dot(along) / length_sq).clamp(0.0, 1.0);
    pos.distance(a + t * along)
}

fn hit(editor: &MTextEditor, camera: &Camera, canvas: Rect, pad: f64, pos: Pos2) -> Option<Handle> {
    let [(Handle::Width, right), (Handle::Height, bottom)] = edges(editor, camera, canvas, pad)
    else {
        unreachable!("the editor always has a right and bottom resize edge")
    };
    let right = distance_to_segment(pos, right);
    let bottom = distance_to_segment(pos, bottom);
    match (right <= GRAB_PX, bottom <= GRAB_PX) {
        (true, true) => Some(Handle::Both),
        (true, false) => Some(Handle::Width),
        (false, true) => Some(Handle::Height),
        (false, false) => None,
    }
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
    let stored: Option<Handle> = ctx.data(|d| d.get_temp(id));
    let mut held = stored.filter(|_| down);
    let hover = pointer.and_then(|p| hit(editor, camera, canvas, pad, p));
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
    // A handle is painted outside the text itself. Force a fresh overlay
    // while it moves and once more on release, otherwise the old accent-
    // coloured square can survive in a backend's unchanged damage region.
    if held.is_some() || stored.is_some() {
        ctx.request_repaint();
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use od_core::{MText, Point, TextStyle, TextStyleId};

    const CANVAS: Rect = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(800.0, 600.0));

    fn fixture(rotation: f64) -> (MTextEditor, Camera, f64) {
        let mut text = MText::new(
            Point::new(150.0, 100.0),
            10.0,
            "one two three",
            TextStyleId::STANDARD,
        );
        text.reference_width = 80.0;
        text.direction = Point::new(rotation.cos(), rotation.sin());
        let editor = MTextEditor::new(text, TextStyle::standard());
        (editor, Camera::default(), 3.0)
    }

    #[test]
    fn every_point_on_the_right_and_bottom_edges_resizes() {
        let (editor, camera, pad) = fixture(0.0);
        let [(Handle::Width, right), (Handle::Height, bottom)] =
            edges(&editor, &camera, CANVAS, pad)
        else {
            unreachable!()
        };
        for t in [0.15, 0.35, 0.65, 0.85] {
            assert_eq!(
                hit(&editor, &camera, CANVAS, pad, right[0].lerp(right[1], t)),
                Some(Handle::Width)
            );
            assert_eq!(
                hit(&editor, &camera, CANVAS, pad, bottom[0].lerp(bottom[1], t)),
                Some(Handle::Height)
            );
        }
        assert_eq!(
            hit(&editor, &camera, CANVAS, pad, right[1]),
            Some(Handle::Both)
        );
    }

    #[test]
    fn edge_hit_testing_follows_rotated_text() {
        let (editor, camera, pad) = fixture(std::f64::consts::FRAC_PI_4);
        let [(Handle::Width, right), (Handle::Height, bottom)] =
            edges(&editor, &camera, CANVAS, pad)
        else {
            unreachable!()
        };
        assert_eq!(
            hit(&editor, &camera, CANVAS, pad, right[0].lerp(right[1], 0.4)),
            Some(Handle::Width)
        );
        assert_eq!(
            hit(
                &editor,
                &camera,
                CANVAS,
                pad,
                bottom[0].lerp(bottom[1], 0.4)
            ),
            Some(Handle::Height)
        );
    }
}
