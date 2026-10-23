//! Layouts in the native file: sheets and their viewports.

use std::collections::HashSet;

use od_core::{LayerId, Layout, LayoutId, Point, Viewport, ViewportId};
use serde::{Deserialize, Serialize};

use super::entity::invalid;
use crate::FileError;

#[derive(Serialize, Deserialize)]
pub struct LayoutDto {
    id: u64,
    name: String,
    /// Sheet size in millimetres.
    width: f64,
    height: f64,
    /// Bottom viewport first.
    viewports: Vec<ViewportDto>,
}

#[derive(Serialize, Deserialize)]
struct ViewportDto {
    id: u64,
    center: [f64; 2],
    width: f64,
    height: f64,
    view_center: [f64; 2],
    /// Paper millimetres per model unit.
    scale: f64,
    #[serde(default)]
    locked: bool,
    #[serde(default = "super::yes")]
    on: bool,
    /// Layers frozen in this viewport only.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    frozen: Vec<u64>,
}

impl LayoutDto {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn from_layout(layout: &Layout) -> Self {
        Self {
            id: layout.id.0,
            name: layout.name.clone(),
            width: layout.width,
            height: layout.height,
            viewports: layout
                .viewports
                .iter()
                .map(|v| ViewportDto {
                    id: v.id.0,
                    center: [v.center.x, v.center.y],
                    width: v.width,
                    height: v.height,
                    view_center: [v.view_center.x, v.view_center.y],
                    scale: v.scale,
                    locked: v.locked,
                    on: v.on,
                    frozen: v.frozen.iter().map(|l| l.0).collect(),
                })
                .collect(),
        }
    }
}

/// Turn the file's layouts into real ones, refusing what could not exist.
pub fn into_layouts(dtos: Vec<LayoutDto>) -> Result<Vec<Layout>, FileError> {
    let mut layout_ids = HashSet::new();
    let mut names = HashSet::new();
    let mut viewport_ids = HashSet::new();
    let mut layouts = Vec::new();
    for dto in dtos {
        if !layout_ids.insert(dto.id) {
            return invalid(format!("layout {} appears twice", dto.id));
        }
        if dto.name.trim().is_empty() || !names.insert(dto.name.to_lowercase()) {
            return invalid(format!("layout {} needs a unique name", dto.id));
        }
        let mut viewports = Vec::new();
        for v in dto.viewports {
            if !viewport_ids.insert(v.id) {
                return invalid(format!("viewport {} appears twice", v.id));
            }
            let viewport = Viewport {
                id: ViewportId(v.id),
                center: Point::new(v.center[0], v.center[1]),
                width: v.width,
                height: v.height,
                view_center: Point::new(v.view_center[0], v.view_center[1]),
                scale: v.scale,
                locked: v.locked,
                on: v.on,
                frozen: {
                    let mut layers: Vec<_> = v.frozen.iter().map(|l| LayerId(*l)).collect();
                    layers.sort_unstable();
                    layers.dedup();
                    layers
                },
            };
            if !viewport.is_valid() {
                return invalid(format!("viewport {} has no usable size or scale", v.id));
            }
            viewports.push(viewport);
        }
        let layout = Layout {
            id: LayoutId(dto.id),
            name: dto.name,
            width: dto.width,
            height: dto.height,
            viewports,
        };
        if !layout.is_valid() {
            return invalid(format!("layout {} has no usable sheet size", dto.id));
        }
        layouts.push(layout);
    }
    Ok(layouts)
}
