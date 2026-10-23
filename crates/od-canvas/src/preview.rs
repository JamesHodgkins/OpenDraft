//! A thumbnail of a drawing, drawn on the CPU with no window or GPU: the same
//! plotted paths and fills as a plot (`od_core::plot_model`), fitted to a
//! fixed picture and rasterised with one-pixel anti-aliased lines. The Home
//! page keeps these as its recent-drawing previews.
//!
//! The picture is on dark grey, like the drawing area: the plot's black (which is
//! what it makes of white and of the foreground colour) is drawn white, and
//! other colours as they are. Every line is one pixel wide whatever its
//! lineweight.

use od_core::plot::{PlotError, PlotSheet};
use od_core::plot_model::plot_region;
use od_core::{Bounds, Document, Point};

/// The picture's size, and so its shape: 16:10.
pub const WIDTH: u32 = 512;
pub const HEIGHT: u32 = 320;
/// Share of the picture left empty on each side of the drawing.
const MARGIN: f64 = 0.06;
/// The plot works in millimetres, where its tolerance is 0.02: a sheet this
/// wide is 20 pixels to the millimetre, so curves stay within half a pixel.
const SHEET_WIDTH: f64 = 25.6;
/// What the picture is drawn on: a dark grey, a little lighter than the page.
pub const BACKGROUND: [u8; 3] = [43, 43, 43];
/// Subsamples per pixel row when filling.
const SUBROWS: usize = 4;

/// A picture: `width` x `height` pixels, RGBA, rows from the top.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preview {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Why a drawing could not be pictured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewError {
    /// The plot refused: too detailed, or a hatch too dense.
    Plot(PlotError),
    /// The drawing's extents are not usable numbers.
    Extents,
}

impl std::fmt::Display for PreviewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plot(e) => e.fmt(f),
            Self::Extents => f.write_str("the drawing's extents are not usable"),
        }
    }
}

/// The part of model space the picture shows for a drawing spanning `bounds`:
/// centred, with `MARGIN` on each side of the longer fit. `None` if the extents
/// are not finite.
pub fn window(bounds: Bounds) -> Option<Bounds> {
    let (w, h) = (bounds.max.x - bounds.min.x, bounds.max.y - bounds.min.y);
    if !(w.is_finite() && h.is_finite() && w >= 0.0 && h >= 0.0) {
        return None;
    }
    let room = 1.0 - 2.0 * MARGIN;
    let (pw, ph) = (f64::from(WIDTH) * room, f64::from(HEIGHT) * room);
    // Pixels per drawing unit; a lone point gets one to the unit.
    let scale = match (w > 0.0, h > 0.0) {
        (true, true) => (pw / w).min(ph / h),
        (true, false) => pw / w,
        (false, true) => ph / h,
        (false, false) => 1.0,
    };
    if !scale.is_finite() {
        return None;
    }
    let centre = Point::new(
        (bounds.min.x + bounds.max.x) / 2.0,
        (bounds.min.y + bounds.max.y) / 2.0,
    );
    let (half_w, half_h) = (
        f64::from(WIDTH) / scale / 2.0,
        f64::from(HEIGHT) / scale / 2.0,
    );
    Some(Bounds::from_points(
        Point::new(centre.x - half_w, centre.y - half_h),
        Point::new(centre.x + half_w, centre.y + half_h),
    ))
}

/// Share of objects, at each end, that may be set aside as strays when
/// working out what to show.
const STRAYS: f64 = 0.02;
/// Below this many objects, every one counts.
const FEW: usize = 50;

/// What to fit: the extents of the drawing's visible model-space objects,
/// less strays. A drawing with something far from the rest (a stray point,
/// a block left at the origin) would otherwise show as a speck in empty
/// space; objects well outside the middle of the drawing's centres, by
/// `STRAYS` at each end and as far again, are left out of the fit (they are
/// still drawn, cut to the picture, if they reach into it).
pub fn content_bounds(document: &Document) -> Option<Bounds> {
    let all: Vec<Bounds> = document
        .visible_in(None)
        .map(|o| o.bounds())
        .filter(|b| {
            [b.min.x, b.min.y, b.max.x, b.max.y]
                .iter()
                .all(|v| v.is_finite())
        })
        .collect();
    let union = |boxes: &[&Bounds]| {
        boxes.iter().fold(None, |acc: Option<Bounds>, b| {
            Some(acc.map_or(**b, |a| {
                Bounds::from_points(
                    Point::new(a.min.x.min(b.min.x), a.min.y.min(b.min.y)),
                    Point::new(a.max.x.max(b.max.x), a.max.y.max(b.max.y)),
                )
            }))
        })
    };
    let everything: Vec<&Bounds> = all.iter().collect();
    if all.len() < FEW {
        return union(&everything);
    }
    // The span of the middle of the centres, widened by its own length.
    let spread = |centre: fn(&Bounds) -> f64| {
        let mut values: Vec<f64> = all.iter().map(centre).collect();
        values.sort_by(f64::total_cmp);
        let at = |share: f64| values[((values.len() - 1) as f64 * share).round() as usize];
        let (low, high) = (at(STRAYS), at(1.0 - STRAYS));
        (low - (high - low), high + (high - low))
    };
    let (x_range, y_range) = (
        spread(|b| (b.min.x + b.max.x) / 2.0),
        spread(|b| (b.min.y + b.max.y) / 2.0),
    );
    let inside: Vec<&Bounds> = all
        .iter()
        .filter(|b| {
            b.min.x >= x_range.0
                && b.max.x <= x_range.1
                && b.min.y >= y_range.0
                && b.max.y <= y_range.1
        })
        .collect();
    union(if inside.is_empty() {
        &everything
    } else {
        &inside
    })
}

