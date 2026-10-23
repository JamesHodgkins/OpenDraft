//! Small text as a picture.
//!
//! Text only a few pixels tall breaks up when filled as triangles, so a whole
//! text object is instead rasterised into one small texture, with its edges
//! anti-aliased by exact horizontal and eight-fold vertical coverage, and drawn
//! as a quad. Pictures are kept by what they show, so a repaint only places a
//! quad.

use std::cell::RefCell;
use std::collections::HashMap;

use egui::epaint::Vertex;
use egui::{Color32, ColorImage, Mesh, Painter, Pos2, TextureHandle, TextureOptions};

/// Sub-rows of coverage per pixel row.
const ROWS: usize = 8;
/// The largest picture, in pixels a side; bigger text is filled instead.
pub const MAX_SIDE_PX: f64 = 2048.0;
/// Pictures kept before the ones not drawn lately are let go.
const CACHE_LIMIT: usize = 512;

/// One filled triangle in layout units.
pub struct Tri {
    pub corners: [(f64, f64); 3],
    pub tint: Color32,
}

/// A rasterised text: the pixels and the layout-unit box they cover.
struct Picture {
    size: [usize; 2],
    pixels: Vec<Color32>,
    left: f64,
    top: f64,
    width: f64,
    height: f64,
}

struct Kept {
    texture: TextureHandle,
    left: f64,
    top: f64,
    width: f64,
    height: f64,
    drawn: u64,
}

thread_local! {
    static PICTURES: RefCell<HashMap<u64, Kept>> = RefCell::new(HashMap::new());
}

