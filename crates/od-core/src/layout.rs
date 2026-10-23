//! Layouts: sheets of paper that show the drawing through viewports.
//!
//! Model space is the drawing's objects. A layout is a sheet (in millimetres)
//! holding viewports, each a rectangle on the sheet that shows part of model
//! space at a scale. Switching between model and paper editing never moves an
//! object: a viewport only maps coordinates (`model_to_paper`, `paper_to_model`).
//! Views are plan views, so the mapping is a uniform scale and an offset.

use crate::{Bounds, LayerId, Point};

/// Permanent identity of a layout. Never reused within a drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayoutId(pub u64);

/// Permanent identity of a viewport. Never reused within a drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ViewportId(pub u64);

/// Sheet sizes (width, height in millimetres, landscape) offered for a new layout.
pub const PAPER_SIZES: &[(&str, f64, f64)] = &[
    ("A4", 297.0, 210.0),
    ("A3", 420.0, 297.0),
    ("A2", 594.0, 420.0),
    ("A1", 841.0, 594.0),
    ("A0", 1189.0, 841.0),
    ("Letter", 279.4, 215.9),
    ("Tabloid", 431.8, 279.4),
];

/// The standard paper size a sheet is, in either orientation (within half a
/// millimetre), or `None` for a custom size.
pub fn paper_size_name(width: f64, height: f64) -> Option<&'static str> {
    let close = |a: f64, b: f64| (a - b).abs() < 0.5;
    PAPER_SIZES
        .iter()
        .find(|(_, w, h)| {
            (close(width, *w) && close(height, *h)) || (close(width, *h) && close(height, *w))
        })
        .map(|(name, ..)| *name)
}

/// A standard paper size by name, landscape or portrait.
pub fn paper_size(name: &str, landscape: bool) -> Option<(f64, f64)> {
    let (_, w, h) = PAPER_SIZES.iter().find(|(n, ..)| *n == name)?;
    Some(if landscape { (*w, *h) } else { (*h, *w) })
}

/// The smallest sheet a layout may have, in millimetres each way.
pub const MIN_SHEET: f64 = 10.0;

/// A window on the sheet showing model space.
///
/// `scale` is paper millimetres per model unit (a 1:50 viewport of a drawing in
/// millimetres has `scale` 0.02). The model point `view_center` appears at the
/// middle of the window.
#[derive(Clone, Debug, PartialEq)]
pub struct Viewport {
    pub id: ViewportId,
    /// Middle of the window on the sheet.
    pub center: Point,
    pub width: f64,
    pub height: f64,
    pub view_center: Point,
    pub scale: f64,
    /// A locked viewport keeps its view: zooming and panning move the sheet.
    pub locked: bool,
    /// An off viewport shows nothing but keeps its place on the sheet.
    pub on: bool,
    /// Layers hidden in this viewport only (VPLAYER), kept sorted. Other
    /// viewports and model space still show them.
    pub frozen: Vec<LayerId>,
}

impl Viewport {
    /// A window on the sheet that frames `model` (or the origin) as large as it
    /// fits, at a scale rounded down to a tidy ratio.
    pub fn framing(
        id: ViewportId,
        center: Point,
        width: f64,
        height: f64,
        model: Option<Bounds>,
    ) -> Self {
        let mut vp = Self {
            id,
            center,
            width,
            height,
            view_center: Point::new(0.0, 0.0),
            scale: 1.0,
            locked: false,
            on: true,
            frozen: Vec::new(),
        };
        if let Some(b) = model {
            vp.view_center = Point::new((b.min.x + b.max.x) / 2.0, (b.min.y + b.max.y) / 2.0);
            let (w, h) = (b.max.x - b.min.x, b.max.y - b.min.y);
            let fit = (width / w.max(1e-9)).min(height / h.max(1e-9)) * 0.9;
            if fit.is_finite() && fit > 0.0 {
                vp.scale = tidy_scale(fit);
            }
        }
        vp
    }

    /// Whether a layer is frozen in this viewport.
    pub fn is_frozen(&self, layer: LayerId) -> bool {
        self.frozen.binary_search(&layer).is_ok()
    }

    /// Freeze or thaw layers here, keeping the list sorted and without repeats.
    pub fn set_frozen(&mut self, layers: &[LayerId], freeze: bool) {
        if freeze {
            self.frozen.extend_from_slice(layers);
            self.frozen.sort_unstable();
            self.frozen.dedup();
        } else {
            self.frozen.retain(|l| !layers.contains(l));
        }
    }

