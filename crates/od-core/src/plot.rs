//! Plotting a layout: everything on the sheet as plain paths in millimetres.
//!
//! The sheet's own objects are taken as they are. Each viewport shows model
//! space through its scale and offset, clipped to its window, minus the layers
//! frozen in it. The result knows nothing about file formats; the writers in
//! `od-files` turn it into SVG or PDF.

use crate::plot_hatch;
use crate::plot_plan::{validate_plot, within_limits, PlotValidationError};
use crate::plot_settings::flags;
use crate::plot_text::{clip_polygon, object_fills};
use crate::{Bounds, Color, Document, Entity, LayoutId, Point};

mod model_space;
mod outlines;
mod settings;

use outlines::dashed_outlines;

/// How far a flattened curve may stray from the true one, in paper millimetres.
pub(crate) const TOLERANCE_MM: f64 = 0.02;

/// One stroked line of the plot.
#[derive(Clone, Debug, PartialEq)]
pub struct PlotPath {
    pub points: Vec<Point>,
    /// Joined back to the first point (only for shapes that were not clipped).
    pub closed: bool,
    pub rgb: [u8; 3],
    /// Line width in millimetres.
    pub width: f64,
}

/// Outlines filled together with the non-zero winding rule (text).
#[derive(Clone, Debug, PartialEq)]
pub struct PlotFill {
    pub contours: Vec<Vec<Point>>,
    pub rgb: [u8; 3],
    /// How many paths are painted before this fill; the fills of a sheet are
    /// in nondecreasing order of it.
    pub after: usize,
}

/// One painted thing of a sheet, in paint order.
#[derive(Clone, Copy, Debug)]
pub enum Painted<'a> {
    Path(&'a PlotPath),
    Fill(&'a PlotFill),
}

/// A sheet ready to write.
#[derive(Clone, Debug, PartialEq)]
pub struct PlotSheet {
    pub width: f64,
    pub height: f64,
    pub paths: Vec<PlotPath>,
    /// Filled shapes, painted after the paths.
    pub fills: Vec<PlotFill>,
    /// What the plot could not show faithfully, worded for the user.
    pub notes: Vec<String>,
}

impl PlotSheet {
    /// Paths and fills as they are painted: a fill goes over the paths
    /// before its `after`, and under the ones after it.
    pub fn paint_order(&self) -> impl Iterator<Item = Painted<'_>> {
        let mut fills = self.fills.iter().peekable();
        let mut paths = self.paths.iter().enumerate().peekable();
        std::iter::from_fn(move || {
            let next_path = paths.peek().map(|(i, _)| *i);
            match (fills.peek(), next_path) {
                (Some(fill), Some(i)) if fill.after <= i => fills.next().map(Painted::Fill),
                (Some(_), None) => fills.next().map(Painted::Fill),
                (_, Some(_)) => paths.next().map(|(_, p)| Painted::Path(p)),
                (None, None) => None,
            }
        })
    }

    /// A fill painted over everything pushed so far.
    pub(crate) fn push_fill(&mut self, contours: Vec<Vec<Point>>, rgb: [u8; 3]) {
        let after = self.paths.len();
        self.fills.push(PlotFill {
            contours,
            rgb,
            after,
        });
    }
}

/// Why a layout could not be plotted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlotError {
    NoLayout,
    /// A curve would need more points than allowed to stay within the
    /// plot's tolerance; plotting it coarser would not be faithful.
    TooDetailed,
    /// A hatch pattern would take more lines than the limits allow.
    HatchTooDense,
    /// The requested area depends on an application view OpenDraft does not persist.
    UnsupportedArea,
    /// The page setup cannot be plotted as it stands.
    Invalid(PlotValidationError),
    /// The plot would hold more paths or points than the limits allow.
    ResourceLimit,
}

impl std::fmt::Display for PlotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NoLayout => "there is no such layout to plot",
            Self::TooDetailed => {
                "an ellipse is too large to plot within 0.02 mm; plot a smaller part of it"
            }
            Self::HatchTooDense => "a hatch pattern is too dense to plot; increase its scale",
            Self::UnsupportedArea => {
                "Display and named-view plot areas are not available; use Layout, Extents or Window"
            }
            Self::Invalid(e) => return e.fmt(f),
            Self::ResourceLimit => "the plot is too large; plot a smaller area or fewer objects",
        })
    }
}

/// What is plotted: the Model tab, or one of the layouts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlotSpace {
    Model,
    Layout(LayoutId),
}

/// The Model tab or a layout, as a sheet made with its own page setup.
pub fn plot_sheet(document: &Document, space: PlotSpace) -> Result<PlotSheet, PlotError> {
    match space {
        PlotSpace::Model => model_space::plot_model(document),
        PlotSpace::Layout(id) => plot_layout(document, id),
    }
}

