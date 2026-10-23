//! What the canvas is given to paint with and what it reports back.

use std::sync::Arc;

use egui::{Color32, Rect};
use od_core::{Entity, EntityId, Point, Viewport, ViewportId};

use crate::grips::GripHit;
use crate::select::Region;

/// Colors the canvas paints with; supplied by the app from its theme.
pub struct CanvasStyle {
    pub background: Color32,
    pub grid_minor: Color32,
    pub grid_major: Color32,
    pub axis_x: Color32,
    pub axis_y: Color32,
    pub entity: Color32,
    pub hover: Color32,
    /// Bright line drawn on selected entities.
    pub selected: Color32,
    /// Translucent overlay under selected entities.
    pub selected_glow: Color32,
    /// Snap marker and its label.
    pub snap: Color32,
    /// Window selection box (solid border, translucent fill).
    pub window: Color32,
    /// Crossing selection box and polygons (dashed border, translucent fill).
    pub crossing: Color32,
}

/// How big the pointer's targets are, in screen points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerSizes {
    /// Radius within which a click or hover picks an object.
    pub pick: f64,
    /// Radius within which the cursor is pulled onto a snap point.
    pub snap: f64,
    /// Side of a grip square; a press within this distance takes the grip.
    pub grip: f64,
}

impl Default for PointerSizes {
    fn default() -> Self {
        Self {
            pick: 6.0,
            snap: 12.0,
            grip: 7.0,
        }
    }
}

/// The drawing cursor's look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crosshair {
    /// Length of the crosshair lines as a percent of the canvas (1 to 100;
    /// 100 reaches the edges).
    pub size: u32,
    /// The lines' colour, or off-white if none.
    pub color: Option<Color32>,
}

impl Default for Crosshair {
    fn default() -> Self {
        Self {
            size: 5,
            color: None,
        }
    }
}

pub struct GridSettings {
    pub visible: bool,
    /// Base spacing in drawing units.
    pub spacing: f64,
}

/// A primary-button click on the canvas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Click {
    /// The entity that was hit, or `None` for empty space.
    pub entity: Option<EntityId>,
    /// Shift was held: remove from the selection instead of adding.
    pub remove: bool,
}

/// A finished region selection (box drag, polygon or fence).
#[derive(Clone, Debug, PartialEq)]
pub struct RegionSelect {
    pub region: Region,
    /// Shift was held: remove from the selection instead of adding.
    pub remove: bool,
}

/// Draws what a command is about to make for a pointer position.
pub type Ghost = Arc<dyn Fn(Point) -> Vec<Entity> + Send + Sync>;

/// How a ghost is drawn: the usual preview, or the colours of a selection
/// region (blue for Window, green and dashed for Crossing and Fence).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GhostStyle {
    #[default]
    Normal,
    Window,
    Crossing,
}

/// A running command is waiting for a point: the canvas snaps the pointer,
/// shows a rubber band and ghost, and reports the point placed.
#[derive(Clone, Default)]
pub struct PointPrompt {
    /// Rubber band start.
    pub base: Option<Point>,
    pub ghost: Option<Ghost>,
    pub ghost_style: GhostStyle,
    /// The command wants an object clicked, not a point: no snapping, the
    /// object under the pointer is highlighted and reported with the click.
    pub pick: bool,
    /// Objects on locked layers can be picked too (see
    /// `od_commands::Options::pick_locked`).
    pub pick_locked: bool,
}

#[derive(Default)]
pub struct CanvasOutput {
    pub rect: Option<Rect>,
    /// Snapped or trailing position used by the prompt preview.
    pub preview_cursor: Option<Point>,
    /// World position under the pointer, when it is over the canvas.
    pub cursor: Option<Point>,
    pub click: Option<Click>,
    /// The entity double-clicked while no command was waiting for input.
    pub double_click: Option<EntityId>,
    pub select: Option<RegionSelect>,
    /// A point placed while a command was waiting for one.
    pub prompted: Option<Point>,
    /// The object under `prompted`, when the prompt asked for one.
    pub picked: Option<EntityId>,
    /// Shift was held for the click in `picked`.
    pub pick_shift: bool,
    /// Grips pressed to edit, the one pressed first (nothing is selected or
    /// deselected by it): usually one, several when picked with Shift first.
    pub grips: Vec<GripHit>,
    /// The right button was clicked while a command was waiting for a point.
    pub prompt_enter: bool,
    /// Shift or Ctrl and the right button while a point is asked for: where
    /// the snap override menu is wanted (screen position).
    pub snap_menu: Option<egui::Pos2>,
    /// The right button was released over the canvas with no command asking
    /// for a point: where the context menu is wanted (screen position).
    pub context_menu: Option<egui::Pos2>,
    /// A click was turned down because a one-shot override found no snap.
    pub refused: bool,
    /// The snap point behind `prompted`, if it was placed on one.
    pub placed_snap: Option<crate::SnapPoint>,
    /// The snap point in force at the pointer, if any.
    pub snapped: Option<crate::SnapPoint>,
    /// Pan or zoom changed the active viewport's view; the app stores it.
    pub viewport_edit: Option<Viewport>,
    /// Double-click on a viewport in paper space.
    pub enter_viewport: Option<ViewportId>,
    /// Double-click outside the active viewport.
    pub leave_viewport: bool,
}
