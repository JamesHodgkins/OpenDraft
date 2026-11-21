//! What a press on the drawing does while no point is asked for (no command
//! running, or "Select objects"), by the selection style.
//!
//! - Click clears (the default): a drag makes a box, Window to the right and
//!   Crossing to the left; a click reports what it hit, and a click on
//!   nothing with no command running clears the selection (the app does
//!   that).
//! - Window and lasso: a click on nothing starts a box that the next click closes
//!   (an implied window), and a drag draws a lasso: Window when it sets off
//!   to the right, Crossing to the left; Space cycles Window, Crossing and
//!   Fence while it is drawn. Esc drops either.

use egui::{Key, Modifiers, PointerButton, Rect, Response, Ui};
use opendraft_core::{Document, Point};

use super::types::{Click, RegionSelect};
use super::{View, BOX_MIN_PX};
use crate::hit::pick_here;
use crate::select::Region;

/// Pointer moves shorter than this (screen points) add no point to a lasso.
const LASSO_STEP_PX: f32 = 3.0;

/// How a click on nothing and a drag select (Settings, Display).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionStyle {
    /// A click on nothing clears the selection; a drag makes a box.
    #[default]
    ClickClears,
    /// A click on nothing starts a window, a drag draws a lasso.
    WindowLasso,
}

/// What a lasso selects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LassoKind {
    Window,
    Crossing,
    Fence,
}

impl LassoKind {
    fn next(self) -> Self {
        match self {
            LassoKind::Window => LassoKind::Crossing,
            LassoKind::Crossing => LassoKind::Fence,
            LassoKind::Fence => LassoKind::Window,
        }
    }
}

/// A window or lasso under way (the window and lasso style).
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Pending {
    /// An implied window from this corner to the pointer.
    Window(Point),
    /// A lasso: its path, in drawing units, and its kind; none until the
    /// drag has gone far enough sideways to say (or Space chose one).
    Lasso {
        path: Vec<Point>,
        kind: Option<LassoKind>,
    },
}

/// What a frame of selecting came to.
#[derive(Default)]
pub(super) struct Selected {
    pub click: Option<Click>,
    pub region: Option<RegionSelect>,
    /// This frame's click started an implied window.
    pub started_window: bool,
}

/// The frame's facts that selecting goes by.
pub(super) struct Press {
    /// The pointer over the drawing, in drawing units.
    pub cursor: Option<Point>,
    /// Pick radius in drawing units.
    pub tolerance: f64,
    pub shift: bool,
    /// The press belongs to something else: a grip, or a prompt that ended.
    pub taken: bool,
}

impl View {
    /// How a click on nothing and a drag select from now on. A change drops
    /// what is under way.
    pub fn set_selection_style(&mut self, style: SelectionStyle) {
        if style != self.selection_style {
            self.selection_style = style;
            self.pending = None;
            self.drag = None;
        }
    }

    /// Whether an implied window or a lasso is under way.
    pub fn selection_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// The keys of a selection under way, taken before the app's own: Esc
    /// drops it, and Space cycles a lasso between Window, Crossing and Fence.
    pub fn take_selection_keys(&mut self, ctx: &egui::Context) {
        let Some(pending) = &mut self.pending else {
            return;
        };
        let mut dropped = false;
        ctx.input_mut(|i| {
            dropped = i.consume_key(Modifiers::NONE, Key::Escape);
            if let Pending::Lasso { kind, .. } = pending {
                if i.consume_key(Modifiers::NONE, Key::Space) {
                    *kind = Some(kind.unwrap_or(LassoKind::Window).next());
                }
            }
        });
        if dropped {
            self.pending = None;
            // The rest of a held drag starts nothing new.
            self.press_taken = true;
        }
    }

    /// Selecting's part of a frame, by the selection style.
    pub(super) fn select_frame(
        &mut self,
        ui: &Ui,
        response: &Response,
        rect: Rect,
        document: &Document,
        press: Press,
    ) -> Selected {
        match self.selection_style {
            SelectionStyle::ClickClears => self.box_frame(ui, response, rect, document, press),
            SelectionStyle::WindowLasso => self.lasso_frame(ui, response, rect, document, press),
        }
    }

    /// Only the layer that owned the original press may start a drag; once
    /// started, it may continue beyond that layer or the canvas.
    fn drag_owned(&self, ui: &Ui, rect: Rect) -> bool {
        self.drag.is_some()
            || self.pending.is_some()
            || ui
                .input(|i| i.pointer.press_origin())
                .is_some_and(|p| rect.contains(p) && !self.blocked(p))
    }