/// The picture of `document`'s model space; a blank grid when it is empty.
pub fn render(document: &Document) -> Result<Preview, PreviewError> {
    let Some(bounds) = content_bounds(document) else {
        return Ok(blank());
    };
    let region = window(bounds).ok_or(PreviewError::Extents)?;
    let sheet = plot_region(document, region, SHEET_WIDTH, None).map_err(PreviewError::Plot)?;
    Ok(render_sheet(&sheet))
}

/// The picture for an empty drawing: faint grid on dark grey.
pub fn blank() -> Preview {
    let mut canvas = Canvas::new();
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let major = x % 128 == 0 || y % 128 == 0;
            let minor = x % 32 == 0 || y % 32 == 0;
            let grey = match (major, minor) {
                (true, _) => 78,
                (_, true) => 58,
                _ => continue,
            };
            canvas.blend(x as i64, y as i64, [grey; 3], 1.0);
        }
    }
    canvas.into_preview()
}

/// A plotted colour as it shows on the dark background: the plot's black, and anything
/// else too dark to see, becomes white.
fn on_black(rgb: [u8; 3]) -> [u8; 3] {
    if rgb.iter().all(|c| *c < 64) {
        [255; 3]
    } else {
        rgb
    }
}

/// A plotted sheet as a picture, its width spread over the picture's.
pub fn render_sheet(sheet: &PlotSheet) -> Preview {
    let mut canvas = Canvas::new();
    if !(sheet.width > 0.0 && sheet.width.is_finite() && sheet.height.is_finite()) {
        return canvas.into_preview();
    }
    let k = f64::from(WIDTH) / sheet.width;
    let to_px = |p: &Point| ((p.x * k) as f32, ((sheet.height - p.y) * k) as f32);
    for path in &sheet.paths {
        let points: Vec<(f32, f32)> = path.points.iter().map(to_px).collect();
        for pair in points.windows(2) {
            canvas.line(pair[0], pair[1], on_black(path.rgb));
        }
        if path.closed && points.len() > 2 {
            canvas.line(points[points.len() - 1], points[0], on_black(path.rgb));
        }
    }
    for fill in &sheet.fills {
        let contours: Vec<Vec<(f32, f32)>> = fill
            .contours
            .iter()
            .map(|c| c.iter().map(to_px).collect())
            .collect();
        canvas.fill(&contours, on_black(fill.rgb));
    }
    canvas.into_preview()
}

struct Canvas {
    rgba: Vec<u8>,
}

