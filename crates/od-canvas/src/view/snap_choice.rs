//! Which snap point the pointer ends up on: the best one under it, or the
//! one the user cycled to with Tab.

use egui::{Key, Modifiers, Ui};
use od_core::{Document, Point};

use super::{GridSettings, View};
use crate::hit::probe;
use crate::snap::{
    cycled, directions, targets, Acquired, Origin, SnapId, SnapKind, SnapModes, SnapOverride,
    SnapPoint, TrackInput, Tracking,
};
use crate::snap_axis::Aim;

/// Where the Tab cycling stands.
#[derive(Default)]
pub(super) struct Cycle {
    /// The snap point the user cycled to, while it stays under the pointer.
    pub chosen: Option<SnapId>,
    /// How many snap points were under the pointer last frame.
    pub count: usize,
    /// The one-shot override waiting for the next point.
    pub over: Option<SnapOverride>,
    /// Object snap tracking and polar tracking.
    pub tracking: Tracking,
    /// The canvas on screen this frame.
    pub rect: Option<egui::Rect>,
    /// Temporary tracking points (`TT`), which track like kept points.
    pub temps: Vec<Point>,
}

/// Whether pausing on a snap point of this kind keeps it as the start of
/// tracking paths.
fn trackable(kind: SnapKind) -> bool {
    matches!(
        kind,
        SnapKind::Endpoint
            | SnapKind::Midpoint
            | SnapKind::Center
            | SnapKind::Node
            | SnapKind::Quadrant
            | SnapKind::Intersection
            | SnapKind::ApparentIntersection
            | SnapKind::GeometricCenter
            | SnapKind::Insertion
    )
}

impl View {
    /// Set (or, with `None`, clear) the one-shot override for the next point.
    pub fn set_snap_override(&mut self, over: Option<SnapOverride>) {
        self.snap_cycle.over = over;
    }

    /// The temporary tracking points to follow, as set by `TT`.
    pub fn set_temp_tracking_points(&mut self, points: Vec<Point>) {
        self.snap_cycle.temps = points;
    }

    /// Set object snap tracking and polar tracking (F11, F10).
    pub fn set_tracking(&mut self, tracking: Tracking) {
        self.snap_cycle.tracking = tracking;
    }

    /// The right button was released over the canvas while a point is asked
    /// for: whether it means Enter, or (with Shift or Ctrl) that the snap
    /// override menu is wanted at that spot. Raw release, not `clicked_by`:
    /// egui drops a click whose pointer drifted past its threshold, so a
    /// slightly dragged right-click would never cancel.
    pub(super) fn right_release(&self, ui: &Ui, rect: egui::Rect) -> (bool, Option<egui::Pos2>) {
        let (at, menu) = self.right_released(ui, rect);
        (at.is_some() && !menu, at.filter(|_| menu))
    }

    /// Where the right button was released over the canvas this frame, and
    /// whether Shift or Ctrl was held.
    pub(super) fn right_released(&self, ui: &Ui, rect: egui::Rect) -> (Option<egui::Pos2>, bool) {
        let (at, menu) = ui.input(|i| {
            let at = i
                .pointer
                .button_released(egui::PointerButton::Secondary)
                .then(|| i.pointer.latest_pos())
                .flatten();
            (at, i.modifiers.shift || i.modifiers.ctrl)
        });
        (at.filter(|p| rect.contains(*p) && !self.blocked(*p)), menu)
    }

    /// Whether Tab would cycle between snap points under the pointer, so the
    /// app leaves the key to the canvas.
    pub fn snap_cycles(&self) -> bool {
        self.snap_cycle.count > 1
    }

