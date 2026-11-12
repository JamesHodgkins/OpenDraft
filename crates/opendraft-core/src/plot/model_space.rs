//! Plotting the Model tab: the drawing's own objects, in drawing units, made
//! into a sheet by the Model tab's page setup (area, scale, margins, offset,
//! rotation). The geometry is generated over the area, with curves as fine as
//! the final scale needs, and then placed on the paper like a layout's.

use super::{add_viewed, settings, PlotError, PlotSheet, TOLERANCE_MM};
use crate::plot_plan::{validate_plot, within_limits, PlotValidationError};
use crate::{Bounds, Document, PlotArea, Point};

/// The Model tab as a sheet ready to write.
pub(super) fn plot_model(document: &Document) -> Result<PlotSheet, PlotError> {
    let plot = document.model_plot();
    validate_plot(plot).map_err(PlotError::Invalid)?;
    let source = match &plot.area {
        PlotArea::Extents => extents(document)?,
        PlotArea::Window { first, second } => Bounds::from_points(*first, *second),
        // A model has no sheet, no limits and (yet) no saved views.
        _ => return Err(PlotError::UnsupportedArea),
    };
    let factor = settings::plan_scale(plot, &source);
    if !(factor.is_finite() && factor > 0.0) {
        return Err(PlotError::Invalid(PlotValidationError::InvalidScale));
    }
    let tolerance = TOLERANCE_MM / factor;
    // Extents are cut a little outside, so a stroke on the edge is whole; a
    // window is cut exactly.
    let margin = match plot.area {
        PlotArea::Extents => tolerance * 4.0,
        _ => 0.0,
    };
    let frame = Bounds::from_points(
        Point::new(source.min.x - margin, source.min.y - margin),
        Point::new(source.max.x + margin, source.max.y + margin),
    );
    let mut sheet = PlotSheet {
        width: source.max.x - source.min.x,
        height: source.max.y - source.min.y,
        paths: Vec::new(),
        fills: Vec::new(),
        notes: Vec::new(),
    };
    let same = |p: Point| p;
    let objects = document
        .visible_in(None)
        .filter(|o| o.bounds().intersects(&frame));
    for object in objects.flat_map(|o| document.drawn(o)) {
        add_viewed(
            &mut sheet,
            document,
            &object,
            tolerance,
            (&same, &frame, &frame),
        )?;
    }
    let sheet = settings::apply(sheet, plot)?;
    if within_limits(&sheet) {
        Ok(sheet)
    } else {
        Err(PlotError::ResourceLimit)
    }
}

/// The box around everything visible in the model.
fn extents(document: &Document) -> Result<Bounds, PlotError> {
    let mut all: Option<Bounds> = None;
    for object in document.visible_in(None) {
        let b = object.bounds();
        all = Some(match all {
            None => b,
            Some(a) => Bounds::from_points(
                Point::new(a.min.x.min(b.min.x), a.min.y.min(b.min.y)),
                Point::new(a.max.x.max(b.max.x), a.max.y.max(b.max.y)),
            ),
        });
    }
    all.filter(|b| b.max.x > b.min.x || b.max.y > b.min.y)
        .ok_or(PlotError::Invalid(PlotValidationError::NothingToPlot))
}

#[cfg(test)]
mod tests {
    use crate::plot::{plot_sheet, PlotSpace};
    use crate::{
        Color, Document, Entity, Layer, Lineweight, PaperUnits, PlotArea, PlotScale, PlotSettings,
        Point, StandardScale,
    };

    use super::*;

    fn line(a: (f64, f64), b: (f64, f64)) -> Entity {
        Entity::Line {
            start: Point::new(a.0, a.1),
            end: Point::new(b.0, b.1),
        }
    }

    fn drawing() -> Document {
        let mut doc = Document::default();
        doc.add(line((1000.0, 2000.0), (5000.0, 4000.0)));
        doc
    }

    fn plot(doc: &mut Document, f: impl FnOnce(&mut PlotSettings)) {
        let mut settings = doc.model_plot().clone();
        f(&mut settings);
        doc.set_model_plot(settings);
    }

    #[test]
    fn extents_fitted_and_centred_fill_the_printable_paper() {
        let doc = drawing();
        let sheet = plot_sheet(&doc, PlotSpace::Model).unwrap();
        assert_eq!((sheet.width, sheet.height), (297.0, 210.0));
        let points = &sheet.paths[0].points;
        // 4000 x 2000 fits 297 x 210 at 297/4000 and sits in the middle.
        let k = 297.0 / 4000.0;
        let height = 2000.0 * k;
        assert!((points[0].x - 0.0).abs() < 1e-6, "{points:?}");
        assert!(
            (points[0].y - (210.0 - height) / 2.0).abs() < 1e-6,
            "{points:?}"
        );
        assert!((points[1].x - 297.0).abs() < 1e-6);
    }