impl Canvas {
    fn new() -> Self {
        let mut rgba = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);
        for _ in 0..WIDTH * HEIGHT {
            rgba.extend([BACKGROUND[0], BACKGROUND[1], BACKGROUND[2], 255]);
        }
        Self { rgba }
    }

    fn into_preview(self) -> Preview {
        Preview {
            width: WIDTH,
            height: HEIGHT,
            rgba: self.rgba,
        }
    }

    /// Lay `rgb` over pixel (`x`, `y`) with opacity `alpha`; off the picture
    /// does nothing.
    fn blend(&mut self, x: i64, y: i64, rgb: [u8; 3], alpha: f32) {
        if x < 0 || y < 0 || x >= i64::from(WIDTH) || y >= i64::from(HEIGHT) || alpha <= 0.0 {
            return;
        }
        let a = alpha.min(1.0);
        let at = ((y as usize) * WIDTH as usize + x as usize) * 4;
        for (i, c) in rgb.iter().enumerate() {
            let under = f32::from(self.rgba[at + i]);
            self.rgba[at + i] = (under + (f32::from(*c) - under) * a).round() as u8;
        }
    }

    /// A one-pixel anti-aliased line (Wu's: each step along the longer axis
    /// shares its weight between the two pixels the line passes between).
    fn line(&mut self, from: (f32, f32), to: (f32, f32), rgb: [u8; 3]) {
        if ![from.0, from.1, to.0, to.1].iter().all(|v| v.is_finite()) {
            return;
        }
        let steep = (to.1 - from.1).abs() > (to.0 - from.0).abs();
        let swap = |p: (f32, f32)| if steep { (p.1, p.0) } else { p };
        let (mut a, mut b) = (swap(from), swap(to));
        if a.0 > b.0 {
            std::mem::swap(&mut a, &mut b);
        }
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let slope = if dx > 0.0 { dy / dx } else { 0.0 };
        // A line under a pixel long (a dot) still leaves a mark.
        let tiny = dx.hypot(dy) < 1.0;
        let first = a.0.floor() as i64;
        let last = b.0.floor() as i64;
        // Nothing of the line is on the picture: skip the walk.
        let (lo, hi) = (first.max(-1), last.min(i64::from(WIDTH.max(HEIGHT)) + 1));
        for xi in lo..=hi {
            let reach = ((xi + 1) as f32).min(b.0) - (xi as f32).max(a.0);
            let weight = if tiny { 0.5 } else { reach.clamp(0.0, 1.0) };
            let across = a.1 + slope * (((xi as f32) + 0.5).clamp(a.0, b.0) - a.0) - 0.5;
            let base = across.floor();
            let frac = across - base;
            let base = base as i64;
            for (offset, share) in [(0, 1.0 - frac), (1, frac)] {
                let (x, y) = if steep {
                    (base + offset, xi)
                } else {
                    (xi, base + offset)
                };
                self.blend(x, y, rgb, weight * share);
            }
        }
    }

    /// Fill `contours` together by the non-zero winding rule, anti-aliased
    /// by `SUBROWS` samples a row and exact coverage along each.
    fn fill(&mut self, contours: &[Vec<(f32, f32)>], rgb: [u8; 3]) {
        let mut edges = Vec::new();
        for contour in contours {
            if contour.len() < 3 {
                continue;
            }
            for (i, a) in contour.iter().enumerate() {
                let b = contour[(i + 1) % contour.len()];
                if [a.0, a.1, b.0, b.1].iter().all(|v| v.is_finite()) && a.1 != b.1 {
                    edges.push((*a, b));
                }
            }
        }
        let Some((top, bottom)) = edges.iter().fold(None, |acc: Option<(f32, f32)>, (a, b)| {
            let (lo, hi) = (a.1.min(b.1), a.1.max(b.1));
            Some(acc.map_or((lo, hi), |(l, h)| (l.min(lo), h.max(hi))))
        }) else {
            return;
        };
        let first = top.floor().max(0.0) as usize;
        let last = (bottom.ceil().max(0.0) as usize).min(HEIGHT as usize);
        let mut coverage = vec![0.0_f32; WIDTH as usize];
        let mut crossings: Vec<(f32, i32)> = Vec::new();
        for row in first..last {
            coverage.fill(0.0);
            let mut touched = false;
            for sub in 0..SUBROWS {
                let y = row as f32 + (sub as f32 + 0.5) / SUBROWS as f32;
                crossings.clear();
                for (a, b) in &edges {
                    let (lo, hi, dir) = if a.1 < b.1 { (a, b, 1) } else { (b, a, -1) };
                    if y >= lo.1 && y < hi.1 {
                        let t = (y - lo.1) / (hi.1 - lo.1);
                        crossings.push((lo.0 + t * (hi.0 - lo.0), dir));
                    }
                }
                crossings.sort_by(|p, q| p.0.total_cmp(&q.0));
                let mut winding = 0;
                for pair in crossings.windows(2) {
                    winding += pair[0].1;
                    if winding != 0 {
                        add_span(&mut coverage, pair[0].0, pair[1].0, 1.0 / SUBROWS as f32);
                        touched = true;
                    }
                }
            }
            if touched {
                for (x, c) in coverage.iter().enumerate() {
                    self.blend(x as i64, row as i64, rgb, *c);
                }
            }
        }
    }
}

/// Add `weight` to the pixels between `x0` and `x1`, partly for the ends.
fn add_span(coverage: &mut [f32], x0: f32, x1: f32, weight: f32) {
    let (x0, x1) = (x0.max(0.0), x1.min(coverage.len() as f32));
    if x1 <= x0 {
        return;
    }
    let (first, last) = (
        x0.floor() as usize,
        (x1.ceil() as usize).min(coverage.len()),
    );
    for (i, slot) in coverage.iter_mut().enumerate().take(last).skip(first) {
        let overlap = ((i + 1) as f32).min(x1) - (i as f32).max(x0);
        *slot += overlap.clamp(0.0, 1.0) * weight;
    }
}

#[cfg(test)]
#[path = "preview_tests.rs"]
mod tests;