    /// Whether the window can be drawn and inverted: finite and not flat.
    pub fn is_valid(&self) -> bool {
        let finite = [
            self.center.x,
            self.center.y,
            self.view_center.x,
            self.view_center.y,
            self.width,
            self.height,
            self.scale,
        ]
        .iter()
        .all(|v| v.is_finite());
        finite && self.width > 0.0 && self.height > 0.0 && self.scale > 0.0
    }

    pub fn model_to_paper(&self, p: Point) -> Point {
        Point::new(
            self.center.x + self.scale * (p.x - self.view_center.x),
            self.center.y + self.scale * (p.y - self.view_center.y),
        )
    }

    pub fn paper_to_model(&self, p: Point) -> Point {
        Point::new(
            self.view_center.x + (p.x - self.center.x) / self.scale,
            self.view_center.y + (p.y - self.center.y) / self.scale,
        )
    }

    /// The window on the sheet.
    pub fn paper_bounds(&self) -> Bounds {
        Bounds::from_points(
            Point::new(
                self.center.x - self.width / 2.0,
                self.center.y - self.height / 2.0,
            ),
            Point::new(
                self.center.x + self.width / 2.0,
                self.center.y + self.height / 2.0,
            ),
        )
    }

    /// The part of model space the window shows.
    pub fn model_bounds(&self) -> Bounds {
        let b = self.paper_bounds();
        Bounds::from_points(self.paper_to_model(b.min), self.paper_to_model(b.max))
    }

    pub fn contains_paper(&self, p: Point) -> bool {
        let b = self.paper_bounds();
        p.x >= b.min.x && p.x <= b.max.x && p.y >= b.min.y && p.y <= b.max.y
    }

    /// Change the scale, keeping the model point `about` where it is on the sheet.
    pub fn zoomed_about_paper(&self, about: Point, factor: f64) -> Self {
        if !factor.is_finite() || factor <= 0.0 {
            return self.clone();
        }
        let fixed = self.paper_to_model(about);
        let mut vp = self.clone();
        vp.scale = (self.scale * factor).clamp(1e-6, 1e6);
        let moved = vp.paper_to_model(about);
        vp.view_center = Point::new(
            vp.view_center.x + fixed.x - moved.x,
            vp.view_center.y + fixed.y - moved.y,
        );
        vp
    }

    /// Slide the view of model space so its content moves by (`dx`, `dy`) on the sheet.
    pub fn panned_by_paper(&self, dx: f64, dy: f64) -> Self {
        let mut vp = self.clone();
        vp.view_center = Point::new(
            self.view_center.x - dx / self.scale,
            self.view_center.y - dy / self.scale,
        );
        vp
    }
}

/// Round a scale down to 1, 2, 2.5, 5 times a power of ten (1:50, 1:100, 2:1...).
fn tidy_scale(scale: f64) -> f64 {
    let exp = scale.log10().floor();
    let base = 10f64.powf(exp);
    let m = scale / base;
    let step = [1.0, 2.0, 2.5, 5.0, 10.0]
        .into_iter()
        .rev()
        .find(|s| *s <= m + 1e-9)
        .unwrap_or(1.0);
    step * base
}

/// Read a viewport scale: `1:50` (one paper unit is 50 model units), `2:1`,
/// `1/50`, or a bare number taken as paper units per model unit.
pub fn parse_scale(text: &str) -> Option<f64> {
    let text = text.trim();
    let ratio = text.split_once(':').or_else(|| text.split_once('/'));
    let scale = match ratio {
        Some((paper, model)) => {
            paper.trim().parse::<f64>().ok()? / model.trim().parse::<f64>().ok()?
        }
        None => text.parse::<f64>().ok()?,
    };
    (scale.is_finite() && scale > 0.0).then_some(scale)
}

/// A scale as people write it: `1:50`, `2:1`, `1:1`; odd ones as a number.
pub fn format_scale(scale: f64) -> String {
    let tidy = |n: f64| (n - n.round()).abs() < 1e-9 * n.max(1.0);
    if scale >= 1.0 && tidy(scale) {
        format!("{}:1", scale.round())
    } else if scale < 1.0 && tidy(1.0 / scale) {
        format!("1:{}", (1.0 / scale).round())
    } else {
        format!("{scale}")
    }
}