    /// What the pointer at `cursor` snaps to while a command waits for a
    /// point, with the [`Aim`] that later clicks resolve by. Tab moves on to
    /// the next snap point under the pointer (Shift+Tab back); a choice the
    /// pointer has left behind is forgotten.
    pub(super) fn aim_at<'a>(
        &mut self,
        ui: &Ui,
        document: &'a Document,
        modes: &'a SnapModes,
        grid: &GridSettings,
        cursor: Option<Point>,
        shift: bool,
    ) -> (Aim<'a>, Option<(Point, Option<SnapPoint>)>) {
        let picking = self.point_prompt.as_ref().is_some_and(|p| p.pick);
        let mut aim = self.aim(document, modes, grid, picking, shift);
        let asking = cursor.filter(|_| self.point_prompt.is_some() && !picking);
        // What the pointer rests on is kept, and the paths of what is kept
        // join the snap points. Parallel needs a last point and no ortho.
        let parallel = aim.from.is_some() && aim.base.is_none();
        // An end is kept for Extension, and for Extended Intersection too.
        let wanted = SnapModes {
            parallel: aim.modes.parallel && parallel,
            extension: aim.modes.extension || aim.modes.extended_intersection,
            ..aim.modes
        };
        let mut resting = asking
            .map(|c| {
                let near = document.visible_here_in_rect(probe(c, aim.radius));
                targets(near, c, aim.radius, &wanted)
            })
            .unwrap_or_default();
        // Paths from what is kept, and the polar paths from the last point.
        // With running snaps off, kept points are suspended, not shown stale.
        let tracking = self.snap_cycle.tracking.clone();
        let keeping = tracking.otrack && aim.modes.enabled;
        aim.tracks = self.snap_acquire.paths(document, parallel).to_vec();
        let rect = self.snap_cycle.rect.unwrap_or(ui.clip_rect());
        let corner = |p: egui::Pos2| self.camera.screen_to_world(p, rect);
        let view = od_core::Bounds::from_points(corner(rect.min), corner(rect.max));
        let base = aim.from.filter(|_| tracking.polar && aim.base.is_none());
        aim.track = Some(TrackInput {
            origins: if keeping {
                self.snap_acquire.origins().to_vec()
            } else {
                Vec::new()
            }
            .into_iter()
            .chain(self.snap_cycle.temps.iter().map(|at| Origin {
                at: *at,
                kind: SnapKind::Tracking,
            }))
            .collect(),
            base,
            track_dirs: directions(&tracking.track_angles()),
            polar_dirs: directions(&tracking.polar_angles()),
            view,
        })
        .filter(|t| !t.origins.is_empty() || t.base.is_some());
        let found = asking.map(|c| aim.found(c));
        let listed = found.as_deref().unwrap_or_default();
        let step = if listed.len() > 1 {
            ui.input_mut(|i| {
                if i.consume_key(Modifiers::SHIFT, Key::Tab) {
                    -1
                } else {
                    i32::from(i.consume_key(Modifiers::NONE, Key::Tab))
                }
            })
        } else {
            0
        };
        self.snap_cycle.count = listed.len();
        self.snap_cycle.chosen = cycled(listed, self.snap_cycle.chosen, step);
        aim.chosen = self.snap_cycle.chosen;
        let resolved = cursor.zip(found).map(|(c, listed)| aim.settle(c, &listed));
        // A snap point the pointer rests on joins what is kept, unless Shift
        // says not to (or, in Shift-to-acquire mode, unless it is not held).
        let acquiring = shift == tracking.shift_to_acquire;
        if keeping && acquiring {
            let rested = resolved.and_then(|(_, s)| s).filter(|s| trackable(s.kind));
            let own = rested.map(|s| (Acquired::Origin(s.id()), s.point));
            let again = asking.and_then(|c| self.snap_acquire.kept_near(c, aim.radius));
            resting.splice(0..0, own.into_iter().chain(again.filter(|_| own.is_none())));
        } else {
            resting.retain(|(k, _)| !matches!(k, Acquired::Origin(_)));
        }
        if asking.is_none() && (self.point_prompt.is_none() || picking) {
            self.snap_acquire.clear();
        } else {
            let pulse = self.snap_acquire.update(ui.input(|i| i.time), &resting);
            if pulse.changed || pulse.counting {
                let wait = if pulse.changed { 0 } else { 60 };
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(wait));
            }
        }
        (aim, resolved)
    }
}
