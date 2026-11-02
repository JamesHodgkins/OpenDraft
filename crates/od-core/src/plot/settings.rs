//! Apply persisted page setup to already-generated sheet-space geometry.

use crate::plot::{clip_polyline, Painted, PlotError, PlotPath, PlotSheet};
use crate::plot_plan::{prints_lineweights, PlotValidationError};
use crate::plot_settings::flags;
use crate::{
    plot_text::clip_polygon, Bounds, PlotArea, PlotRotation, PlotScale, PlotSettings, Point,
};

pub(super) fn apply(mut sheet: PlotSheet, settings: &PlotSettings) -> Result<PlotSheet, PlotError> {
    if identity_layout(settings, &sheet) {
        return Ok(sheet);
    }
    let paper = Bounds::from_points(
        Point::new(0.0, 0.0),
        Point::new(settings.paper_width_mm, settings.paper_height_mm),
    );
    let printable = upright_printable(settings);
    let source = source_bounds(&sheet, settings)?;
    let factor = scale_factor(settings, &source, &printable);
    let source_size = Point::new(source.max.x - source.min.x, source.max.y - source.min.y);
    let scaled = Point::new(source_size.x * factor, source_size.y * factor);
    // A layout plots from the corner of its printable area, centred or not.
    let centred = settings.flags & flags::PLOT_CENTERED != 0 && settings.area != PlotArea::Layout;
    let at = if centred {
        Point::new(
            printable.min.x + (printable.max.x - printable.min.x - scaled.x) / 2.0,
            printable.min.y + (printable.max.y - printable.min.y - scaled.y) / 2.0,
        )
    } else {
        Point::new(
            printable.min.x + settings.origin_mm.x,
            printable.min.y + settings.origin_mm.y,
        )
    };
    let map = |p: Point| {
        rotate(
            settings,
            Point::new(
                at.x + (p.x - source.min.x) * factor,
                at.y + (p.y - source.min.y) * factor,
            ),
        )
    };
    let exact = prints_lineweights(settings);
    let mut placed = PlotSheet {
        width: settings.paper_width_mm,
        height: settings.paper_height_mm,
        paths: Vec::new(),
        fills: Vec::new(),
        notes: std::mem::take(&mut sheet.notes),
    };
    for item in sheet.paint_order() {
        match item {
            Painted::Path(path) => {
                let points: Vec<_> = path.points.iter().copied().map(map).collect();
                let mut width = if exact { path.width } else { 0.0 };
                if settings.flags & flags::SCALE_LINEWEIGHTS != 0 {
                    width *= factor;
                }
                for (points, closed) in clip_polyline(&points, path.closed, &paper) {
                    placed.paths.push(PlotPath {
                        points,
                        closed,
                        width,
                        ..path.clone()
                    });
                }
            }
            Painted::Fill(fill) => {
                let contours: Vec<_> = fill
                    .contours
                    .iter()
                    .map(|contour| {
                        let mapped: Vec<_> = contour.iter().copied().map(map).collect();
                        clip_polygon(&mapped, &paper)
                    })
                    .filter(|contour| !contour.is_empty())
                    .collect();
                if !contours.is_empty() {
                    placed.push_fill(contours, fill.rgb);
                }
            }
        }
    }
    Ok(placed)
}

/// An upright point (the drawing's own orientation) to physical media.
fn rotate(settings: &PlotSettings, p: Point) -> Point {
    let (w, h) = (settings.paper_width_mm, settings.paper_height_mm);
    match settings.rotation {
        PlotRotation::Degrees0 => p,
        PlotRotation::CounterClockwise90 => Point::new(w - p.y, p.x),
        PlotRotation::Degrees180 => Point::new(w - p.x, h - p.y),
        PlotRotation::Clockwise90 => Point::new(p.y, h - p.x),
    }
}

/// The inverse of [`rotate`]: a point on the media in upright coordinates.
fn unrotate(settings: &PlotSettings, p: Point) -> Point {
    let (w, h) = (settings.paper_width_mm, settings.paper_height_mm);
    match settings.rotation {
        PlotRotation::Degrees0 => p,
        PlotRotation::CounterClockwise90 => Point::new(p.y, w - p.x),
        PlotRotation::Degrees180 => Point::new(w - p.x, h - p.y),
        PlotRotation::Clockwise90 => Point::new(h - p.y, p.x),
    }
}

