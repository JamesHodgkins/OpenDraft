//! The drawing cursor: AutoCAD's full crosshair and pickbox, painted in
//! place of the system cursor over the canvas.

use egui::{Color32, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};

use od_core::Document;

use super::types::{Crosshair, GridSettings, PointerSizes};
use super::{View, GRID_MIN_PX, WHEEL_ZOOM_RATE};
use crate::grid::grid_step;
use crate::snap::{SnapModes, SnapOverride};
use crate::snap_axis::Aim;

/// The crosshair's colour unless one is chosen: off-white, not so bright as
/// to glare on a dark background.
pub const DEFAULT_COLOR: Color32 = Color32::from_gray(215);

/// Which parts of the cursor the canvas shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CursorShape {
    /// Waiting for a point: the crosshair alone.
    Crosshair,
    /// Nothing running: the crosshair with the pickbox on it.
    Both,
    /// Waiting for an object to be clicked: the pickbox alone.
    PickBox,
}

/// How far each arm reaches from the centre. A size of 100 spans the whole
/// canvas in both directions wherever the cursor is; below that every arm is
/// the same length, that share of the canvas's longer side (so the cross is
/// square, not stretched with the window).
pub(super) fn arms(size: u32, canvas: Vec2) -> Vec2 {
    if size >= 100 {
        canvas
    } else {
        Vec2::splat(canvas.max_elem() * (size as f32 / 200.0))
    }
}

fn paint(
    painter: &Painter,
    canvas: Rect,
    at: Pos2,
    shape: CursorShape,
    crosshair: &Crosshair,
    pick: f64,
) {
    let stroke = Stroke::new(1.0_f32, crosshair.color.unwrap_or(DEFAULT_COLOR));
    if shape != CursorShape::PickBox {
        let arm = arms(crosshair.size, canvas.size());
        painter.hline((at.x - arm.x)..=(at.x + arm.x), at.y, stroke);
        painter.vline(at.x, (at.y - arm.y)..=(at.y + arm.y), stroke);
    }
    if shape != CursorShape::Crosshair {
        let half = pick as f32;
        let square = Rect::from_center_size(at, Vec2::splat(2.0 * half));
        painter.rect_stroke(square, 0.0, stroke, StrokeKind::Middle);
    }
}

impl View {
    pub fn set_pointer_sizes(&mut self, sizes: PointerSizes) {
        self.sizes = sizes;
    }

    /// Hold points to the horizontal or vertical from a prompt's base point.
    pub fn set_ortho(&mut self, ortho: crate::snap_axis::Ortho) {
        self.ortho = ortho;
    }

    /// What the pointer snaps to this frame. Ortho holds points to the axis
    /// from the prompt's base, unless a pick is wanted; Shift flips it.
    pub(super) fn aim<'a>(
        &self,
        document: &'a Document,
        modes: &SnapModes,
        grid: &GridSettings,
        picking: bool,
        shift: bool,
    ) -> Aim<'a> {
        let held = !picking && self.ortho.enabled != shift;
        Aim {
            document,
            modes: self
                .snap_cycle
                .over
                .as_ref()
                .map_or(*modes, |o| o.applied(modes)),
            step: grid_step(grid.spacing, self.camera.zoom, GRID_MIN_PX),
            radius: self.sizes.snap / self.camera.zoom,
            ortho: self.ortho,
            base: self
                .point_prompt
                .as_ref()
                .and_then(|p| p.base)
                .filter(|_| held),
            from: self.point_prompt.as_ref().and_then(|p| p.base),
            chosen: None,
            tracks: Vec::new(),
            track: None,
            extra: self.snap_extra(),
            insist: self
                .snap_cycle
                .over
                .as_ref()
                .is_some_and(SnapOverride::insists),
        }
    }

    /// The prompt's extra snap geometry as objects, for the snap pass.
    fn snap_extra(&self) -> std::sync::Arc<[od_core::Object]> {
        let Some(prompt) = &self.point_prompt else {
            return Default::default();
        };
        (prompt.snap_geometry.iter().enumerate())
            .map(|(i, entity)| {
                let id = od_core::EntityId(u64::MAX - i as u64);
                let attrs = od_core::Attributes::on_layer(od_core::LayerId::DEFAULT);
                od_core::Object::new(id, entity.clone(), attrs, None)
            })
            .collect()
    }

    pub fn set_crosshair(&mut self, crosshair: Crosshair) {
        self.crosshair = crosshair;
    }

    /// Set the wheel zoom speed, in percent of the base rate (100 is normal).
    pub fn set_zoom_speed(&mut self, percent: u32) {
        self.zoom_speed = Some(percent as f32 / 100.0);
    }

    pub(super) fn wheel_rate(&self) -> f32 {
        WHEEL_ZOOM_RATE * self.zoom_speed.unwrap_or(1.0)
    }

    /// Hide the system cursor and draw ours at `at` (screen position).
    pub(super) fn paint_cursor(&self, ui: &egui::Ui, canvas: Rect, at: Pos2, shape: CursorShape) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
        let painter = ui.painter_at(canvas);
        paint(
            &painter,
            canvas,
            at,
            shape,
            &self.crosshair,
            self.sizes.pick,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arms_are_equal_and_a_hundred_spans_the_canvas() {
        let canvas = Vec2::new(1000.0, 600.0);
        assert_eq!(arms(5, canvas), Vec2::splat(25.0));
        assert_eq!(arms(50, canvas), Vec2::splat(250.0));
        assert_eq!(
            arms(100, canvas),
            canvas,
            "reaches either edge from anywhere"
        );
    }
}
