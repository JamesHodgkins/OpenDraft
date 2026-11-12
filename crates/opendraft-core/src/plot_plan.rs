//! Checking a plot before any geometry is made: the page setup must make
//! sense, and the sheet that comes out must stay within the plot's limits.

use crate::plot::PlotSheet;
use crate::plot_settings::flags;
use crate::{PlotArea, PlotScale, PlotSettings};

/// The largest side of a sheet, in millimetres.
pub const MAX_PAPER_MM: f64 = 10_000.0;
/// The most points one plot may hold across all its paths and fills.
pub const MAX_PLOT_VERTICES: usize = 10_000_000;
/// The most paths and fills one plot may hold.
pub const MAX_PLOT_PATHS: usize = 2_000_000;
/// The most copies of a paper plot.
pub const MAX_COPIES: u32 = 999;
/// The smallest and largest paper-to-drawing ratio.
const SCALE_RANGE: (f64, f64) = (1e-12, 1e12);

/// Why a page setup cannot be plotted as it stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotValidationError {
    InvalidPaper,
    InvalidMargins,
    InvalidScale,
    InvalidWindow,
    InvalidSettings,
    /// Nothing is in the area to plot.
    NothingToPlot,
}

impl std::fmt::Display for PlotValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidPaper => "the paper size is not usable; each side is 10 to 10000 mm",
            Self::InvalidMargins => "the printable margins leave no room on the paper",
            Self::InvalidScale => "the plot scale must be a positive ratio",
            Self::InvalidWindow => "the plot window has no width or no height",
            Self::InvalidSettings => "the page setup has values that cannot be plotted",
            Self::NothingToPlot => "there is nothing to plot in that area",
        })
    }
}

/// Checks `settings` the way a plot needs them, naming the first thing wrong.
pub fn validate_plot(settings: &PlotSettings) -> Result<(), PlotValidationError> {
    let sides = [settings.paper_width_mm, settings.paper_height_mm];
    if !sides
        .iter()
        .all(|s| s.is_finite() && (10.0..=MAX_PAPER_MM).contains(s))
    {
        return Err(PlotValidationError::InvalidPaper);
    }
    let [left, bottom, right, top] = settings.margins_mm;
    let margins = settings
        .margins_mm
        .iter()
        .all(|m| m.is_finite() && *m >= 0.0);
    if !(margins && left + right < sides[0] && bottom + top < sides[1]) {
        return Err(PlotValidationError::InvalidMargins);
    }
    let ratio = match settings.scale {
        PlotScale::Standard { value, .. } => value,
        PlotScale::Custom {
            paper_units,
            drawing_units,
        } => paper_units / drawing_units,
    };
    if !(ratio.is_finite() && (SCALE_RANGE.0..=SCALE_RANGE.1).contains(&ratio)) {
        return Err(PlotValidationError::InvalidScale);
    }
    if let PlotArea::Window { first, second } = &settings.area {
        let flat = first.x == second.x || first.y == second.y;
        let finite = [first.x, first.y, second.x, second.y]
            .iter()
            .all(|v| v.is_finite());
        if flat || !finite {
            return Err(PlotValidationError::InvalidWindow);
        }
    }
    if !settings.is_valid() {
        return Err(PlotValidationError::InvalidSettings);
    }
    Ok(())
}

/// Whether the sheet is within the plot's limits (checked before a writer
/// allocates for it). Counts with checked arithmetic.
pub fn within_limits(sheet: &PlotSheet) -> bool {
    if sheet.paths.len().saturating_add(sheet.fills.len()) > MAX_PLOT_PATHS {
        return false;
    }
    let mut vertices = 0usize;
    let points = sheet.paths.iter().map(|p| p.points.len()).chain(
        sheet
            .fills
            .iter()
            .flat_map(|f| f.contours.iter().map(Vec::len)),
    );
    for n in points {
        vertices = match vertices.checked_add(n) {
            Some(total) if total <= MAX_PLOT_VERTICES => total,
            _ => return false,
        };
    }
    true
}

/// Whether the plot draws every lineweight exactly (off: hairlines).
pub fn prints_lineweights(settings: &PlotSettings) -> bool {
    settings.flags & flags::PRINT_LINEWEIGHTS != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plot::PlotPath;
    use crate::Point;

    fn base() -> PlotSettings {
        PlotSettings::for_layout(297.0, 210.0)
    }

    #[test]
    fn a_default_setup_is_valid() {
        assert_eq!(validate_plot(&base()), Ok(()));
    }

    #[test]
    fn each_kind_of_bad_setup_is_named() {
        let mut s = base();
        s.paper_width_mm = 20_000.0;
        assert_eq!(validate_plot(&s), Err(PlotValidationError::InvalidPaper));
        s = base();
        s.margins_mm = [150.0, 0.0, 150.0, 0.0];
        assert_eq!(validate_plot(&s), Err(PlotValidationError::InvalidMargins));
        s = base();
        s.scale = PlotScale::Custom {
            paper_units: 1.0,
            drawing_units: 1e20,
        };
        assert_eq!(validate_plot(&s), Err(PlotValidationError::InvalidScale));
        s = base();
        s.area = PlotArea::Window {
            first: Point::new(0.0, 0.0),
            second: Point::new(5.0, 0.0),
        };
        assert_eq!(validate_plot(&s), Err(PlotValidationError::InvalidWindow));
        s = base();
        s.device_name = "a\0b".into();
        assert_eq!(validate_plot(&s), Err(PlotValidationError::InvalidSettings));
    }

    #[test]
    fn a_sheet_over_the_path_limit_is_refused() {
        let path = PlotPath {
            points: vec![Point::new(0.0, 0.0)],
            closed: false,
            rgb: [0, 0, 0],
            width: 0.0,
        };
        let mut sheet = PlotSheet {
            width: 10.0,
            height: 10.0,
            paths: vec![path; 3],
            fills: Vec::new(),
            notes: Vec::new(),
        };
        assert!(within_limits(&sheet));
        sheet.paths = vec![sheet.paths[0].clone(); MAX_PLOT_PATHS + 1];
        assert!(!within_limits(&sheet));
    }
}
