//! The bundled font: metrics, and glyph outlines as flat contours.
//!
//! The application ships DejaVu Sans (see `assets/fonts/LICENSE-DejaVu.txt`)
//! so text looks the same on every machine and in every plot. A style that
//! asks for another font still gets this one, and says so in the layout's
//! diagnostics; what the style asked for is never overwritten.

use std::sync::OnceLock;

use rustybuzz::ttf_parser::{GlyphId, OutlineBuilder};
use rustybuzz::Face;

/// One of the four bundled DejaVu faces, or OD Roman, the stroke font.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FontFace {
    #[default]
    Regular,
    Bold,
    Italic,
    BoldItalic,
    /// OD Roman: its glyph ids are slots in `stroke_font::od_roman()` and
    /// its outlines depend on a pen weight.
    OdRoman,
}

impl FontFace {
    pub fn new(bold: bool, italic: bool) -> Self {
        match (bold, italic) {
            (false, false) => Self::Regular,
            (true, false) => Self::Bold,
            (false, true) => Self::Italic,
            (true, true) => Self::BoldItalic,
        }
    }

    fn index(self) -> usize {
        self as usize
    }

    pub fn is_stroke(self) -> bool {
        self == Self::OdRoman
    }
}

static BYTES: [&[u8]; 4] = [
    include_bytes!("../../assets/fonts/DejaVuSans.ttf"),
    include_bytes!("../../assets/fonts/DejaVuSans-Bold.ttf"),
    include_bytes!("../../assets/fonts/DejaVuSans-Oblique.ttf"),
    include_bytes!("../../assets/fonts/DejaVuSans-BoldOblique.ttf"),
];

/// The shaping face, or `None` if the bundled bytes were somehow unreadable.
pub(crate) fn face_of(face: FontFace) -> Option<&'static Face<'static>> {
    static FACES: OnceLock<[Option<Face<'static>>; 4]> = OnceLock::new();
    FACES
        .get_or_init(|| BYTES.map(|bytes| Face::from_slice(bytes, 0)))
        .get(face.index())?
        .as_ref()
}

/// The measures of a face.
pub(crate) fn metrics_of(face: FontFace) -> Option<Metrics> {
    static METRICS: OnceLock<[Option<Metrics>; 4]> = OnceLock::new();
    METRICS
        .get_or_init(|| {
            [
                FontFace::Regular,
                FontFace::Bold,
                FontFace::Italic,
                FontFace::BoldItalic,
            ]
            .map(|f| face_of(f).map(metrics))
        })
        .get(face.index())
        .copied()
        .flatten()
}

/// The regular face.
pub(crate) fn face() -> Option<&'static Face<'static>> {
    face_of(FontFace::Regular)
}

/// Font-unit measures a layout needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Metrics {
    /// The height of a capital letter, which is what a text height means.
    pub cap_height: f64,
    /// Below the baseline, positive.
    pub descent: f64,
}

pub(crate) fn metrics(face: &Face) -> Metrics {
    let em = f64::from(face.units_per_em());
    // The height of an H: older fonts leave the OS/2 cap height unset.
    let cap = face
        .glyph_index('H')
        .and_then(|g| face.glyph_bounding_box(g))
        .map(|b| f64::from(b.y_max))
        .or_else(|| face.capital_height().map(f64::from))
        .unwrap_or(0.7 * em);
    Metrics {
        cap_height: cap.max(1.0),
        descent: f64::from(face.descender()).abs(),
    }
}

/// A closed outline, in font units, y up.
pub type Contour = Vec<(f64, f64)>;

/// The outline of `glyph` in `face` as straight-edged contours, curves flattened to
/// within `tolerance` font units. Empty for a glyph with no outline (a space).
pub fn glyph_contours(face: FontFace, glyph: u16, tolerance: f64) -> Vec<Contour> {
    glyph_outline(face, glyph, 0, tolerance)
}

/// [`glyph_contours`] for a pen `weight` in thousandths of the cap height,
/// which only a stroke face looks at (0 is that font's default weight).
pub fn glyph_outline(face: FontFace, glyph: u16, weight: u16, tolerance: f64) -> Vec<Contour> {
    if face.is_stroke() {
        let Some(font) = super::stroke_font::od_roman() else {
            return Vec::new();
        };
        let fraction = if weight == 0 {
            font.metrics.default_weight
        } else {
            f64::from(weight) / 1000.0
        };
        return font.glyph(glyph).map_or_else(Vec::new, |g| {
            super::stroke_outline::stroke_contours(g, fraction, font.metrics.cap_height, tolerance)
        });
    }
    let Some(face) = face_of(face) else {
        return Vec::new();
    };
    let mut flat = Flattener {
        contours: Vec::new(),
        current: Vec::new(),
        tolerance: tolerance.max(1e-6),
    };
    if face.outline_glyph(GlyphId(glyph), &mut flat).is_none() {
        return Vec::new();
    }
    flat.finish();
    flat.contours
}

struct Flattener {
    contours: Vec<Contour>,
    current: Contour,
    tolerance: f64,
}

impl Flattener {
    fn finish(&mut self) {
        // A contour that returns to its start does not repeat it.
        if self.current.len() > 1 && self.current.first() == self.current.last() {
            self.current.pop();
        }
        if self.current.len() > 2 {
            self.contours.push(std::mem::take(&mut self.current));
        } else {
            self.current.clear();
        }
    }

    fn last(&self) -> (f64, f64) {
        self.current.last().copied().unwrap_or((0.0, 0.0))
    }
}

/// How many straight pieces keep a curve within tolerance, where `bend` is
/// the largest second difference of its control points.
pub(super) fn pieces(bend: f64, factor: f64, tolerance: f64) -> usize {
    ((factor * bend / tolerance).sqrt().ceil() as usize).clamp(1, 64)
}

impl OutlineBuilder for Flattener {
    fn move_to(&mut self, x: f32, y: f32) {
        self.finish();
        self.current.push((f64::from(x), f64::from(y)));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.current.push((f64::from(x), f64::from(y)));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (p0, p1, p2) = (
            self.last(),
            (f64::from(x1), f64::from(y1)),
            (f64::from(x), f64::from(y)),
        );
        let bend = (p0.0 - 2.0 * p1.0 + p2.0).hypot(p0.1 - 2.0 * p1.1 + p2.1);
        let n = pieces(bend, 0.25, self.tolerance);
        for i in 1..=n {
            let t = i as f64 / n as f64;
            let u = 1.0 - t;
            self.current.push((
                u * u * p0.0 + 2.0 * u * t * p1.0 + t * t * p2.0,
                u * u * p0.1 + 2.0 * u * t * p1.1 + t * t * p2.1,
            ));
        }
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let p0 = self.last();
        let (p1, p2, p3) = (
            (f64::from(x1), f64::from(y1)),
            (f64::from(x2), f64::from(y2)),
            (f64::from(x), f64::from(y)),
        );
        let bend_a = (p0.0 - 2.0 * p1.0 + p2.0).hypot(p0.1 - 2.0 * p1.1 + p2.1);
        let bend_b = (p1.0 - 2.0 * p2.0 + p3.0).hypot(p1.1 - 2.0 * p2.1 + p3.1);
        let n = pieces(bend_a.max(bend_b), 0.75, self.tolerance);
        for i in 1..=n {
            let t = i as f64 / n as f64;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            self.current.push((
                a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0,
                a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1,
            ));
        }
    }

    fn close(&mut self) {
        self.finish();
    }
}
