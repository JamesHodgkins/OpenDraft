//! Painting multiline text: glyph outlines filled as triangles.
//!
//! The text engine gives positioned glyphs; each glyph's outline is cut into
//! triangles once per size bucket and kept, so a repaint only moves vertices.

use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke};
use lyon_tessellation::math::point;
use lyon_tessellation::path::Path;
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, VertexBuffers,
};
use od_core::aci::aci_rgb;
use od_core::text::{
    glyph_outline, od_roman, FontFace, TextColor, TextLayout, MAX_WEIGHT, MIN_WEIGHT,
};
use od_core::{Document, MText, TextStyle};

use super::text_raster::{paint_picture, Tri, MAX_SIDE_PX};
use super::View;
use crate::camera::Camera;
use crate::view::CanvasStyle;

/// Text shorter than this on screen is drawn as a line per text line, fading
/// out with its height so it meets the smallest picture without a jump.
const GREEK_BELOW_PX: f64 = 0.75;
/// How opaque that line is at `GREEK_BELOW_PX`: about what a picture of
/// text that small averages to.
const GREEK_ALPHA: f32 = 0.2;
/// Text shorter than this on screen is drawn as a small picture.
const PICTURE_BELOW_PX: f64 = 12.0;
/// Picture scales are rounded to this many steps per doubling.
const PICTURE_STEPS: f64 = 8.0;
/// The thinnest a stroke-font pen is drawn, in screen pixels.
const MIN_PEN_PX: f64 = 1.1;
/// How far a curve may stray from its true shape on screen.
const TOLERANCE_PX: f64 = 0.25;
/// Glyph shapes kept before the cache is emptied and started again.
const CACHE_LIMIT: usize = 4096;