/// The layout as paths.
pub fn plot_layout(document: &Document, layout: LayoutId) -> Result<PlotSheet, PlotError> {
    let layout = document.layout(layout).ok_or(PlotError::NoLayout)?;
    validate_plot(&layout.plot).map_err(PlotError::Invalid)?;
    let mut sheet = PlotSheet {
        width: layout.width,
        height: layout.height,
        paths: Vec::new(),
        fills: Vec::new(),
        notes: Vec::new(),
    };
    // The viewports go under the sheet's own objects unless the plot says
    // "paper space last" the other way round.
    if layout.plot.flags & flags::DRAW_VIEWPORTS_FIRST != 0 {
        add_viewports(&mut sheet, document, layout)?;
        add_paper(&mut sheet, document, layout)?;
    } else {
        add_paper(&mut sheet, document, layout)?;
        add_viewports(&mut sheet, document, layout)?;
    }
    let sheet = settings::apply(sheet, &layout.plot)?;
    if within_limits(&sheet) {
        Ok(sheet)
    } else {
        Err(PlotError::ResourceLimit)
    }
}

/// What each viewport shows of model space, and its border when the plot
/// settings ask for borders.
fn add_viewports(
    sheet: &mut PlotSheet,
    document: &Document,
    layout: &crate::Layout,
) -> Result<(), PlotError> {
    for vp in layout.viewports.iter().filter(|v| v.on && v.is_valid()) {
        let (frame, shown, tolerance) = (
            vp.paper_bounds(),
            vp.model_bounds(),
            TOLERANCE_MM / vp.scale,
        );
        let objects = document
            .visible_in(None)
            .filter(|o| !vp.is_frozen(o.attrs.layer) && o.bounds().intersects(&shown));
        let pieces = objects.flat_map(|o| document.drawn(o));
        for object in pieces.filter(|o| !vp.is_frozen(o.attrs.layer)) {
            let to_paper = |p: Point| vp.model_to_paper(p);
            add_viewed(
                sheet,
                document,
                &object,
                tolerance,
                (&to_paper, &frame, &shown),
            )?;
        }
        if layout.plot.flags & flags::PLOT_VIEWPORT_BORDERS != 0 {
            let (a, b) = (frame.min, frame.max);
            sheet.paths.push(PlotPath {
                points: vec![a, Point::new(b.x, a.y), b, Point::new(a.x, b.y)],
                closed: true,
                rgb: [0, 0, 0],
                width: 0.0,
            });
        }
    }
    Ok(())
}

/// The sheet's own objects, as they are.
fn add_paper(
    sheet: &mut PlotSheet,
    document: &Document,
    layout: &crate::Layout,
) -> Result<(), PlotError> {
    // The sheet's own objects are not cut at its edge here (the page setup cuts
    // them at the paper), but a dash pattern is laid inside it.
    let window = Bounds::from_points(Point::new(0.0, 0.0), Point::new(sheet.width, sheet.height));
    let frame = Bounds::from_points(Point::new(-1e12, -1e12), Point::new(1e12, 1e12));
    let same = |p: Point| p;
    for object in document
        .visible_in(Some(layout.id))
        .flat_map(|o| document.drawn(o))
    {
        add_viewed(
            sheet,
            document,
            &object,
            TOLERANCE_MM,
            (&same, &frame, &window),
        )?;
    }
    Ok(())
}

/// One object seen through a window: its outlines, fills and text moved to
/// the sheet by `to_paper` and clipped to `frame`. `tolerance` is in the
/// object's own units.
pub(crate) fn add_viewed(
    sheet: &mut PlotSheet,
    document: &Document,
    object: &crate::Object,
    tolerance: f64,
    view: (&dyn Fn(Point) -> Point, &Bounds, &Bounds),
) -> Result<(), PlotError> {
    // How the object's units map to the sheet, the sheet's frame, and the
    // window of the object's units that frame shows.
    let (to_paper, frame, window) = view;
    let (rgb, width) = look(document, object);
    let ribbons = wide_ribbons(object, tolerance);
    let outlines = if ribbons.as_ref().is_some_and(|r| r.covers_all) {
        Vec::new()
    } else {
        dashed_outlines(document, object, tolerance, window)?
    };
    for (points, closed) in outlines {
        let paper: Vec<Point> = points.iter().map(|p| to_paper(*p)).collect();
        for (points, closed) in clip_polyline(&paper, closed, frame) {
            sheet.paths.push(PlotPath {
                points,
                closed,
                rgb,
                width,
            });
        }
    }
    let clipped = |contours: &[Vec<Point>]| -> Vec<Vec<Point>> {
        contours
            .iter()
            .map(|c| {
                let paper: Vec<Point> = c.iter().map(|p| to_paper(*p)).collect();
                clip_polygon(&paper, frame)
            })
            .filter(|c| !c.is_empty())
            .collect()
    };
    if let Entity::Hatch(h) = &object.entity {
        if let Some(solid) = plot_hatch::fill(h, tolerance)? {
            let contours = clipped(&solid);
            if !contours.is_empty() {
                sheet.push_fill(contours, rgb);
            }
        }
    }
    if let Some(ribbons) = ribbons {
        let contours = clipped(&ribbons.contours);
        if !contours.is_empty() {
            sheet.push_fill(contours, rgb);
        }
    }
    for fill in object_fills(document, object, tolerance, rgb) {
        let contours = clipped(&fill.contours);
        if !contours.is_empty() {
            sheet.push_fill(contours, fill.rgb);
        }
    }
    Ok(())
}