/// Rasterise `tris` at `scale` pixels per layout unit, with a pixel of margin.
fn rasterise(tris: &[Tri], scale: f64) -> Option<Picture> {
    let points = tris.iter().flat_map(|t| t.corners);
    let (mut min_x, mut max_x) = (f64::MAX, f64::MIN);
    let (mut min_y, mut max_y) = (f64::MAX, f64::MIN);
    for (x, y) in points {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    if min_x > max_x || scale <= 0.0 {
        return None;
    }
    let left = min_x - 1.0 / scale;
    let top = max_y + 1.0 / scale;
    let w = ((max_x - left) * scale).ceil() as usize + 1;
    let h = ((top - min_y) * scale).ceil() as usize + 1;
    if w as f64 > MAX_SIDE_PX || h as f64 > MAX_SIDE_PX {
        return None;
    }
    // Premultiplied colour and coverage, summed over everything drawn.
    let mut sum = vec![[0.0_f32; 4]; w * h];
    for tri in tris {
        let px = tri
            .corners
            .map(|(x, y)| ((x - left) * scale, (top - y) * scale));
        fill_triangle(&mut sum, w, h, px, tri.tint);
    }
    let pixels = sum
        .iter()
        .map(|&[r, g, b, a]| {
            let k = if a > 1.0 { 1.0 / a } else { 1.0 };
            let byte = |v: f32| (v * k * 255.0).round().clamp(0.0, 255.0) as u8;
            Color32::from_rgba_premultiplied(byte(r), byte(g), byte(b), byte(a.min(1.0)))
        })
        .collect();
    Some(Picture {
        size: [w, h],
        pixels,
        left,
        top,
        width: w as f64 / scale,
        height: h as f64 / scale,
    })
}

/// Add `tint` weighted by how much of each pixel the triangle `p` covers.
fn fill_triangle(sum: &mut [[f32; 4]], w: usize, h: usize, p: [(f64, f64); 3], tint: Color32) {
    let lo = p.iter().map(|q| q.1).fold(f64::MAX, f64::min);
    let hi = p.iter().map(|q| q.1).fold(f64::MIN, f64::max);
    let first = ((lo * ROWS as f64).floor().max(0.0)) as usize;
    let last = ((hi * ROWS as f64).ceil().max(0.0) as usize).min(h * ROWS);
    let [r, g, b, a] = tint.to_array().map(|c| f32::from(c) / 255.0);
    let colour = [r * a, g * a, b * a, a];
    for sub in first..last {
        let y = (sub as f64 + 0.5) / ROWS as f64;
        let Some((x0, x1)) = span(&p, y) else {
            continue;
        };
        let row = sub / ROWS;
        let x0 = x0.max(0.0);
        let x1 = x1.min(w as f64);
        let mut col = x0.floor() as usize;
        while (col as f64) < x1 && col < w {
            let over = x1.min(col as f64 + 1.0) - x0.max(col as f64);
            let weight = (over / ROWS as f64) as f32;
            if let Some(cell) = sum.get_mut(row * w + col) {
                for (c, v) in cell.iter_mut().zip(colour) {
                    *c += v * weight;
                }
            }
            col += 1;
        }
    }
}

/// Where the horizontal line at `y` crosses the triangle `p`.
fn span(p: &[(f64, f64); 3], y: f64) -> Option<(f64, f64)> {
    let mut lo = f64::MAX;
    let mut hi = f64::MIN;
    for i in 0..3 {
        let (a, b) = (p[i], p[(i + 1) % 3]);
        if (a.1 <= y) != (b.1 <= y) {
            let x = a.0 + (y - a.1) / (b.1 - a.1) * (b.0 - a.0);
            lo = lo.min(x);
            hi = hi.max(x);
        }
    }
    (lo < hi).then_some((lo, hi))
}

/// Draw the picture kept under `key`, making it from `tris` if there is none,
/// as a quad whose corners `to_screen` places from layout units.
pub fn paint_picture(
    painter: &Painter,
    key: u64,
    scale: f64,
    tris: impl FnOnce() -> Vec<Tri>,
    to_screen: &dyn Fn(f64, f64) -> Pos2,
) {
    let ctx = painter.ctx();
    let now = ctx.cumulative_pass_nr();
    let found = PICTURES.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(k) = cache.get_mut(&key) {
            k.drawn = now;
            return Some((k.texture.id(), k.left, k.top, k.width, k.height));
        }
        let picture = rasterise(&tris(), scale)?;
        let image = ColorImage {
            size: picture.size,
            pixels: picture.pixels,
        };
        let texture = ctx.load_texture("text", image, TextureOptions::LINEAR);
        if cache.len() >= CACHE_LIMIT {
            cache.retain(|_, k| k.drawn + 1 >= now);
            if cache.len() >= CACHE_LIMIT {
                cache.clear();
            }
        }
        let id = texture.id();
        let out = (id, picture.left, picture.top, picture.width, picture.height);
        cache.insert(
            key,
            Kept {
                texture,
                left: picture.left,
                top: picture.top,
                width: picture.width,
                height: picture.height,
                drawn: now,
            },
        );
        Some(out)
    });
    let Some((id, left, top, width, height)) = found else {
        return;
    };
    let corners = [
        (left, top, [0.0, 0.0]),
        (left + width, top, [1.0, 0.0]),
        (left + width, top - height, [1.0, 1.0]),
        (left, top - height, [0.0, 1.0]),
    ];
    let mut mesh = Mesh::with_texture(id);
    for (x, y, uv) in corners {
        mesh.vertices.push(Vertex {
            pos: to_screen(x, y),
            uv: Pos2::new(uv[0], uv[1]),
            color: Color32::WHITE,
        });
    }
    mesh.indices.extend([0, 1, 2, 0, 2, 3]);
    painter.add(egui::Shape::mesh(mesh));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Tri> {
        let tint = Color32::WHITE;
        vec![
            Tri {
                corners: [(x0, y0), (x1, y0), (x1, y1)],
                tint,
            },
            Tri {
                corners: [(x0, y0), (x1, y1), (x0, y1)],
                tint,
            },
        ]
    }

    #[test]
    fn a_whole_pixel_square_is_opaque_and_a_half_pixel_is_half() {
        let picture = rasterise(&square(0.0, 0.0, 4.0, 4.0), 1.0).unwrap();
        let [w, _] = picture.size;
        // One pixel of margin, so the square starts at pixel (1, 1).
        assert_eq!(picture.pixels[2 * w + 2].a(), 255);
        let half = rasterise(&square(0.0, 0.0, 3.5, 4.0), 1.0).unwrap();
        let edge = half.pixels[2 * half.size[0] + 4].a();
        assert!((120..=135).contains(&edge), "{edge}");
    }

    #[test]
    fn the_picture_covers_the_shape_and_nothing_empty_is_made() {
        let picture = rasterise(&square(0.0, 0.0, 4.0, 2.0), 2.0).unwrap();
        assert!(picture.left < 0.0 && picture.top > 2.0);
        assert!(picture.width >= 4.0 && picture.height >= 2.0);
        assert!(rasterise(&[], 1.0).is_none());
        assert!(rasterise(&square(0.0, 0.0, 5000.0, 1.0), 1.0).is_none());
    }
}
