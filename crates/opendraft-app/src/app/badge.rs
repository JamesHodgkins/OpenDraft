//! The OpenDraft badge (`assets/badge_logo_dark.svg`) drawn with the painter,
//! so the app needs no SVG loader.

use eframe::egui::{self, Color32, Pos2, Shape};

/// The logo's artboard is 200 x 200.
const ART: f32 = 200.0;
/// Leg length of the sharp triangle behind each rounded half.
const LEG: f32 = 185.94;
const RADIUS: f32 = 8.36;
const HOLE_MIN: f32 = 28.452;
const HOLE_MAX: f32 = 117.256;
const ARC_STEPS: usize = 6;

const GREEN: Color32 = Color32::from_rgb(0x0f, 0xb8, 0x81);
const GREY: Color32 = Color32::from_rgb(0xe4, 0xe6, 0xea);

/// Paint the badge into `rect`. `hole` is the colour behind it, which shows
/// through the two triangular holes.
pub(super) fn paint(painter: &egui::Painter, rect: egui::Rect, hole: Color32) {
    let scale = rect.width().min(rect.height()) / ART;
    let at = |p: Pos2| rect.min + egui::vec2(p.x, p.y) * scale;
    let flip = |p: Pos2| Pos2::new(ART - p.x, ART - p.y);

    let outer = rounded(&[
        Pos2::new(0.0, 0.0),
        Pos2::new(LEG, 0.0),
        Pos2::new(0.0, LEG),
    ]);
    let inner = [
        Pos2::new(HOLE_MIN, HOLE_MIN),
        Pos2::new(HOLE_MAX, HOLE_MIN),
        Pos2::new(HOLE_MIN, HOLE_MAX),
    ];
    for (colour, mirrored) in [(GREEN, false), (GREY, true)] {
        let map = |p: &Pos2| at(if mirrored { flip(*p) } else { *p });
        let body: Vec<Pos2> = outer.iter().map(map).collect();
        painter.add(Shape::convex_polygon(body, colour, egui::Stroke::NONE));
        let cut: Vec<Pos2> = inner.iter().map(map).collect();
        painter.add(Shape::convex_polygon(cut, hole, egui::Stroke::NONE));
    }
}

/// A convex polygon with every corner rounded by `RADIUS`.
fn rounded(corners: &[Pos2]) -> Vec<Pos2> {
    let n = corners.len();
    let mut out = Vec::new();
    for i in 0..n {
        let (prev, here, next) = (corners[(i + n - 1) % n], corners[i], corners[(i + 1) % n]);
        let to_prev = (prev - here).normalized();
        let to_next = (next - here).normalized();
        let half = to_prev.dot(to_next).clamp(-1.0, 1.0).acos() / 2.0;
        let tangent = RADIUS / half.tan();
        let centre = here + (to_prev + to_next).normalized() * (RADIUS / half.sin());
        let start = here + to_prev * tangent;
        let end = here + to_next * tangent;
        let a0 = (start - centre).angle();
        let mut sweep = (end - centre).angle() - a0;
        while sweep > std::f32::consts::PI {
            sweep -= std::f32::consts::TAU;
        }
        while sweep < -std::f32::consts::PI {
            sweep += std::f32::consts::TAU;
        }
        for s in 0..=ARC_STEPS {
            let a = a0 + sweep * s as f32 / ARC_STEPS as f32;
            out.push(centre + egui::vec2(a.cos(), a.sin()) * RADIUS);
        }
    }
    out
}