/// A named sheet and the viewports on it.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub id: LayoutId,
    pub name: String,
    /// Sheet size in millimetres.
    pub width: f64,
    pub height: f64,
    /// Viewports in stacking order, the last on top.
    pub viewports: Vec<Viewport>,
}

impl Layout {
    pub fn viewport(&self, id: ViewportId) -> Option<&Viewport> {
        self.viewports.iter().find(|v| v.id == id)
    }

    pub fn sheet_bounds(&self) -> Bounds {
        Bounds::from_points(Point::new(0.0, 0.0), Point::new(self.width, self.height))
    }

    /// The top viewport that is on and holds the sheet point.
    pub fn viewport_at(&self, p: Point) -> Option<ViewportId> {
        self.viewports
            .iter()
            .rev()
            .find(|v| v.on && v.is_valid() && v.contains_paper(p))
            .map(|v| v.id)
    }

    /// The viewport `MSPACE` enters when none is named: the topmost one that is on.
    pub fn default_viewport(&self) -> Option<ViewportId> {
        self.viewports
            .iter()
            .rev()
            .find(|v| v.on && v.is_valid())
            .map(|v| v.id)
    }

    /// The sheet is usable: positive size, and every viewport has a unique id.
    pub fn is_valid(&self) -> bool {
        let mut ids: Vec<_> = self.viewports.iter().map(|v| v.id).collect();
        ids.sort_unstable();
        ids.dedup();
        self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0
            && ids.len() == self.viewports.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewport() -> Viewport {
        Viewport {
            id: ViewportId(1),
            center: Point::new(150.0, 100.0),
            width: 200.0,
            height: 120.0,
            view_center: Point::new(5000.0, 3000.0),
            scale: 0.02,
            locked: false,
            on: true,
            frozen: Vec::new(),
        }
    }

    fn close(a: Point, b: Point) -> bool {
        (a.x - b.x).abs() < 1e-9 * a.x.abs().max(1.0)
            && (a.y - b.y).abs() < 1e-9 * a.y.abs().max(1.0)
    }

    #[test]
    fn model_to_paper_and_back_round_trips() {
        let vp = viewport();
        for p in [
            Point::new(0.0, 0.0),
            Point::new(5000.0, 3000.0),
            Point::new(-1234.5, 99999.0),
        ] {
            assert!(close(vp.paper_to_model(vp.model_to_paper(p)), p));
        }
        assert_eq!(vp.model_to_paper(vp.view_center), vp.center);
    }

    #[test]
    fn scale_is_paper_height_over_model_height() {
        let vp = viewport();
        let m = vp.model_bounds();
        let model_height = m.max.y - m.min.y;
        assert!((vp.height / model_height - vp.scale).abs() < 1e-12);
        assert!((model_height - 6000.0).abs() < 1e-9, "120 mm at 1:50");
    }

    #[test]
    fn containment_follows_the_window() {
        let vp = viewport();
        assert!(vp.contains_paper(Point::new(150.0, 100.0)));
        assert!(
            vp.contains_paper(Point::new(50.0, 40.0)),
            "corner is inside"
        );
        assert!(!vp.contains_paper(Point::new(49.9, 100.0)));
    }

    #[test]
    fn zooming_keeps_the_point_under_the_pointer() {
        let vp = viewport();
        let about = Point::new(120.0, 80.0);
        let before = vp.paper_to_model(about);
        let zoomed = vp.zoomed_about_paper(about, 2.0);
        assert!((zoomed.scale - 0.04).abs() < 1e-12);
        assert!(close(zoomed.paper_to_model(about), before));
        assert_eq!(vp.zoomed_about_paper(about, f64::NAN), vp);
        assert_eq!(vp.zoomed_about_paper(about, -1.0), vp);
    }

    #[test]
    fn panning_moves_content_with_the_pointer() {
        let vp = viewport();
        let p = Point::new(5100.0, 3100.0);
        let before = vp.model_to_paper(p);
        let after = vp.panned_by_paper(10.0, -4.0).model_to_paper(p);
        assert!((after.x - before.x - 10.0).abs() < 1e-9);
        assert!((after.y - before.y + 4.0).abs() < 1e-9);
    }

    #[test]
    fn degenerate_viewports_are_not_valid() {
        assert!(viewport().is_valid());
        for bad in [
            Viewport {
                width: 0.0,
                ..viewport()
            },
            Viewport {
                height: -1.0,
                ..viewport()
            },
            Viewport {
                scale: 0.0,
                ..viewport()
            },
            Viewport {
                scale: f64::NAN,
                ..viewport()
            },
            Viewport {
                center: Point::new(f64::INFINITY, 0.0),
                ..viewport()
            },
        ] {
            assert!(!bad.is_valid());
        }
    }

    #[test]
    fn framing_fits_the_model_at_a_tidy_scale() {
        let b = Bounds::from_points(Point::new(0.0, 0.0), Point::new(10000.0, 6000.0));
        let vp = Viewport::framing(
            ViewportId(1),
            Point::new(150.0, 100.0),
            280.0,
            190.0,
            Some(b),
        );
        assert_eq!(vp.view_center, Point::new(5000.0, 3000.0));
        assert!((vp.scale - 0.025).abs() < 1e-12, "1:40, got {}", vp.scale);
        let shown = vp.model_bounds();
        assert!(shown.min.x <= b.min.x && shown.max.x >= b.max.x);
        let empty = Viewport::framing(ViewportId(2), Point::new(0.0, 0.0), 10.0, 10.0, None);
        assert_eq!(empty.scale, 1.0);
    }

    #[test]
    fn tidy_scales_round_down() {
        for (input, want) in [
            (0.0213, 0.02),
            (0.03, 0.025),
            (0.07, 0.05),
            (1.0, 1.0),
            (3.0, 2.5),
            (120.0, 100.0),
        ] {
            assert!((tidy_scale(input) - want).abs() < 1e-12, "{input}");
        }
    }

    #[test]
    fn layers_freeze_and_thaw_per_viewport_without_repeats() {
        let mut vp = viewport();
        let (a, b, c) = (LayerId(3), LayerId(1), LayerId(2));
        vp.set_frozen(&[a, b, a], true);
        assert_eq!(vp.frozen, [b, a], "sorted, no repeats");
        assert!(vp.is_frozen(a) && !vp.is_frozen(c));
        vp.set_frozen(&[b, c], false);
        assert_eq!(vp.frozen, [a]);
        vp.set_frozen(&[a], false);
        assert!(vp.frozen.is_empty());
    }

    #[test]
    fn standard_paper_sizes_are_found_in_either_orientation() {
        assert_eq!(paper_size_name(297.0, 210.0), Some("A4"));
        assert_eq!(paper_size_name(210.0, 297.0), Some("A4"));
        assert_eq!(paper_size_name(420.2, 297.0), Some("A3"));
        assert_eq!(paper_size_name(300.0, 200.0), None);
        assert_eq!(paper_size("A3", true), Some((420.0, 297.0)));
        assert_eq!(paper_size("A3", false), Some((297.0, 420.0)));
        assert_eq!(paper_size("B5", true), None);
    }

    #[test]
    fn scales_are_read_and_written_as_ratios() {
        for (text, want) in [
            ("1:50", 0.02),
            (" 2 : 1 ", 2.0),
            ("1/100", 0.01),
            ("0.5", 0.5),
        ] {
            assert!((parse_scale(text).unwrap() - want).abs() < 1e-12, "{text}");
        }
        for bad in ["", "abc", "1:0", "0:5", "-1:2", "1:", "nan"] {
            assert_eq!(parse_scale(bad), None, "{bad:?}");
        }
        for (scale, text) in [(0.02, "1:50"), (1.0, "1:1"), (4.0, "4:1"), (0.025, "1:40")] {
            assert_eq!(format_scale(scale), text);
            assert!((parse_scale(&format_scale(scale)).unwrap() - scale).abs() < 1e-12);
        }
        assert_eq!(format_scale(0.3), "0.3");
    }

    #[test]
    fn the_top_visible_viewport_wins_overlaps() {
        let under = viewport();
        let over = Viewport {
            id: ViewportId(2),
            width: 40.0,
            height: 40.0,
            ..viewport()
        };
        let off = Viewport {
            id: ViewportId(3),
            on: false,
            ..viewport()
        };
        let layout = Layout {
            id: LayoutId(1),
            name: "A".into(),
            width: 297.0,
            height: 210.0,
            viewports: vec![under, over, off],
        };
        assert_eq!(
            layout.viewport_at(Point::new(150.0, 100.0)),
            Some(ViewportId(2))
        );
        assert_eq!(
            layout.viewport_at(Point::new(70.0, 100.0)),
            Some(ViewportId(1))
        );
        assert_eq!(layout.viewport_at(Point::new(5.0, 5.0)), None);
        assert_eq!(layout.default_viewport(), Some(ViewportId(2)));
        assert!(layout.is_valid());
    }
}
