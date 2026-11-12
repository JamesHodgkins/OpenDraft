//! Persisted page and plot settings shared by model and paper layouts.
//!
//! These are drawing semantics. Printer discovery, file paths, copies and
//! progress belong to the application and deliberately do not appear here.

use crate::Point;

/// Autodesk's documented plot-layout option bits (DXF group 70).
/// The output devices OpenDraft has of its own, as a page setup names them.
pub const DEVICE_PDF: &str = "OpenDraft PDF";
pub const DEVICE_SVG: &str = "OpenDraft SVG";

pub mod flags {
    pub const PLOT_VIEWPORT_BORDERS: u16 = 1;
    pub const SHOW_PLOT_STYLES: u16 = 2;
    pub const PLOT_CENTERED: u16 = 4;
    pub const PLOT_HIDDEN: u16 = 8;
    pub const USE_STANDARD_SCALE: u16 = 16;
    pub const PLOT_PLOT_STYLES: u16 = 32;
    pub const SCALE_LINEWEIGHTS: u16 = 64;
    pub const PRINT_LINEWEIGHTS: u16 = 128;
    pub const DRAW_VIEWPORTS_FIRST: u16 = 512;
    pub const MODEL_TYPE: u16 = 1024;
    pub const UPDATE_PAPER: u16 = 2048;
    pub const ZOOM_TO_PAPER_ON_UPDATE: u16 = 4096;
    pub const INITIALIZING: u16 = 8192;
    pub const PREVIOUS_PLOT_INITIALIZED: u16 = 16384;

