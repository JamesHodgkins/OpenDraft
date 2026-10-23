//! The drawing viewport: camera, grid and entity rendering. World space is
//! drawing units with Y pointing up; screen space is egui points.

mod camera;
mod grid;
mod grips;
mod hatch_cache;
mod hit;
#[cfg(test)]
mod indexed_tests;
#[cfg(test)]
mod picture_tests;
pub mod preview;
mod select;
mod snap;
mod snap_axis;
mod view;

pub use camera::{Camera, MAX_ZOOM, MIN_ZOOM};
pub use grid::{grid_step, line_indices};
pub use grips::{grip_at, gripped, keep_selected, press_grip, GripHit, GRIP_OBJECT_LIMIT};
pub use hatch_cache::HatchCache;
pub use hit::{distance_to_entity, pick};
pub use select::{select, select_in, Region};
pub use snap::{
    candidates, snap, snap_all, Acquired, Guide, Origin, Probe, SnapId, SnapKind, SnapModes,
    SnapOverride, SnapPoint, Track, TrackInput, Tracking,
};
pub use snap_axis::Ortho;
pub use view::{
    paint_text, CanvasOutput, CanvasStyle, Click, Crosshair, Ghost, GhostStyle, GridSettings,
    PointPrompt, PointerSizes, RegionSelect, Space, View,
};

/// Helpers shared by the unit tests.
#[cfg(test)]
mod testing {
    use od_core::{Document, Entity, EntityId, Object};

    /// Objects with ids 0, 1, 2... in the order given.
    pub fn objs(entities: &[Entity]) -> Vec<Object> {
        Document::from_entities(entities.iter().cloned())
            .objects()
            .to_vec()
    }

    pub fn ids(ids: &[u64]) -> Vec<EntityId> {
        ids.iter().map(|&i| EntityId(i)).collect()
    }
}
