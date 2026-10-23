//! Plotting a part of model space: the same pipeline as a layout's viewport
//! (linetypes, text, hatches, blocks, dimensions), but for a window of the
//! drawing chosen by the caller and scaled to a given sheet width. Used to
//! look at a result, not to print.

use std::collections::HashSet;

use crate::plot::{add_viewed, PlotError, PlotSheet, TOLERANCE_MM};
use crate::{Bounds, Document, EntityId, Point};

/// `region` of model space drawn `width` units wide (the height follows),
/// with only the objects in `only` when it is given. The window must have
/// some size.
pub fn plot_region(
    document: &Document,
    region: Bounds,
    width: f64,
    only: Option<&HashSet<EntityId>>,
) -> Result<PlotSheet, PlotError> {
    let span = region.max.x - region.min.x;
    let rise = region.max.y - region.min.y;
    if !(span > 0.0 && rise >= 0.0 && width > 0.0 && span.is_finite() && rise.is_finite()) {
        return Err(PlotError::NoLayout);
    }
    let scale = width / span;
    let mut sheet = PlotSheet {
        width,
        height: rise * scale,
        paths: Vec::new(),
        fills: Vec::new(),
        notes: Vec::new(),
    };
    let frame = Bounds::from_points(Point::new(0.0, 0.0), Point::new(sheet.width, sheet.height));
    let to_paper =
        |p: Point| Point::new((p.x - region.min.x) * scale, (p.y - region.min.y) * scale);
    let tolerance = TOLERANCE_MM / scale;
    let mut wide = false;
    let objects = document
        .visible_in(None)
        .filter(|o| only.is_none_or(|ids| ids.contains(&o.id)) && o.bounds().intersects(&region));
    for object in objects.flat_map(|o| document.drawn(o)) {
        wide |= add_viewed(
            &mut sheet,
            document,
            &object,
            tolerance,
            (&to_paper, &frame, &region),
        )?;
    }
    if wide {
        sheet
            .notes
            .push("Wide polyline segments were drawn as single lines.".to_owned());
    }
    Ok(sheet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Entity;

    fn line(a: (f64, f64), b: (f64, f64)) -> Entity {
        Entity::Line {
            start: Point::new(a.0, a.1),
            end: Point::new(b.0, b.1),
        }
    }

    fn region(x0: f64, y0: f64, x1: f64, y1: f64) -> Bounds {
        Bounds::from_points(Point::new(x0, y0), Point::new(x1, y1))
    }

    #[test]
    fn a_window_scales_to_the_sheet_width() {
        let mut doc = Document::default();
        doc.add(line((100.0, 100.0), (200.0, 150.0)));
        let sheet = plot_region(&doc, region(100.0, 100.0, 200.0, 150.0), 50.0, None).unwrap();
        assert_eq!((sheet.width, sheet.height), (50.0, 25.0));
        assert_eq!(sheet.paths.len(), 1);
        let p = &sheet.paths[0].points;
        assert_eq!((p[0], p[1]), (Point::new(0.0, 0.0), Point::new(50.0, 25.0)));
    }

    #[test]
    fn only_what_is_in_the_window_and_the_set_is_drawn_and_clipped() {
        let mut doc = Document::default();
        let near = doc.add(line((0.0, 0.0), (10.0, 0.0)));
        doc.add(line((500.0, 500.0), (510.0, 500.0)));
        let long = doc.add(line((-100.0, 5.0), (100.0, 5.0)));
        let window = region(0.0, 0.0, 20.0, 10.0);
        let all = plot_region(&doc, window, 20.0, None).unwrap();
        assert_eq!(all.paths.len(), 2, "the far one is outside the window");
        let clipped = all
            .paths
            .iter()
            .find(|p| p.points.len() == 2 && p.points[0].y == 5.0)
            .unwrap();
        assert!(
            clipped.points.iter().all(|p| (0.0..=20.0).contains(&p.x)),
            "cut to the frame"
        );
        let only: HashSet<_> = [near].into_iter().collect();
        assert_eq!(
            plot_region(&doc, window, 20.0, Some(&only))
                .unwrap()
                .paths
                .len(),
            1
        );
        assert!(!only.contains(&long));
    }

    #[test]
    fn a_window_without_size_is_refused() {
        let doc = Document::default();
        assert!(plot_region(&doc, region(0.0, 0.0, 0.0, 5.0), 10.0, None).is_err());
        assert!(plot_region(&doc, region(0.0, 0.0, 5.0, 5.0), 0.0, None).is_err());
        assert!(plot_region(&doc, region(0.0, 0.0, 5.0, 5.0), f64::NAN, None).is_err());
    }
}