/// The area of a wide polyline, as triangles all turned the same way (so
/// overlaps add up under the non-zero rule).
struct Ribbons {
    contours: Vec<Vec<Point>>,
    /// Whether every segment is wide, so no centre line is needed.
    covers_all: bool,
}

fn wide_ribbons(object: &crate::Object, tolerance: f64) -> Option<Ribbons> {
    let Entity::Polyline {
        points,
        bulges,
        widths,
        closed,
    } = &object.entity
    else {
        return None;
    };
    let triangles = crate::width::triangles(points, bulges, widths, *closed, tolerance);
    if triangles.is_empty() {
        return None;
    }
    let segments = crate::bulge::segment_count(points.len(), *closed);
    let covers_all = (0..segments).all(|i| {
        let (a, b) = crate::width::at(widths, i);
        a > 0.0 || b > 0.0
    });
    let contours = triangles
        .into_iter()
        .map(|[a, b, c]| {
            let turn = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
            if turn < 0.0 {
                vec![a, c, b]
            } else {
                vec![a, b, c]
            }
        })
        .collect();
    Some(Ribbons {
        contours,
        covers_all,
    })
}

/// Colour and width of an object as plotted: black on white for the
/// foreground colour and for white, the layer's lineweight in millimetres
/// (0 is a hairline: the thinnest line the device draws).
fn look(document: &Document, object: &crate::Object) -> ([u8; 3], f64) {
    let rgb = match document.color_of(object) {
        Color::Foreground => [0, 0, 0],
        Color::Rgb(r, g, b) if r >= 240 && g >= 240 && b >= 240 => [0, 0, 0],
        Color::Rgb(r, g, b) => [r, g, b],
    };
    let width = f64::from(document.lineweight_of(object).0) / 100.0;
    (rgb, width)
}

/// The parts of a polyline inside `frame`. A shape wholly inside comes back as
/// it was (still closed); otherwise it is cut into open pieces.
pub fn clip_polyline(points: &[Point], closed: bool, frame: &Bounds) -> Vec<(Vec<Point>, bool)> {
    let inside = |p: &Point| {
        p.x >= frame.min.x && p.x <= frame.max.x && p.y >= frame.min.y && p.y <= frame.max.y
    };
    if points.len() < 2 {
        return Vec::new();
    }
    if points.iter().all(inside) {
        return vec![(points.to_vec(), closed)];
    }
    let mut ring = points.to_vec();
    if closed {
        ring.push(points[0]);
    }
    let mut pieces: Vec<Vec<Point>> = Vec::new();
    let mut current: Vec<Point> = Vec::new();
    for pair in ring.windows(2) {
        match clip_segment(pair[0], pair[1], frame) {
            Some((a, b)) => {
                if current.last() != Some(&a) {
                    if !current.is_empty() {
                        pieces.push(std::mem::take(&mut current));
                    }
                    current.push(a);
                }
                current.push(b);
            }
            None => {
                if !current.is_empty() {
                    pieces.push(std::mem::take(&mut current));
                }
            }
        }
    }
    if !current.is_empty() {
        pieces.push(current);
    }
    pieces.into_iter().map(|p| (p, false)).collect()
}