fn identity_layout(settings: &PlotSettings, sheet: &PlotSheet) -> bool {
    matches!(settings.area, PlotArea::Layout)
        && matches!(settings.scale, PlotScale::Standard { value, .. } if value == 1.0)
        && settings.paper_width_mm == sheet.width
        && settings.paper_height_mm == sheet.height
        && settings.rotation == PlotRotation::Degrees0
        && settings.margins_mm == [0.0; 4]
        && settings.origin_mm == Point::new(0.0, 0.0)
        && settings.flags & (flags::PLOT_CENTERED | flags::SCALE_LINEWEIGHTS) == 0
        && settings.flags & flags::PRINT_LINEWEIGHTS != 0
}

fn source_bounds(sheet: &PlotSheet, settings: &PlotSettings) -> Result<Bounds, PlotError> {
    match settings.area {
        PlotArea::Layout | PlotArea::Limits => Ok(Bounds::from_points(
            Point::new(0.0, 0.0),
            Point::new(sheet.width, sheet.height),
        )),
        PlotArea::Window { first, second } => Ok(Bounds::from_points(first, second)),
        PlotArea::Extents => {
            geometry_bounds(sheet).ok_or(PlotError::Invalid(PlotValidationError::NothingToPlot))
        }
        PlotArea::Display | PlotArea::View(_) => Err(PlotError::UnsupportedArea),
    }
}

fn geometry_bounds(sheet: &PlotSheet) -> Option<Bounds> {
    let points = sheet
        .paths
        .iter()
        .flat_map(|path| path.points.iter())
        .chain(
            sheet
                .fills
                .iter()
                .flat_map(|fill| fill.contours.iter().flatten()),
        );
    points.copied().fold(None, |bounds, point| {
        Some(match bounds {
            None => Bounds::from_points(point, point),
            Some(bounds) => Bounds::from_points(
                Point::new(bounds.min.x.min(point.x), bounds.min.y.min(point.y)),
                Point::new(bounds.max.x.max(point.x), bounds.max.y.max(point.y)),
            ),
        })
    })
}

/// The printable area as the drawing sees it: the drawing is placed upright
/// in the printable area turned back by the rotation, then each point is
/// turned onto the physical media.
fn upright_printable(settings: &PlotSettings) -> Bounds {
    let printable = Bounds::from_points(
        Point::new(settings.margins_mm[0], settings.margins_mm[1]),
        Point::new(
            settings.paper_width_mm - settings.margins_mm[2],
            settings.paper_height_mm - settings.margins_mm[3],
        ),
    );
    Bounds::from_points(
        unrotate(settings, printable.min),
        unrotate(settings, printable.max),
    )
}

/// Paper millimetres per unit of `source`, before anything is made from it
/// (to know how fine a curve has to be flattened).
pub(super) fn plan_scale(settings: &PlotSettings, source: &Bounds) -> f64 {
    scale_factor(settings, source, &upright_printable(settings))
}