/// A glyph's triangles, in font units.
struct Triangles {
    vertices: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

/// A glyph shape's identity: face, glyph, pen weight and size bucket.
type GlyphKey = (FontFace, u16, u16, i32);

thread_local! {
    static GLYPHS: RefCell<HashMap<GlyphKey, Rc<Triangles>>> = RefCell::new(HashMap::new());
}

/// The triangles of `glyph` (at pen `weight` for a stroke face), with curves
/// flattened to `2^bucket` font units.
fn triangles(face: FontFace, glyph: u16, weight: u16, bucket: i32) -> Rc<Triangles> {
    GLYPHS.with(|cache| {
        if let Some(found) = cache.borrow().get(&(face, glyph, weight, bucket)) {
            return Rc::clone(found);
        }
        let made = Rc::new(tessellate(face, glyph, weight, f64::from(bucket).exp2()));
        let mut cache = cache.borrow_mut();
        if cache.len() >= CACHE_LIMIT {
            cache.clear();
        }
        cache.insert((face, glyph, weight, bucket), Rc::clone(&made));
        made
    })
}

fn tessellate(face: FontFace, glyph: u16, weight: u16, tolerance: f64) -> Triangles {
    let mut builder = Path::builder();
    for contour in glyph_outline(face, glyph, weight, tolerance) {
        let mut points = contour.iter().map(|&(x, y)| point(x as f32, y as f32));
        let Some(first) = points.next() else {
            continue;
        };
        builder.begin(first);
        for p in points {
            builder.line_to(p);
        }
        builder.end(true);
    }
    let mut buffers: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    let options = FillOptions::default().with_fill_rule(FillRule::NonZero);
    let result = FillTessellator::new().tessellate_path(
        &builder.build(),
        &options,
        &mut BuffersBuilder::new(&mut buffers, |v: FillVertex| v.position().to_array()),
    );
    if result.is_err() {
        return Triangles {
            vertices: Vec::new(),
            indices: Vec::new(),
        };
    }
    Triangles {
        vertices: buffers.vertices,
        indices: buffers.indices,
    }
}

/// `weight` widened, for a stroke face, so the pen is never thinner than
/// `MIN_PEN_PX` on screen: hairline strokes break up when zoomed out.
fn legible_weight(face: FontFace, weight: u16, per_unit: f64) -> u16 {
    let Some(font) = face.is_stroke().then(od_roman).flatten() else {
        return weight;
    };
    let fraction = if weight == 0 {
        font.metrics.default_weight
    } else {
        f64::from(weight) / 1000.0
    };
    let cap_px = font.metrics.cap_height * per_unit;
    if cap_px <= 0.0 || fraction * cap_px >= MIN_PEN_PX {
        return weight;
    }
    let widened = (MIN_PEN_PX / cap_px).clamp(MIN_WEIGHT, MAX_WEIGHT);
    // Round up to a step so zooming reuses cached shapes.
    ((widened * 100.0).ceil() * 10.0) as u16
}

/// Fill `layout`'s glyphs and decorations, each in the colour `color` gives
/// for it, through `camera` into `rect`.
pub fn paint_text(
    painter: &Painter,
    camera: &Camera,
    rect: Rect,
    text: &MText,
    layout: &TextLayout,
    color: &dyn Fn(TextColor) -> Color32,
) {
    let to_screen =
        |x: f64, y: f64| -> Pos2 { camera.world_to_screen(text.world_point(layout, x, y), rect) };
    let height_px = text.nominal_height * camera.zoom;
    if height_px < GREEK_BELOW_PX {
        let fade = GREEK_ALPHA * (height_px / GREEK_BELOW_PX) as f32;
        for line in layout.lines.iter().filter(|l| l.width > 0.0) {
            let y = line.baseline + 0.3 * text.nominal_height;
            let stroke = Stroke::new(1.0_f32, color(TextColor::Entity).gamma_multiply(fade));
            painter.line_segment([to_screen(0.0, y), to_screen(line.width, y)], stroke);
        }
        return;
    }
    if height_px < PICTURE_BELOW_PX && paint_small(painter, camera, text, layout, color, &to_screen)
    {
        return;
    }
    // Only lines that reach the screen are built: zoomed in on a long text,
    // that is a few lines, not all of them.
    let view = rect.intersect(painter.clip_rect()).expand(2.0);
    let across = layout.frame_width.max(layout.actual_width);
    let mut mesh = Mesh::default();
    for line in &layout.lines {
        let reach = 2.0 * line.height;
        let (lo, hi) = (line.baseline - reach, line.baseline + reach);
        if !reaches(view, &to_screen, (-reach, across + reach), (lo, hi)) {
            continue;
        }
        for glyph in layout.glyphs.get(line.glyphs.clone()).unwrap_or(&[]) {
            let (bucket, weight) =
                glyph_detail(glyph.face, glyph.weight, camera.zoom * glyph.scale_y);
            let shape = triangles(glyph.face, glyph.id, weight, bucket);
            let base = mesh.vertices.len() as u32;
            let tint = color(glyph.color);
            for v in &shape.vertices {
                let (x, y) = glyph.place(f64::from(v[0]), f64::from(v[1]));
                mesh.colored_vertex(to_screen(x, y), tint);
            }
            mesh.indices.extend(shape.indices.iter().map(|i| base + i));
        }
    }
    for d in &layout.decorations {
        let half = d.thickness / 2.0;
        if !reaches(view, &to_screen, (d.x0, d.x1), (d.y - half, d.y + half)) {
            continue;
        }
        let base = mesh.vertices.len() as u32;
        let tint = color(d.color);
        let corners = [
            (d.x0, d.y - half),
            (d.x1, d.y - half),
            (d.x1, d.y + half),
            (d.x0, d.y + half),
        ];
        for (x, y) in corners {
            mesh.colored_vertex(to_screen(x, y), tint);
        }
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    if !mesh.is_empty() {
        painter.add(Shape::mesh(mesh));
    }
}

/// Whether the layout-space box `xs` by `ys` reaches `view` on the screen.
fn reaches(
    view: Rect,
    to_screen: &dyn Fn(f64, f64) -> Pos2,
    xs: (f64, f64),
    ys: (f64, f64),
) -> bool {
    let corners = [
        to_screen(xs.0, ys.0),
        to_screen(xs.1, ys.0),
        to_screen(xs.1, ys.1),
        to_screen(xs.0, ys.1),
    ];
    Rect::from_points(&corners).intersects(view)
}

/// A glyph's tessellation bucket and pen weight at `per_unit` pixels per font unit.
fn glyph_detail(face: FontFace, weight: u16, per_unit: f64) -> (i32, u16) {
    let bucket = (TOLERANCE_PX / per_unit.max(1e-9))
        .log2()
        .floor()
        .clamp(-4.0, 8.0) as i32;
    (bucket, legible_weight(face, weight, per_unit))
}

/// Everything `layout` draws, as triangles in layout units, for a picture at
/// `scale` pixels per unit.
fn small_triangles(
    layout: &TextLayout,
    color: &dyn Fn(TextColor) -> Color32,
    scale: f64,
) -> Vec<Tri> {
    let mut out = Vec::new();
    for g in &layout.glyphs {
        // A picture shades by coverage, so a thin pen just comes out lighter.
        let (bucket, _) = glyph_detail(g.face, g.weight, scale * g.scale_y);
        let weight = g.weight;
        let shape = triangles(g.face, g.id, weight, bucket);
        let at = |i: u32| {
            let v = shape.vertices.get(i as usize).copied().unwrap_or_default();
            g.place(f64::from(v[0]), f64::from(v[1]))
        };
        let tint = color(g.color);
        for t in shape.indices.as_chunks::<3>().0 {
            out.push(Tri {
                corners: [at(t[0]), at(t[1]), at(t[2])],
                tint,
            });
        }
    }
    for d in &layout.decorations {
        let half = d.thickness / 2.0;
        let [a, b, c, e] = [
            (d.x0, d.y - half),
            (d.x1, d.y - half),
            (d.x1, d.y + half),
            (d.x0, d.y + half),
        ];
        let tint = color(d.color);
        out.push(Tri {
            corners: [a, b, c],
            tint,
        });
        out.push(Tri {
            corners: [a, c, e],
            tint,
        });
    }
    out
}

/// Draw `layout` as a picture kept for its look at this zoom. False when it
/// is too large for one, and is to be filled instead.
fn paint_small(
    painter: &Painter,
    camera: &Camera,
    text: &MText,
    layout: &TextLayout,
    color: &dyn Fn(TextColor) -> Color32,
    to_screen: &dyn Fn(f64, f64) -> Pos2,
) -> bool {
    let step = (camera.zoom.max(1e-9).log2() * PICTURE_STEPS).round();
    let scale = (step / PICTURE_STEPS).exp2();
    if layout.actual_width.max(layout.frame_width) * scale > MAX_SIDE_PX
        || layout.actual_height.max(layout.frame_height) * scale > MAX_SIDE_PX
    {
        return false;
    }
    let mut hasher = DefaultHasher::new();
    (step as i32, text.nominal_height.to_bits()).hash(&mut hasher);
    for g in &layout.glyphs {
        (g.face, g.id, g.weight, color(g.color)).hash(&mut hasher);
        for v in [g.x, g.y, g.scale_x, g.scale_y, g.shear] {
            v.to_bits().hash(&mut hasher);
        }
    }
    for d in &layout.decorations {
        color(d.color).hash(&mut hasher);
        for v in [d.x0, d.x1, d.y, d.thickness] {
            v.to_bits().hash(&mut hasher);
        }
    }
    paint_picture(
        painter,
        hasher.finish(),
        scale,
        || small_triangles(layout, color, scale),
        to_screen,
    );
    true
}

impl View {
    /// Fill `layout`'s glyphs and decorations with this view's camera.
    pub(super) fn fill_text(
        &self,
        painter: &Painter,
        rect: Rect,
        text: &MText,
        layout: &TextLayout,
        color: &dyn Fn(TextColor) -> Color32,
    ) {
        paint_text(painter, &self.camera, rect, text, layout, color);
    }

    /// One text object as the drawing shows it: filled glyphs in their own
    /// colours (the selection colour while selected) and, when selected or
    /// under the pointer, its frame.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn paint_mtext(
        &self,
        painter: &Painter,
        rect: Rect,
        style: &CanvasStyle,
        document: &Document,
        text: &MText,
        line: Color32,
        selected: bool,
        hovered: bool,
    ) {
        let stock;
        let text_style = match document.text_style(text.style) {
            Some(s) => s,
            None => {
                stock = TextStyle::standard();
                &stock
            }
        };
        let layout = text.layout(text_style);
        let resolve = |c: TextColor| match c {
            _ if selected => style.selected,
            TextColor::Entity | TextColor::Aci(7) => line,
            TextColor::Aci(n) => {
                let (r, g, b) = aci_rgb(u8::try_from(n).unwrap_or(7));
                Color32::from_rgb(r, g, b)
            }
            TextColor::Rgb(v) => Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8),
        };
        self.fill_text(painter, rect, text, &layout, &resolve);
        if selected || hovered {
            let corners = [
                (0.0, 0.0),
                (layout.frame_width, 0.0),
                (layout.frame_width, -layout.frame_height),
                (0.0, -layout.frame_height),
            ];
            let path: Vec<Pos2> = corners
                .iter()
                .map(|&(x, y)| {
                    self.camera
                        .world_to_screen(text.world_point(&layout, x, y), rect)
                })
                .collect();
            let frame = if selected {
                style.selected
            } else {
                style.hover
            };
            painter.add(Shape::closed_line(path, Stroke::new(1.0_f32, frame)));
        }
    }
}
