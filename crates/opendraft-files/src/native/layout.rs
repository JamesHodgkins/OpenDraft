//! Layouts in the native file: sheets and their viewports.

use std::collections::HashSet;

use opendraft_core::{
    LayerId, Layout, LayoutId, PaperUnits, PlotArea, PlotRotation, PlotScale, PlotSettings, Point,
    ShadePlotMode, ShadePlotSettings, ShadeResolution, StandardScale, Viewport, ViewportId,
};
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    plot: Option<PlotSettingsDto>,
    /// Bottom viewport first.
    viewports: Vec<ViewportDto>,
}

#[derive(Serialize, Deserialize)]
pub(super) struct PlotSettingsDto {
    device_name: String,
    canonical_media_name: String,
    paper_units: PaperUnitsDto,
    paper_size_mm: [f64; 2],
    margins_mm: [f64; 4],
    origin_mm: [f64; 2],
    area: PlotAreaDto,
    scale: PlotScaleDto,
    rotation: PlotRotationDto,
    flags: u16,
    style_sheet: String,
    shade: ShadePlotDto,
    paper_image_origin_mm: [f64; 2],
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PaperUnitsDto {
    Inches,
    Millimeters,
    Pixels,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PlotAreaDto {
    Display,
    Extents,
    Limits,
    View { name: String },
    Window { first: [f64; 2], second: [f64; 2] },
    Layout,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum PlotScaleDto {
    Standard {
        code: u8,
        value: f64,
    },
    Custom {
        paper_units: f64,
        drawing_units: f64,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PlotRotationDto {
    Degrees0,
    CounterClockwise90,
    Degrees180,
    Clockwise90,
}

#[derive(Serialize, Deserialize)]
struct ShadePlotDto {
    mode: ShadeModeDto,
    resolution: ShadeResolutionDto,
    custom_dpi: u16,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ShadeModeDto {
    AsDisplayed,
    Wireframe,
    Hidden,
    Rendered,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ShadeResolutionDto {
    Draft,
    Preview,
    Normal,
    Presentation,
    Maximum,
    Custom,
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
    locked: bool,
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
            plot: Some(PlotSettingsDto::from_settings(&layout.plot)),
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
        let plot = dto
            .plot
            .map(PlotSettingsDto::into_settings)
            .unwrap_or_else(|| PlotSettings::for_layout(dto.width, dto.height));
        let layout = Layout {
            id: LayoutId(dto.id),
            name: dto.name,
            width: dto.width,
            height: dto.height,
            plot,
            viewports,
        };
        if !layout.is_valid() {
            return invalid(format!("layout {} has no usable sheet size", dto.id));
        }
        layouts.push(layout);
    }
    Ok(layouts)
}

impl PlotSettingsDto {
    pub(super) fn from_settings(settings: &PlotSettings) -> Self {
        let area = match &settings.area {
            PlotArea::Display => PlotAreaDto::Display,
            PlotArea::Extents => PlotAreaDto::Extents,
            PlotArea::Limits => PlotAreaDto::Limits,
            PlotArea::View(name) => PlotAreaDto::View { name: name.clone() },
            PlotArea::Window { first, second } => PlotAreaDto::Window {
                first: [first.x, first.y],
                second: [second.x, second.y],
            },
            PlotArea::Layout => PlotAreaDto::Layout,
        };
        let scale = match settings.scale {
            PlotScale::Standard { kind, value } => PlotScaleDto::Standard {
                code: match kind {
                    StandardScale::Fit => 0,
                    StandardScale::Ratio(code) => code,
                },
                value,
            },
            PlotScale::Custom {
                paper_units,
                drawing_units,
            } => PlotScaleDto::Custom {
                paper_units,
                drawing_units,
            },
        };
        Self {
            device_name: settings.device_name.clone(),
            canonical_media_name: settings.canonical_media_name.clone(),
            paper_units: match settings.paper_units {
                PaperUnits::Inches => PaperUnitsDto::Inches,
                PaperUnits::Millimeters => PaperUnitsDto::Millimeters,
                PaperUnits::Pixels => PaperUnitsDto::Pixels,
            },
            paper_size_mm: [settings.paper_width_mm, settings.paper_height_mm],
            margins_mm: settings.margins_mm,
            origin_mm: [settings.origin_mm.x, settings.origin_mm.y],
            area,
            scale,
            rotation: match settings.rotation {
                PlotRotation::Degrees0 => PlotRotationDto::Degrees0,
                PlotRotation::CounterClockwise90 => PlotRotationDto::CounterClockwise90,
                PlotRotation::Degrees180 => PlotRotationDto::Degrees180,
                PlotRotation::Clockwise90 => PlotRotationDto::Clockwise90,
            },
            flags: settings.flags,
            style_sheet: settings.style_sheet.clone(),
            shade: ShadePlotDto {
                mode: match settings.shade.mode {
                    ShadePlotMode::AsDisplayed => ShadeModeDto::AsDisplayed,
                    ShadePlotMode::Wireframe => ShadeModeDto::Wireframe,
                    ShadePlotMode::Hidden => ShadeModeDto::Hidden,
                    ShadePlotMode::Rendered => ShadeModeDto::Rendered,
                },
                resolution: match settings.shade.resolution {
                    ShadeResolution::Draft => ShadeResolutionDto::Draft,
                    ShadeResolution::Preview => ShadeResolutionDto::Preview,
                    ShadeResolution::Normal => ShadeResolutionDto::Normal,
                    ShadeResolution::Presentation => ShadeResolutionDto::Presentation,
                    ShadeResolution::Maximum => ShadeResolutionDto::Maximum,
                    ShadeResolution::Custom => ShadeResolutionDto::Custom,
                },
                custom_dpi: settings.shade.custom_dpi,
            },
            paper_image_origin_mm: [
                settings.paper_image_origin_mm.x,
                settings.paper_image_origin_mm.y,
            ],
        }
    }

    pub(super) fn into_settings(self) -> PlotSettings {
        PlotSettings {
            device_name: self.device_name,
            canonical_media_name: self.canonical_media_name,
            paper_units: match self.paper_units {
                PaperUnitsDto::Inches => PaperUnits::Inches,
                PaperUnitsDto::Millimeters => PaperUnits::Millimeters,
                PaperUnitsDto::Pixels => PaperUnits::Pixels,
            },
            paper_width_mm: self.paper_size_mm[0],
            paper_height_mm: self.paper_size_mm[1],
            margins_mm: self.margins_mm,
            origin_mm: Point::new(self.origin_mm[0], self.origin_mm[1]),
            area: match self.area {
                PlotAreaDto::Display => PlotArea::Display,
                PlotAreaDto::Extents => PlotArea::Extents,
                PlotAreaDto::Limits => PlotArea::Limits,
                PlotAreaDto::View { name } => PlotArea::View(name),
                PlotAreaDto::Window { first, second } => PlotArea::Window {
                    first: Point::new(first[0], first[1]),
                    second: Point::new(second[0], second[1]),
                },
                PlotAreaDto::Layout => PlotArea::Layout,
            },
            scale: match self.scale {
                PlotScaleDto::Standard { code, value } => PlotScale::Standard {
                    kind: if code == 0 {
                        StandardScale::Fit
                    } else {
                        StandardScale::Ratio(code)
                    },
                    value,
                },
                PlotScaleDto::Custom {
                    paper_units,
                    drawing_units,
                } => PlotScale::Custom {
                    paper_units,
                    drawing_units,
                },
            },
            rotation: match self.rotation {
                PlotRotationDto::Degrees0 => PlotRotation::Degrees0,
                PlotRotationDto::CounterClockwise90 => PlotRotation::CounterClockwise90,
                PlotRotationDto::Degrees180 => PlotRotation::Degrees180,
                PlotRotationDto::Clockwise90 => PlotRotation::Clockwise90,
            },
            flags: self.flags,
            style_sheet: self.style_sheet,
            shade: ShadePlotSettings {
                mode: match self.shade.mode {
                    ShadeModeDto::AsDisplayed => ShadePlotMode::AsDisplayed,
                    ShadeModeDto::Wireframe => ShadePlotMode::Wireframe,
                    ShadeModeDto::Hidden => ShadePlotMode::Hidden,
                    ShadeModeDto::Rendered => ShadePlotMode::Rendered,
                },
                resolution: match self.shade.resolution {
                    ShadeResolutionDto::Draft => ShadeResolution::Draft,
                    ShadeResolutionDto::Preview => ShadeResolution::Preview,
                    ShadeResolutionDto::Normal => ShadeResolution::Normal,
                    ShadeResolutionDto::Presentation => ShadeResolution::Presentation,
                    ShadeResolutionDto::Maximum => ShadeResolution::Maximum,
                    ShadeResolutionDto::Custom => ShadeResolution::Custom,
                },
                custom_dpi: self.shade.custom_dpi,
            },
            paper_image_origin_mm: Point::new(
                self.paper_image_origin_mm[0],
                self.paper_image_origin_mm[1],
            ),
        }
    }
}