    #[test]
    fn a_numeric_scale_places_the_drawing_at_that_size() {
        let mut doc = drawing();
        plot(&mut doc, |s| {
            s.scale = PlotScale::Custom {
                paper_units: 1.0,
                drawing_units: 20.0,
            };
            s.flags &= !crate::plot_settings::flags::PLOT_CENTERED;
        });
        let sheet = plot_sheet(&doc, PlotSpace::Model).unwrap();
        let points = &sheet.paths[0].points;
        assert!(
            (points[1].x - points[0].x - 200.0).abs() < 1e-6,
            "4000 units at 1:20 is 200 mm"
        );
        assert!((points[1].y - points[0].y - 100.0).abs() < 1e-6);
        assert!((points[0].x).abs() < 1e-6, "from the printable corner");
    }

    #[test]
    fn a_scale_in_inches_is_inches_on_the_paper() {
        let mut doc = drawing();
        plot(&mut doc, |s| {
            s.paper_units = PaperUnits::Inches;
            s.scale = PlotScale::Custom {
                paper_units: 1.0,
                drawing_units: 1000.0,
            };
        });
        let sheet = plot_sheet(&doc, PlotSpace::Model).unwrap();
        let points = &sheet.paths[0].points;
        assert!((points[1].x - points[0].x - 4000.0 / 1000.0 * 25.4).abs() < 1e-6);
    }

    #[test]
    fn a_window_plots_only_what_is_in_it_and_cuts_at_its_edge() {
        let mut doc = drawing();
        doc.add(line((9000.0, 9000.0), (9500.0, 9500.0)));
        plot(&mut doc, |s| {
            s.area = PlotArea::Window {
                first: Point::new(0.0, 0.0),
                second: Point::new(3000.0, 3000.0),
            };
            s.scale = PlotScale::Custom {
                paper_units: 1.0,
                drawing_units: 10.0,
            };
        });
        let sheet = plot_sheet(&doc, PlotSpace::Model).unwrap();
        assert_eq!(sheet.paths.len(), 1, "the far line is outside");
        let furthest = sheet.paths[0]
            .points
            .iter()
            .map(|p| p.x)
            .fold(0.0_f64, f64::max);
        assert!(
            furthest <= 300.0 + 1e-6,
            "cut at the window's edge: {furthest}"
        );
    }

    #[test]
    fn rotation_and_margins_apply_to_a_model_plot_too() {
        let mut doc = drawing();
        plot(&mut doc, |s| {
            s.rotation = crate::PlotRotation::CounterClockwise90;
            s.margins_mm = [10.0, 10.0, 10.0, 10.0];
        });
        let sheet = plot_sheet(&doc, PlotSpace::Model).unwrap();
        for p in &sheet.paths[0].points {
            assert!(p.x >= 10.0 - 1e-6 && p.x <= 287.0 + 1e-6, "{p:?}");
            assert!(p.y >= 10.0 - 1e-6 && p.y <= 200.0 + 1e-6, "{p:?}");
        }
    }

    #[test]
    fn nothing_to_plot_and_areas_a_model_cannot_have_are_refused() {
        let mut doc = Document::default();
        assert_eq!(
            plot_sheet(&doc, PlotSpace::Model),
            Err(PlotError::Invalid(PlotValidationError::NothingToPlot))
        );
        doc.add(line((0.0, 0.0), (10.0, 10.0)));
        for area in [PlotArea::Layout, PlotArea::Display, PlotArea::Limits] {
            plot(&mut doc, |s| s.area = area.clone());
            assert_eq!(
                plot_sheet(&doc, PlotSpace::Model),
                Err(PlotError::UnsupportedArea)
            );
        }
    }

    #[test]
    fn a_frozen_layer_and_lineweights_follow_the_drawing() {
        let mut doc = Document::default();
        let hidden = doc.allocate_layer_id();
        doc.push_layer(Layer {
            color: Color::Rgb(200, 0, 0),
            lineweight: Lineweight(50),
            ..Layer::new(hidden, "Hidden")
        });
        doc.add(line((0.0, 0.0), (100.0, 0.0)));
        doc.add_with(
            line((0.0, 10.0), (100.0, 10.0)),
            crate::Attributes::on_layer(hidden),
        );
        assert_eq!(plot_sheet(&doc, PlotSpace::Model).unwrap().paths.len(), 2);
        doc.replace_layer(Layer {
            frozen: true,
            color: Color::Rgb(200, 0, 0),
            lineweight: Lineweight(50),
            ..Layer::new(hidden, "Hidden")
        });
        let sheet = plot_sheet(&doc, PlotSpace::Model).unwrap();
        assert_eq!(sheet.paths.len(), 1);
        assert_eq!(sheet.paths[0].rgb, [0, 0, 0]);
    }

    #[test]
    fn scale_lineweights_and_fit_do_not_confuse_the_standard_ratio() {
        let mut doc = drawing();
        plot(&mut doc, |s| {
            s.scale = PlotScale::Standard {
                kind: StandardScale::Fit,
                value: 1.0,
            };
        });
        assert!(plot_sheet(&doc, PlotSpace::Model).is_ok());
    }
}