    /// Click clears: a click reports what it hit, a drag makes a box.
    fn box_frame(
        &mut self,
        ui: &Ui,
        response: &Response,
        rect: Rect,
        document: &Document,
        press: Press,
    ) -> Selected {
        let mut out = Selected::default();
        if response.clicked_by(PointerButton::Primary) && !press.taken {
            out.click = press.cursor.map(|c| Click {
                entity: pick_here(document, c, press.tolerance),
                remove: press.shift,
            });
        }
        if response.dragged_by(PointerButton::Primary) && !press.taken && self.drag_owned(ui, rect)
        {
            let now = ui
                .input(|i| i.pointer.latest_pos())
                .unwrap_or(rect.center());
            let start = self
                .drag
                .map(|(start, _)| start)
                .or_else(|| ui.input(|i| i.pointer.press_origin()))
                .unwrap_or(now);
            self.drag = Some((start, now));
        }
        if response.drag_stopped_by(PointerButton::Primary) {
            if let Some((start, end)) = self.drag.take() {
                if (end - start).abs().max_elem() > BOX_MIN_PX {
                    let (a, b) = (
                        self.camera.screen_to_world(start, rect),
                        self.camera.screen_to_world(end, rect),
                    );
                    out.region = Some(RegionSelect {
                        region: box_region(a, b),
                        remove: press.shift,
                    });
                }
            }
        }
        out
    }

    /// Window and lasso: a click on nothing starts an implied window that the next
    /// click (or release) closes; a drag draws a lasso.
    fn lasso_frame(
        &mut self,
        ui: &Ui,
        response: &Response,
        rect: Rect,
        document: &Document,
        press: Press,
    ) -> Selected {
        let mut out = Selected::default();
        let pointer = ui
            .input(|i| i.pointer.latest_pos())
            .map(|p| self.camera.screen_to_world(p, rect));
        let clicked = response.clicked_by(PointerButton::Primary) && !press.taken;
        if let Some(Pending::Window(corner)) = self.pending {
            let released = clicked || response.drag_stopped_by(PointerButton::Primary);
            if let (true, Some(to)) = (released, pointer) {
                self.pending = None;
                out.region = Some(RegionSelect {
                    region: box_region(corner, to),
                    remove: press.shift,
                });
            }
            return out;
        }
        if response.dragged_by(PointerButton::Primary) && !press.taken && self.drag_owned(ui, rect)
        {
            if let Some(now) = pointer {
                self.follow_lasso(ui, rect, now);
            }
        }
        if response.drag_stopped_by(PointerButton::Primary) {
            if let Some(Pending::Lasso { path, kind }) = self.pending.take() {
                let region = match kind.unwrap_or(LassoKind::Window) {
                    LassoKind::Window => Region::Window(path),
                    LassoKind::Crossing => Region::Crossing(path),
                    LassoKind::Fence => Region::Fence(path),
                };
                if region_points(&region) >= region.min_points() {
                    out.region = Some(RegionSelect {
                        region,
                        remove: press.shift,
                    });
                }
            }
        }
        if let (true, Some(at)) = (clicked, press.cursor) {
            match pick_here(document, at, press.tolerance) {
                Some(id) => {
                    out.click = Some(Click {
                        entity: Some(id),
                        remove: press.shift,
                    });
                }
                None => {
                    self.pending = Some(Pending::Window(at));
                    out.started_window = true;
                }
            }
        }
        out
    }

    /// Start a lasso at the press, or add the pointer to the one being drawn
    /// and, once it has gone far enough sideways, say which kind it is.
    fn follow_lasso(&mut self, ui: &Ui, rect: Rect, now: Point) {
        let camera = self.camera;
        match &mut self.pending {
            Some(Pending::Lasso { path, kind }) => {
                let screen_now = camera.world_to_screen(now, rect);
                let far = path.last().is_none_or(|last| {
                    camera.world_to_screen(*last, rect).distance(screen_now) >= LASSO_STEP_PX
                });
                if far {
                    path.push(now);
                }
                if let (None, Some(first)) = (*kind, path.first()) {
                    let dx = screen_now.x - camera.world_to_screen(*first, rect).x;
                    if dx.abs() > BOX_MIN_PX {
                        *kind = Some(if dx > 0.0 {
                            LassoKind::Window
                        } else {
                            LassoKind::Crossing
                        });
                    }
                }
            }
            _ => {
                let start = ui
                    .input(|i| i.pointer.press_origin())
                    .map_or(now, |p| camera.screen_to_world(p, rect));
                self.pending = Some(Pending::Lasso {
                    path: vec![start, now],
                    kind: None,
                });
            }
        }
    }

    /// Drop an implied window this frame's click started: the click took a
    /// viewport's border instead.
    pub(super) fn drop_new_window(&mut self) {
        if matches!(self.pending, Some(Pending::Window(_))) {
            self.pending = None;
        }
    }
}

/// A box between two corners: Window when drawn to the right, Crossing to
/// the left.
fn box_region(a: Point, b: Point) -> Region {
    let corners = Region::rect_corners(a, b);
    if b.x >= a.x {
        Region::Window(corners)
    } else {
        Region::Crossing(corners)
    }
}

fn region_points(region: &Region) -> usize {
    match region {
        Region::Window(p) | Region::Crossing(p) | Region::Fence(p) => p.len(),
    }
}