fn scale_factor(settings: &PlotSettings, source: &Bounds, printable: &Bounds) -> f64 {
    match settings.scale {
        PlotScale::Standard {
            kind: crate::StandardScale::Fit,
            ..
        } => {
            let source_width = (source.max.x - source.min.x).max(f64::EPSILON);
            let source_height = (source.max.y - source.min.y).max(f64::EPSILON);
            ((printable.max.x - printable.min.x) / source_width)
                .min((printable.max.y - printable.min.y) / source_height)
        }
        PlotScale::Standard { value, .. } => value,
        PlotScale::Custom {
            paper_units,
            drawing_units,
        } => {
            // The paper side of the ratio is in the setup's paper units.
            let to_mm = if settings.paper_units == crate::PaperUnits::Inches {
                25.4
            } else {
                1.0
            };
            paper_units * to_mm / drawing_units
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plot::{PlotPath, PlotSheet};

    fn sheet() -> PlotSheet {
        PlotSheet {
            width: 100.0,
            height: 50.0,
            paths: vec![PlotPath {
                points: vec![Point::new(10.0, 10.0), Point::new(30.0, 20.0)],
                closed: false,
                rgb: [0, 0, 0],
                width: 0.25,
            }],
            fills: Vec::new(),
            notes: Vec::new(),
        }
    }

    #[test]
    fn custom_scale_window_and_offset_map_exactly() {
        let mut settings = PlotSettings::for_layout(100.0, 50.0);
        settings.area = PlotArea::Window {
            first: Point::new(10.0, 10.0),
            second: Point::new(50.0, 30.0),
        };
        settings.scale = PlotScale::Custom {
            paper_units: 2.0,
            drawing_units: 1.0,
        };
        settings.margins_mm = [5.0, 7.0, 5.0, 3.0];
        settings.origin_mm = Point::new(3.0, 4.0);
        let plotted = apply(sheet(), &settings).unwrap();
        assert_eq!(plotted.paths[0].points[0], Point::new(8.0, 11.0));
        assert_eq!(plotted.paths[0].points[1], Point::new(48.0, 31.0));
        assert_eq!(plotted.paths[0].width, 0.25);
    }

    #[test]
    fn extents_fit_and_center_uses_the_printable_rectangle() {
        let mut settings = PlotSettings::for_layout(120.0, 80.0);
        settings.area = PlotArea::Extents;
        settings.scale = PlotScale::Standard {
            kind: crate::StandardScale::Fit,
            value: 1.0,
        };
        settings.flags |= flags::PLOT_CENTERED;
        settings.margins_mm = [10.0; 4];
        let plotted = apply(sheet(), &settings).unwrap();
        assert_eq!(plotted.paths[0].points[0], Point::new(10.0, 15.0));
        assert_eq!(plotted.paths[0].points[1], Point::new(110.0, 65.0));
    }

    #[test]
    fn every_rotation_turns_the_drawing_onto_the_media_and_back() {
        let mut settings = PlotSettings::for_layout(100.0, 50.0);
        let p = Point::new(30.0, 10.0);
        for rotation in [
            PlotRotation::Degrees0,
            PlotRotation::CounterClockwise90,
            PlotRotation::Degrees180,
            PlotRotation::Clockwise90,
        ] {
            settings.rotation = rotation;
            let back = unrotate(&settings, rotate(&settings, p));
            assert!((back.x - p.x).abs() < 1e-9 && (back.y - p.y).abs() < 1e-9);
        }
    }

    #[test]
    fn a_quarter_turn_fits_the_drawing_to_the_turned_printable_area() {
        let mut settings = PlotSettings::for_layout(100.0, 50.0);
        settings.area = PlotArea::Extents;
        settings.scale = PlotScale::Standard {
            kind: crate::StandardScale::Fit,
            value: 1.0,
        };
        settings.rotation = PlotRotation::CounterClockwise90;
        let plotted = apply(sheet(), &settings).unwrap();
        // 20 x 10 of drawing fits 50 x 100 upright at 2.5, so it spans 50 x 25 upright from the corner.
        let [a, b] = [plotted.paths[0].points[0], plotted.paths[0].points[1]];
        assert!((a.x - 100.0).abs() < 1e-9 && a.y.abs() < 1e-9, "{a:?}");
        assert!(
            (b.x - 75.0).abs() < 1e-9 && (b.y - 50.0).abs() < 1e-9,
            "{b:?}"
        );
        assert_eq!((plotted.width, plotted.height), (100.0, 50.0));
    }

    #[test]
    fn a_layout_plots_from_the_printable_corner_even_with_centre_set() {
        let mut settings = PlotSettings::for_layout(120.0, 80.0);
        settings.flags |= flags::PLOT_CENTERED;
        settings.margins_mm = [10.0; 4];
        let plotted = apply(sheet(), &settings).unwrap();
        assert_eq!(
            plotted.paths[0].points[0],
            Point::new(20.0, 20.0),
            "10 in from the sheet corner, 10 for the margin"
        );
    }

    #[test]
    fn a_custom_scale_in_inches_is_converted_to_millimetres() {
        let mut settings = PlotSettings::for_layout(100.0, 50.0);
        settings.paper_units = crate::PaperUnits::Inches;
        let source = Bounds::from_points(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        settings.scale = PlotScale::Custom {
            paper_units: 1.0,
            drawing_units: 10.0,
        };
        assert!((plan_scale(&settings, &source) - 2.54).abs() < 1e-12);
        settings.paper_units = crate::PaperUnits::Millimeters;
        assert!((plan_scale(&settings, &source) - 0.1).abs() < 1e-12);
    }

    #[test]
    fn extents_of_an_empty_sheet_is_nothing_to_plot() {
        let mut settings = PlotSettings::for_layout(100.0, 50.0);
        settings.area = PlotArea::Extents;
        let mut empty = sheet();
        empty.paths.clear();
        assert_eq!(
            apply(empty, &settings),
            Err(PlotError::Invalid(PlotValidationError::NothingToPlot))
        );
    }

    #[test]
    fn display_and_named_view_are_refused_without_view_state() {
        let mut settings = PlotSettings::for_layout(100.0, 50.0);
        settings.area = PlotArea::Display;
        assert_eq!(apply(sheet(), &settings), Err(PlotError::UnsupportedArea));
        settings.area = PlotArea::View("Front".into());
        assert_eq!(apply(sheet(), &settings), Err(PlotError::UnsupportedArea));
    }
}