/// The part of the segment inside `frame` (Liang-Barsky), or `None`.
fn clip_segment(a: Point, b: Point, frame: &Bounds) -> Option<(Point, Point)> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for (p, q) in [
        (-dx, a.x - frame.min.x),
        (dx, frame.max.x - a.x),
        (-dy, a.y - frame.min.y),
        (dy, frame.max.y - a.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    (t0 <= t1).then(|| {
        let at = |t: f64| {
            if t == 0.0 {
                a
            } else if t == 1.0 {
                b
            } else {
                Point::new(a.x + t * dx, a.y + t * dy)
            }
        };
        (at(t0), at(t1))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Attr, Attributes, Layer, Lineweight, Viewport, ViewportId};

    fn frame() -> Bounds {
        Bounds::from_points(Point::new(0.0, 0.0), Point::new(10.0, 10.0))
    }

    fn line(a: (f64, f64), b: (f64, f64)) -> Entity {
        Entity::Line {
            start: Point::new(a.0, a.1),
            end: Point::new(b.0, b.1),
        }
    }

    /// A drawing with `entities` in model space and a 297x210 layout whose one
    /// viewport (1:100, 100 x 80 mm at (50,50)) shows model (0..) around (5000, 4000).
    fn plotted(entities: &[Entity]) -> (Document, LayoutId) {
        let mut doc = Document::default();
        for e in entities {
            doc.add(e.clone());
        }
        let vp = Viewport {
            id: ViewportId(1),
            center: Point::new(100.0, 100.0),
            width: 100.0,
            height: 80.0,
            view_center: Point::new(5000.0, 4000.0),
            scale: 0.01,
            locked: false,
            on: true,
            frozen: Vec::new(),
        };
        let id = doc.allocate_layout_id();
        let layout = crate::Layout {
            id,
            name: "A".into(),
            width: 297.0,
            height: 210.0,
            plot: crate::PlotSettings::for_layout(297.0, 210.0),
            viewports: vec![vp],
        };
        doc.set_layouts(vec![layout]);
        (doc, id)
    }

    #[test]
    fn segments_are_cut_at_the_frame() {
        let cut = clip_segment(Point::new(-5.0, 5.0), Point::new(15.0, 5.0), &frame()).unwrap();
        assert_eq!(cut, (Point::new(0.0, 5.0), Point::new(10.0, 5.0)));
        assert_eq!(
            clip_segment(Point::new(-5.0, 20.0), Point::new(15.0, 20.0), &frame()),
            None
        );
        let inside = (Point::new(1.0, 1.0), Point::new(2.0, 2.0));
        assert_eq!(clip_segment(inside.0, inside.1, &frame()), Some(inside));
        // Parallel to an edge and outside it.
        assert_eq!(
            clip_segment(Point::new(-1.0, 0.0), Point::new(-1.0, 5.0), &frame()),
            None
        );
    }

    #[test]
    fn a_shape_inside_stays_whole_and_one_crossing_the_frame_is_cut_into_pieces() {
        let square = [
            Point::new(2.0, 2.0),
            Point::new(4.0, 2.0),
            Point::new(4.0, 4.0),
            Point::new(2.0, 4.0),
        ];
        assert_eq!(
            clip_polyline(&square, true, &frame()),
            vec![(square.to_vec(), true)]
        );
        // A "U" whose arms leave the top of the frame: two pieces joined at the bottom.
        let u = [
            Point::new(2.0, 12.0),
            Point::new(2.0, 2.0),
            Point::new(8.0, 2.0),
            Point::new(8.0, 12.0),
        ];
        let pieces = clip_polyline(&u, false, &frame());
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].0.first(), Some(&Point::new(2.0, 10.0)));
        assert_eq!(pieces[0].0.last(), Some(&Point::new(8.0, 10.0)));
        // A line fully outside leaves nothing.
        assert!(clip_polyline(
            &[Point::new(20.0, 20.0), Point::new(30.0, 20.0)],
            false,
            &frame()
        )
        .is_empty());
    }

    #[test]
    fn a_viewport_plots_model_space_at_its_scale_and_clips_to_its_window() {
        // 1000 model units long = 10 mm at 1:100, centred on the view centre.
        let (doc, id) = plotted(&[
            line((4500.0, 4000.0), (5500.0, 4000.0)),
            line((20000.0, 20000.0), (20100.0, 20100.0)),
        ]);
        let sheet = plot_layout(&doc, id).unwrap();
        assert_eq!((sheet.width, sheet.height), (297.0, 210.0));
        assert_eq!(sheet.paths.len(), 1, "the far line is outside the viewport");
        let path = &sheet.paths[0];
        assert!((path.points[0].x - 95.0).abs() < 1e-9 && (path.points[1].x - 105.0).abs() < 1e-9);
        assert!((path.points[0].y - 100.0).abs() < 1e-9);
        // A line wider than the window is cut at its edge (x = 50..150).
        let (doc, id) = plotted(&[line((0.0, 4000.0), (10000.0, 4000.0))]);
        let cut = &plot_layout(&doc, id).unwrap().paths[0];
        assert!((cut.points[0].x - 50.0).abs() < 1e-9 && (cut.points[1].x - 150.0).abs() < 1e-9);
    }

    #[test]
    fn construction_lines_and_rays_are_cut_to_the_viewport() {
        let (doc, id) = plotted(&[
            Entity::Xline {
                origin: Point::new(0.0, 4000.0),
                direction: Point::new(1.0, 0.0),
            },
            Entity::Ray {
                origin: Point::new(5000.0, 4000.0),
                direction: Point::new(0.0, 1.0),
            },
        ]);
        let sheet = plot_layout(&doc, id).unwrap();
        assert_eq!(sheet.paths.len(), 2);
        // The window is 50..150 across and 60..140 up on the sheet.
        let across = &sheet.paths[0].points;
        assert!((across[0].x - 50.0).abs() < 1e-6 && (across[1].x - 150.0).abs() < 1e-6);
        let up = &sheet.paths[1].points;
        assert!((up[0].y - 100.0).abs() < 1e-6 && (up[1].y - 140.0).abs() < 1e-6);
    }

    #[test]
    fn a_point_plots_in_the_drawings_point_style() {
        let at = Point::new(5000.0, 4000.0);
        let (mut doc, id) = plotted(&[Entity::Point { at }]);
        // The default is a dot: a hair of line.
        assert_eq!(plot_layout(&doc, id).unwrap().paths.len(), 1);
        // A plus 400 model units across is 4 mm on the sheet at 1:100.
        doc.set_point_style(crate::PointStyle {
            mode: 2,
            size: 400.0,
        });
        let sheet = plot_layout(&doc, id).unwrap();
        assert_eq!(sheet.paths.len(), 2);
        let across = sheet.paths[0].points[1].x - sheet.paths[0].points[0].x;
        assert!((across - 4.0).abs() < 1e-9, "{across}");
        // None draws nothing.
        doc.set_point_style(crate::PointStyle { mode: 1, size: 0.0 });
        assert!(plot_layout(&doc, id).unwrap().paths.is_empty());
        // A share of the screen is a share of what the viewport shows: 5% of
        // 80 mm is 4 mm.
        doc.set_point_style(crate::PointStyle { mode: 2, size: 0.0 });
        let sheet = plot_layout(&doc, id).unwrap();
        let across = sheet.paths[0].points[1].x - sheet.paths[0].points[0].x;
        assert!((across - 4.0).abs() < 1e-9, "{across}");
    }

    #[test]
    fn frozen_layers_and_off_viewports_are_left_out_and_sheet_objects_are_not_clipped() {
        let (mut doc, id) = plotted(&[line((4500.0, 4000.0), (5500.0, 4000.0))]);
        let layer = doc.allocate_layer_id();
        doc.push_layer(Layer::new(layer, "Walls"));
        doc.add_with(
            line((4600.0, 4100.0), (5400.0, 4100.0)),
            Attributes::on_layer(layer),
        );
        assert_eq!(plot_layout(&doc, id).unwrap().paths.len(), 2);
        let mut layouts = doc.layouts().to_vec();
        layouts[0].viewports[0].set_frozen(&[layer], true);
        doc.set_layouts(layouts.clone());
        assert_eq!(plot_layout(&doc, id).unwrap().paths.len(), 1);
        layouts[0].viewports[0].on = false;
        doc.set_layouts(layouts);
        assert!(plot_layout(&doc, id).unwrap().paths.is_empty());

        doc.set_active_layout(Some(id));
        doc.add(line((-50.0, 5.0), (400.0, 5.0)));
        let sheet = plot_layout(&doc, id).unwrap();
        assert_eq!(sheet.paths.len(), 1);
        assert_eq!(
            sheet.paths[0].points,
            [Point::new(-50.0, 5.0), Point::new(400.0, 5.0)]
        );
        assert_eq!(
            plot_layout(&doc, crate::LayoutId(99)),
            Err(PlotError::NoLayout)
        );
    }

    #[test]
    fn text_plots_as_filled_glyphs_clipped_to_its_viewport() {
        use crate::TextStyleId;
        let text = |at: Point, contents: &str| {
            Entity::MText(Box::new(crate::MText::new(
                at,
                250.0,
                contents,
                TextStyleId::STANDARD,
            )))
        };
        // The viewport shows model x 4500..5500, y 3600..4400 at 1:100.
        let (mut doc, id) = plotted(&[
            text(Point::new(4600.0, 4200.0), "Hi"),
            text(Point::new(9000.0, 9000.0), "Away"),
            text(Point::new(5400.0, 4200.0), "Cut off at the edge"),
        ]);
        doc.set_active_layout(Some(id));
        doc.add(Entity::MText(Box::new(crate::MText::new(
            Point::new(10.0, 20.0),
            5.0,
            "Sheet",
            TextStyleId::STANDARD,
        ))));
        let sheet = plot_layout(&doc, id).unwrap();
        assert!(sheet.paths.is_empty(), "text is not stroked");
        let frame = doc.layout(id).unwrap().viewports[0].paper_bounds();
        let (inside, on_sheet): (Vec<_>, Vec<_>) = sheet.fills.iter().partition(|f| {
            f.contours
                .iter()
                .flatten()
                .all(|p| p.x >= frame.min.x - 1e-9 && p.x <= frame.max.x + 1e-9)
        });
        assert!(!inside.is_empty() && !on_sheet.is_empty());
        // "Hi" is 2.5 mm tall on paper at 1:100, near the viewport's left.
        let hi = inside[0].contours.iter().flatten();
        let top = hi.clone().map(|p| p.y).fold(f64::MIN, f64::max);
        let bottom = hi.map(|p| p.y).fold(f64::MAX, f64::min);
        assert!((top - bottom - 2.5).abs() < 0.3, "{}", top - bottom);
        // Nothing from the far text got in, and the long one stopped at the frame.
        let widest = inside
            .iter()
            .flat_map(|f| f.contours.iter().flatten())
            .map(|p| p.x)
            .fold(f64::MIN, f64::max);
        assert!(widest <= frame.max.x + 1e-9);
        // Sheet text is unclipped, in millimetres, at its own height.
        let sheet_text = on_sheet.iter().flat_map(|f| f.contours.iter().flatten());
        let min_x = sheet_text.map(|p| p.x).fold(f64::MAX, f64::min);
        assert!(min_x > 10.0 && min_x < 11.0, "{min_x}");
    }

    #[test]
    fn dimension_text_plots_as_filled_glyphs_not_strokes() {
        let (mut doc, id) = plotted(&[]);
        doc.set_active_layout(Some(id));
        let style = crate::DimStyle::default();
        doc.add(Entity::Dimension(Box::new(crate::Dimension::aligned(
            Point::new(10.0, 10.0),
            Point::new(60.0, 10.0),
            Point::new(10.0, 25.0),
            &style,
        ))));
        let sheet = plot_layout(&doc, id).unwrap();
        assert!(!sheet.fills.is_empty(), "the measurement is filled glyphs");
        // The lines and arrows are paths, and none of them is a letter shape.
        assert!(!sheet.paths.is_empty());
        let (top, bottom) = sheet
            .fills
            .iter()
            .flat_map(|f| f.contours.iter().flatten())
            .fold((f64::MIN, f64::MAX), |(t, b), p| (t.max(p.y), b.min(p.y)));
        assert!(top - bottom > 0.0 && top - bottom < 5.0);
    }

    #[test]
    fn colours_and_lineweights_are_as_plotted() {
        let (mut doc, id) = plotted(&[]);
        doc.set_active_layout(Some(id));
        let attrs = |c: Color, w: u16| Attributes {
            color: Attr::Set(c),
            lineweight: Attr::Set(Lineweight(w)),
            ..Attributes::on_layer(crate::LayerId::DEFAULT)
        };
        doc.add_with(line((0.0, 0.0), (1.0, 0.0)), attrs(Color::Foreground, 50));
        doc.add_with(
            line((0.0, 1.0), (1.0, 1.0)),
            attrs(Color::Rgb(255, 255, 255), 0),
        );
        doc.add_with(
            line((0.0, 2.0), (1.0, 2.0)),
            attrs(Color::Rgb(200, 30, 30), 25),
        );
        let paths = plot_layout(&doc, id).unwrap().paths;
        assert_eq!((paths[0].rgb, paths[0].width), ([0, 0, 0], 0.5));
        assert_eq!(
            (paths[1].rgb, paths[1].width),
            ([0, 0, 0], 0.0),
            "white plots black, 0 is a hairline"
        );
        assert_eq!((paths[2].rgb, paths[2].width), ([200, 30, 30], 0.25));
    }

    fn length(path: &PlotPath) -> f64 {
        path.points
            .windows(2)
            .map(|w| (w[1].x - w[0].x).hypot(w[1].y - w[0].y))
            .sum()
    }

    #[test]
    fn a_dashed_construction_line_plots_as_dashes_within_the_window_only() {
        let dashed = Attributes {
            linetype: Attr::Set(crate::LinetypeId::DASHED),
            ..Attributes::on_layer(crate::LayerId::DEFAULT)
        };
        let (mut doc, id) = plotted(&[]);
        doc.add_with(
            Entity::Xline {
                origin: Point::new(5000.0, 4000.0),
                direction: Point::new(1.0, 0.0),
            },
            dashed,
        );
        let sheet = plot_layout(&doc, id).unwrap();
        // The pattern is laid in model units (12.7 of them, 0.127 mm at 1:100),
        // so a 100 mm window holds hundreds of dashes, not the millions a line
        // 2e7 units long would make.
        assert!(
            sheet.paths.len() > 100 && sheet.paths.len() < 2000,
            "{}",
            sheet.paths.len()
        );
        assert!(sheet.paths.iter().all(|p| p
            .points
            .iter()
            .all(|q| q.x >= 50.0 - 1e-6 && q.x <= 150.0 + 1e-6)));
        // The pattern lies where it would on the whole line: the same on a
        // second plot whose window moves along it.
        let again = plot_layout(&doc, id).unwrap();
        assert_eq!(sheet.paths, again.paths);
    }

    #[test]
    fn a_dashed_line_on_the_sheet_plots_as_dashes_and_a_continuous_one_whole() {
        let (mut doc, id) = plotted(&[]);
        doc.set_active_layout(Some(id));
        let dashed = Attributes {
            linetype: Attr::Set(crate::LinetypeId::DASHED),
            ..Attributes::on_layer(crate::LayerId::DEFAULT)
        };
        doc.add_with(line((0.0, 0.0), (100.0, 0.0)), dashed);
        doc.add(line((0.0, 10.0), (100.0, 10.0)));
        let paths = plot_layout(&doc, id).unwrap().paths;
        let (on_row, solid): (Vec<_>, Vec<_>) = paths.iter().partition(|p| p.points[0].y == 0.0);
        assert_eq!(on_row.len(), 5);
        assert_eq!(solid.len(), 1);
        assert!(on_row.iter().all(|p| length(p) >= 12.7 - 1e-9));
        assert_eq!(on_row[0].points[0].x, 0.0);
        assert_eq!(on_row[4].points[1].x, 100.0);
    }

    #[test]
    fn a_layers_linetype_is_followed() {
        let (mut doc, id) = plotted(&[]);
        doc.set_active_layout(Some(id));
        let mut layer = doc.layer(crate::LayerId::DEFAULT).unwrap().clone();
        layer.linetype = crate::LinetypeId::DOT;
        doc.replace_layer(layer);
        doc.add(line((0.0, 0.0), (50.0, 0.0)));
        let paths = plot_layout(&doc, id).unwrap().paths;
        // Dots every 6.35 mm: a hair of line each.
        assert!(paths.len() > 7 && paths.iter().all(|p| length(p) < 0.01));
    }

    #[test]
    fn dashes_in_a_viewport_are_laid_in_model_units_then_scaled() {
        let (mut doc, id) = plotted(&[]);
        let dashed = Attributes {
            linetype: Attr::Set(crate::LinetypeId::DASHED),
            ..Attributes::on_layer(crate::LayerId::DEFAULT)
        };
        doc.add_with(line((4900.0, 4000.0), (5100.0, 4000.0)), dashed);
        let paths = plot_layout(&doc, id).unwrap().paths;
        assert!(paths.len() > 5);
        // A model dash of 12.7 shows at 1:100 as 0.127 mm.
        assert!(paths.iter().any(|p| (length(p) - 0.127).abs() < 1e-6));
    }

    #[test]
    fn linetype_generation_decides_whether_each_side_restarts_the_pattern() {
        let (mut doc, id) = plotted(&[]);
        doc.set_active_layout(Some(id));
        let square = || Entity::Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(100.0, 0.0),
                Point::new(100.0, 100.0),
                Point::new(0.0, 100.0),
            ],
            closed: true,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        let attrs = |continuous: bool| Attributes {
            linetype: Attr::Set(crate::LinetypeId::DASHED),
            continuous_generation: continuous,
            ..Attributes::on_layer(crate::LayerId::DEFAULT)
        };
        doc.add_with(square(), attrs(false));
        let corner_dashes = |paths: &[PlotPath]| {
            let corners = [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)];
            corners
                .iter()
                .filter(|c| {
                    paths
                        .iter()
                        .any(|p| p.points.iter().any(|q| (q.x, q.y) == **c))
                })
                .count()
        };
        let per_segment = plot_layout(&doc, id).unwrap().paths;
        assert_eq!(
            corner_dashes(&per_segment),
            4,
            "every side starts with a dash"
        );

        let mut doc = plotted(&[]).0;
        let layout = doc.layouts()[0].id;
        doc.set_active_layout(Some(layout));
        doc.add_with(square(), attrs(true));
        let continuous = plot_layout(&doc, layout).unwrap().paths;
        assert!(continuous.len() < per_segment.len() || corner_dashes(&continuous) < 4);
    }

    fn with_flags(doc: &mut Document, id: LayoutId, f: impl FnOnce(&mut u16)) {
        let mut layouts = doc.layouts().to_vec();
        let layout = layouts.iter_mut().find(|l| l.id == id).unwrap();
        f(&mut layout.plot.flags);
        doc.set_layouts(layouts);
    }

    /// A sheet line at y = 5 and a model line the viewport shows.
    fn sheet_and_model_line() -> (Document, LayoutId) {
        let (mut doc, id) = plotted(&[line((4990.0, 4000.0), (5010.0, 4000.0))]);
        doc.set_active_layout(Some(id));
        doc.add(line((0.0, 5.0), (10.0, 5.0)));
        (doc, id)
    }

    #[test]
    fn paper_space_goes_over_the_viewports_unless_they_are_drawn_last() {
        let (mut doc, id) = sheet_and_model_line();
        let y_of_first = |doc: &Document| plot_layout(doc, id).unwrap().paths[0].points[0].y;
        assert_eq!(y_of_first(&doc), 100.0, "viewports first by default");
        with_flags(&mut doc, id, |f| *f &= !flags::DRAW_VIEWPORTS_FIRST);
        assert_eq!(y_of_first(&doc), 5.0, "paper space first");
    }

    #[test]
    fn viewport_borders_plot_only_when_asked() {
        let (mut doc, id) = sheet_and_model_line();
        let count = |doc: &Document| plot_layout(doc, id).unwrap().paths.len();
        assert_eq!(count(&doc), 2);
        with_flags(&mut doc, id, |f| *f |= flags::PLOT_VIEWPORT_BORDERS);
        let sheet = plot_layout(&doc, id).unwrap();
        assert_eq!(sheet.paths.len(), 3);
        let border = sheet.paths.iter().find(|p| p.closed).unwrap();
        assert_eq!(border.points[0], Point::new(50.0, 60.0));
        assert_eq!(border.points[2], Point::new(150.0, 140.0));
    }

    #[test]
    fn without_print_lineweights_every_line_is_a_hairline() {
        let (mut doc, id) = sheet_and_model_line();
        doc.set_active_layout(Some(id));
        let widths = |doc: &Document| -> Vec<f64> {
            plot_layout(doc, id)
                .unwrap()
                .paths
                .iter()
                .map(|p| p.width)
                .collect()
        };
        with_flags(&mut doc, id, |f| *f |= flags::SCALE_LINEWEIGHTS);
        assert!(widths(&doc).iter().all(|w| *w >= 0.0));
        with_flags(&mut doc, id, |f| *f &= !flags::PRINT_LINEWEIGHTS);
        assert!(widths(&doc).iter().all(|w| *w == 0.0));
    }

    #[test]
    fn a_fill_is_painted_between_the_paths_around_it() {
        let path = |x: f64| PlotPath {
            points: vec![Point::new(x, 0.0), Point::new(x, 1.0)],
            closed: false,
            rgb: [0, 0, 0],
            width: 0.1,
        };
        let mut sheet = PlotSheet {
            width: 10.0,
            height: 10.0,
            paths: vec![path(1.0), path(2.0)],
            fills: Vec::new(),
            notes: Vec::new(),
        };
        sheet.paths.truncate(1);
        sheet.push_fill(vec![vec![Point::new(0.0, 0.0)]], [0, 0, 0]);
        sheet.paths.push(path(2.0));
        sheet.push_fill(vec![vec![Point::new(0.0, 0.0)]], [1, 1, 1]);
        let order: Vec<char> = sheet
            .paint_order()
            .map(|p| match p {
                Painted::Path(_) => 'p',
                Painted::Fill(_) => 'f',
            })
            .collect();
        assert_eq!(order, ['p', 'f', 'p', 'f']);
    }

    #[test]
    fn circles_arcs_and_polylines_are_flattened_and_wide_ones_are_filled() {
        let (mut doc, id) = plotted(&[]);
        doc.set_active_layout(Some(id));
        doc.add(Entity::Circle {
            center: Point::new(50.0, 50.0),
            radius: 20.0,
        });
        doc.add(Entity::Arc {
            center: Point::new(0.0, 0.0),
            radius: 10.0,
            start: 0.0,
            end: 1.0,
        });
        doc.add(Entity::Rectangle {
            min: Point::new(0.0, 0.0),
            max: Point::new(5.0, 5.0),
        });
        doc.add(Entity::Polyline {
            points: vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)],
            closed: false,
            bulges: vec![1.0, 0.0],
            widths: vec![(1.0, 1.0), (0.0, 0.0)],
        });
        let sheet = plot_layout(&doc, id).unwrap();
        let circle = &sheet.paths[0];
        assert!(circle.closed && circle.points.len() > 16);
        let on_circle = circle
            .points
            .iter()
            .all(|p| ((p.x - 50.0).hypot(p.y - 50.0) - 20.0).abs() < 1e-9);
        assert!(on_circle);
        assert!(!sheet.paths[1].closed && sheet.paths[1].points.len() > 2);
        assert!(sheet.paths[2].closed && sheet.paths[2].points.len() == 4);
        assert_eq!(
            sheet.paths.len(),
            3,
            "the wide polyline is an area, not a line"
        );
        assert_eq!(sheet.fills.len(), 1);
        assert!(
            sheet.fills[0].contours.len() > 2,
            "the bulge became a curve"
        );
        assert!(sheet.notes.is_empty());
    }

    #[test]
    fn a_spline_plots_on_the_curve_and_a_huge_one_is_refused() {
        // A rational half circle of radius 400 about the view centre: 4 mm at 1:100.
        let w = 0.5f64.sqrt();
        let c = |x: f64, y: f64| Point::new(5000.0 + 400.0 * x, 4000.0 + 400.0 * y);
        let half = crate::Spline::new_nurbs(
            2,
            vec![
                c(1.0, 0.0),
                c(1.0, 1.0),
                c(0.0, 1.0),
                c(-1.0, 1.0),
                c(-1.0, 0.0),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 2.0],
            Some(vec![1.0, w, 1.0, w, 1.0]),
            false,
            false,
        )
        .unwrap();
        let (doc, id) = plotted(&[Entity::Spline(Box::new(half.clone()))]);
        let sheet = plot_layout(&doc, id).unwrap();
        let path = &sheet.paths[0];
        assert!(!path.closed && path.points.len() > 8);
        for q in &path.points {
            assert!(
                ((q.x - 100.0).hypot(q.y - 100.0) - 4.0).abs() < 1e-9,
                "{q:?}"
            );
        }
        let huge = half.scaled(Point::new(5000.0, 4000.0), 1e7);
        let (doc, id) = plotted(&[Entity::Spline(Box::new(huge))]);
        assert_eq!(plot_layout(&doc, id), Err(PlotError::TooDetailed));
    }

    #[test]
    fn an_ellipse_plots_closed_on_the_curve_and_a_huge_one_is_refused() {
        // 400 by 200 model units about the view centre: 4 by 2 mm at 1:100.
        let e = crate::Ellipse::full(Point::new(5000.0, 4000.0), Point::new(400.0, 0.0), 0.5);
        let (doc, id) = plotted(&[Entity::Ellipse(e.unwrap())]);
        let sheet = plot_layout(&doc, id).unwrap();
        assert_eq!(sheet.paths.len(), 1);
        let path = &sheet.paths[0];
        assert!(path.closed && path.points.len() > 8);
        for q in &path.points {
            let (dx, dy) = ((q.x - 100.0) / 4.0, (q.y - 100.0) / 2.0);
            assert!((dx.hypot(dy) - 1.0).abs() < 1e-9, "{q:?}");
        }
        let huge = crate::Ellipse::full(Point::new(5000.0, 4000.0), Point::new(1e10, 0.0), 0.5);
        let (doc, id) = plotted(&[Entity::Ellipse(huge.unwrap())]);
        assert_eq!(plot_layout(&doc, id), Err(PlotError::TooDetailed));
    }
}