    pub const KNOWN: u16 = PLOT_VIEWPORT_BORDERS
        | SHOW_PLOT_STYLES
        | PLOT_CENTERED
        | PLOT_HIDDEN
        | USE_STANDARD_SCALE
        | PLOT_PLOT_STYLES
        | SCALE_LINEWEIGHTS
        | PRINT_LINEWEIGHTS
        | DRAW_VIEWPORTS_FIRST
        | MODEL_TYPE
        | UPDATE_PAPER
        | ZOOM_TO_PAPER_ON_UPDATE
        | INITIALIZING
        | PREVIOUS_PLOT_INITIALIZED;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaperUnits {
    Inches,
    Millimeters,
    Pixels,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotRotation {
    Degrees0,
    CounterClockwise90,
    Degrees180,
    Clockwise90,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlotArea {
    Display,
    Extents,
    Limits,
    View(String),
    Window { first: Point, second: Point },
    Layout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StandardScale {
    Fit,
    Ratio(u8),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlotScale {
    Standard {
        kind: StandardScale,
        value: f64,
    },
    Custom {
        paper_units: f64,
        drawing_units: f64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShadePlotMode {
    AsDisplayed,
    Wireframe,
    Hidden,
    Rendered,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShadeResolution {
    Draft,
    Preview,
    Normal,
    Presentation,
    Maximum,
    Custom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShadePlotSettings {
    pub mode: ShadePlotMode,
    pub resolution: ShadeResolution,
    pub custom_dpi: u16,
}

impl Default for ShadePlotSettings {
    fn default() -> Self {
        Self {
            mode: ShadePlotMode::AsDisplayed,
            resolution: ShadeResolution::Normal,
            custom_dpi: 300,
        }
    }
}

/// Complete settings embedded in a layout.
#[derive(Clone, Debug, PartialEq)]
pub struct PlotSettings {
    pub device_name: String,
    pub canonical_media_name: String,
    pub paper_units: PaperUnits,
    pub paper_width_mm: f64,
    pub paper_height_mm: f64,
    /// Left, bottom, right and top, in millimetres.
    pub margins_mm: [f64; 4],
    pub origin_mm: Point,
    pub area: PlotArea,
    pub scale: PlotScale,
    pub rotation: PlotRotation,
    /// The documented DXF group-70 bits. Unknown bits are rejected by native validation.
    pub flags: u16,
    pub style_sheet: String,
    pub shade: ShadePlotSettings,
    pub paper_image_origin_mm: Point,
}

impl PlotSettings {
    /// The settings OpenDraft used before they were stored explicitly.
    pub fn for_layout(width: f64, height: f64) -> Self {
        Self {
            device_name: "none_device".into(),
            canonical_media_name: String::new(),
            paper_units: PaperUnits::Millimeters,
            paper_width_mm: width,
            paper_height_mm: height,
            margins_mm: [0.0; 4],
            origin_mm: Point::new(0.0, 0.0),
            area: PlotArea::Layout,
            scale: PlotScale::Standard {
                kind: StandardScale::Ratio(16),
                value: 1.0,
            },
            rotation: PlotRotation::Degrees0,
            flags: flags::USE_STANDARD_SCALE
                | flags::PLOT_PLOT_STYLES
                | flags::PRINT_LINEWEIGHTS
                | flags::DRAW_VIEWPORTS_FIRST,
            style_sheet: String::new(),
            shade: ShadePlotSettings::default(),
            paper_image_origin_mm: Point::new(0.0, 0.0),
        }
    }

    /// The Model tab's starting setup: A4 landscape, the drawing's extents
    /// fitted to the paper and centred. Model plots have no viewports to
    /// order, so that bit stays off.
    pub fn for_model() -> Self {
        let mut settings = Self::for_layout(297.0, 210.0);
        settings.area = PlotArea::Extents;
        settings.scale = PlotScale::Standard {
            kind: StandardScale::Fit,
            value: 1.0,
        };
        settings.flags = (settings.flags & !flags::DRAW_VIEWPORTS_FIRST)
            | flags::PLOT_CENTERED
            | flags::MODEL_TYPE;
        settings
    }

    pub fn is_valid(&self) -> bool {
        let finite = [
            self.paper_width_mm,
            self.paper_height_mm,
            self.margins_mm[0],
            self.margins_mm[1],
            self.margins_mm[2],
            self.margins_mm[3],
            self.origin_mm.x,
            self.origin_mm.y,
            self.paper_image_origin_mm.x,
            self.paper_image_origin_mm.y,
        ]
        .iter()
        .all(|v| v.is_finite());
        let margins = self.margins_mm.iter().all(|m| *m >= 0.0)
            && self.margins_mm[0] + self.margins_mm[2] < self.paper_width_mm
            && self.margins_mm[1] + self.margins_mm[3] < self.paper_height_mm;
        let scale = match self.scale {
            PlotScale::Standard { value, .. } => value.is_finite() && value > 0.0,
            PlotScale::Custom {
                paper_units,
                drawing_units,
            } => {
                paper_units.is_finite()
                    && drawing_units.is_finite()
                    && paper_units > 0.0
                    && drawing_units > 0.0
            }
        };
        let area = match &self.area {
            PlotArea::Window { first, second } => {
                first.x.is_finite()
                    && first.y.is_finite()
                    && second.x.is_finite()
                    && second.y.is_finite()
                    && first.x != second.x
                    && first.y != second.y
            }
            PlotArea::View(name) => valid_name(name) && !name.trim().is_empty(),
            _ => true,
        };
        finite
            && self.paper_width_mm >= 10.0
            && self.paper_height_mm >= 10.0
            && margins
            && scale
            && area
            && self.flags & !flags::KNOWN == 0
            && valid_name(&self.device_name)
            && valid_name(&self.canonical_media_name)
            && valid_name(&self.style_sheet)
            && (100..=32767).contains(&self.shade.custom_dpi)
    }
}

fn valid_name(value: &str) -> bool {
    !value.contains('\0') && value.chars().count() <= 255
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_layout_defaults_are_valid_and_one_to_one() {
        let settings = PlotSettings::for_layout(297.0, 210.0);
        assert!(settings.is_valid());
        assert_eq!(settings.area, PlotArea::Layout);
        assert_eq!(settings.paper_units, PaperUnits::Millimeters);
        assert!(matches!(
            settings.scale,
            PlotScale::Standard { value: 1.0, .. }
        ));
    }

    #[test]
    fn invalid_numbers_margins_flags_and_windows_are_refused() {
        let base = PlotSettings::for_layout(297.0, 210.0);
        let mut bad = base.clone();
        bad.paper_width_mm = f64::NAN;
        assert!(!bad.is_valid());
        bad = base.clone();
        bad.margins_mm = [200.0, 0.0, 200.0, 0.0];
        assert!(!bad.is_valid());
        bad = base.clone();
        bad.flags |= 256;
        assert!(!bad.is_valid());
        bad = base;
        bad.area = PlotArea::Window {
            first: Point::new(1.0, 1.0),
            second: Point::new(1.0, 2.0),
        };
        assert!(!bad.is_valid());
    }
}
