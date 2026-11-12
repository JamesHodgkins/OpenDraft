//! The `AcDbPlotSettings` part embedded in a DXF `LAYOUT` object.

use opendraft_core::{
    plot_settings::flags, PaperUnits, PlotArea, PlotRotation, PlotScale, PlotSettings, Point,
    ShadePlotMode, ShadePlotSettings, ShadeResolution, StandardScale,
};

use super::super::{decode, Record};

#[derive(Default)]
pub(super) struct Parsed {
    device: Option<String>,
    media: Option<String>,
    view: Option<String>,
    margins: [Option<f64>; 4],
    origin: [Option<f64>; 2],
    window: [Option<f64>; 4],
    custom_scale: [Option<f64>; 2],
    raw_flags: Option<i64>,
    units: Option<i64>,
    rotation: Option<i64>,
    area: Option<i64>,
    style_sheet: Option<String>,
    standard_scale: Option<i64>,
    shade_mode: Option<i64>,
    shade_resolution: Option<i64>,
    shade_dpi: Option<i64>,
    standard_value: Option<f64>,
    image_origin: [Option<f64>; 2],
    shade_handle: bool,
}

pub(super) fn parse(record: &Record) -> Parsed {
    let mut parsed = Parsed::default();
    let mut subclass = "";
    for group in &record.groups {
        if group.code == 100 {
            subclass = &group.value;
            continue;
        }
        if subclass != "AcDbPlotSettings" {
            continue;
        }
        let number = || group.value.parse::<f64>().ok().filter(|v| v.is_finite());
        let integer = || group.value.parse::<i64>().ok();
        match group.code {
            2 => parsed.device = Some(decode(&group.value)),
            4 => parsed.media = Some(decode(&group.value)),
            6 => parsed.view = Some(decode(&group.value)),
            40..=43 => parsed.margins[(group.code - 40) as usize] = number(),
            46 => parsed.origin[0] = number(),
            47 => parsed.origin[1] = number(),
            48 => parsed.window[0] = number(),
            140 => parsed.window[1] = number(),
            49 => parsed.window[2] = number(),
            141 => parsed.window[3] = number(),
            142 => parsed.custom_scale[0] = number(),
            143 => parsed.custom_scale[1] = number(),
            70 => parsed.raw_flags = integer(),
            72 => parsed.units = integer(),
            73 => parsed.rotation = integer(),
            74 => parsed.area = integer(),
            7 => parsed.style_sheet = Some(decode(&group.value)),
            75 => parsed.standard_scale = integer(),
            76 => parsed.shade_mode = integer(),
            77 => parsed.shade_resolution = integer(),
            78 => parsed.shade_dpi = integer(),
            147 => parsed.standard_value = number(),
            148 => parsed.image_origin[0] = number(),
            149 => parsed.image_origin[1] = number(),
            333 => parsed.shade_handle = true,
            _ => {}
        }
    }
    parsed
}

impl Parsed {
    /// Resolve optional DXF groups over OpenDraft's current defaults. The
    /// boolean says that an unsupported or damaged value was replaced.
    pub(super) fn finish(self, width: f64, height: f64) -> (PlotSettings, bool) {
        let mut settings = PlotSettings::for_layout(width, height);
        let mut unsupported = self.shade_handle;
        if let Some(value) = self.device {
            settings.device_name = value;
        }
        if let Some(value) = self.media {
            settings.canonical_media_name = value;
        }
        for (slot, value) in settings.margins_mm.iter_mut().zip(self.margins) {
            if let Some(value) = value {
                *slot = value;
            }
        }
        settings.origin_mm =
            Point::new(self.origin[0].unwrap_or(0.0), self.origin[1].unwrap_or(0.0));
        settings.paper_units = match self.units.unwrap_or(1) {
            0 => PaperUnits::Inches,
            1 => PaperUnits::Millimeters,
            2 => PaperUnits::Pixels,
            _ => {
                unsupported = true;
                PaperUnits::Millimeters
            }
        };
        settings.rotation = match self.rotation.unwrap_or(0) {
            0 => PlotRotation::Degrees0,
            1 => PlotRotation::CounterClockwise90,
            2 => PlotRotation::Degrees180,
            3 => PlotRotation::Clockwise90,
            _ => {
                unsupported = true;
                PlotRotation::Degrees0
            }
        };
        let first = Point::new(self.window[0].unwrap_or(0.0), self.window[1].unwrap_or(0.0));
        let second = Point::new(self.window[2].unwrap_or(0.0), self.window[3].unwrap_or(0.0));
        settings.area = match self.area.unwrap_or(5) {
            0 => PlotArea::Display,
            1 => PlotArea::Extents,
            2 => PlotArea::Limits,
            3 => PlotArea::View(self.view.unwrap_or_default()),
            4 => PlotArea::Window { first, second },
            5 => PlotArea::Layout,
            _ => {
                unsupported = true;
                PlotArea::Layout
            }
        };
        let raw_flags = self.raw_flags.unwrap_or(i64::from(settings.flags));
        if !(0..=i64::from(u16::MAX)).contains(&raw_flags) {
            unsupported = true;
        } else {
            let raw_flags = raw_flags as u16;
            unsupported |= raw_flags & !flags::KNOWN != 0;
            settings.flags = raw_flags & flags::KNOWN;
        }
        let standard = self.standard_scale.unwrap_or(16);
        settings.scale = if settings.flags & flags::USE_STANDARD_SCALE != 0 {
            if !(0..=32).contains(&standard) {
                unsupported = true;
            }
            PlotScale::Standard {
                kind: if standard == 0 {
                    StandardScale::Fit
                } else {
                    StandardScale::Ratio(standard.clamp(1, 32) as u8)
                },
                value: self.standard_value.unwrap_or(1.0),
            }
        } else {
            PlotScale::Custom {
                paper_units: self.custom_scale[0].unwrap_or(1.0),
                drawing_units: self.custom_scale[1].unwrap_or(1.0),
            }
        };
        settings.style_sheet = self.style_sheet.unwrap_or_default();
        settings.shade = ShadePlotSettings {
            mode: match self.shade_mode.unwrap_or(0) {
                0 => ShadePlotMode::AsDisplayed,
                1 => ShadePlotMode::Wireframe,
                2 => ShadePlotMode::Hidden,
                3 => ShadePlotMode::Rendered,
                _ => {
                    unsupported = true;
                    ShadePlotMode::AsDisplayed
                }
            },
            resolution: match self.shade_resolution.unwrap_or(2) {
                0 => ShadeResolution::Draft,
                1 => ShadeResolution::Preview,
                2 => ShadeResolution::Normal,
                3 => ShadeResolution::Presentation,
                4 => ShadeResolution::Maximum,
                5 => ShadeResolution::Custom,
                _ => {
                    unsupported = true;
                    ShadeResolution::Normal
                }
            },
            custom_dpi: self.shade_dpi.unwrap_or(300).clamp(0, i64::from(u16::MAX)) as u16,
        };
        settings.paper_image_origin_mm = Point::new(
            self.image_origin[0].unwrap_or(0.0),
            self.image_origin[1].unwrap_or(0.0),
        );
        if !settings.is_valid() {
            unsupported = true;
            settings = PlotSettings::for_layout(width, height);
        }
        (settings, unsupported)
    }
}
