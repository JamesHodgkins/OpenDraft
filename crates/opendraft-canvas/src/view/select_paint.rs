//! Drawing a selection under way: a box (dragged, or an implied window to the
//! pointer) and a lasso, blue for Window and green and dashed for Crossing
//! and Fence.

use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind};
use lyon_tessellation::math::point;
use lyon_tessellation::path::Path;
use lyon_tessellation::{BuffersBuilder, FillOptions, FillTessellator, FillVertex, VertexBuffers};

use super::select_gesture::{LassoKind, Pending};
use super::types::CanvasStyle;
use super::View;

/// How strongly a selection's area is tinted.
const TINT: f32 = 0.15;

impl View {
    /// The box being dragged, or the implied window or lasso under way.
    pub(super) fn paint_selection(
        &self,
        painter: &Painter,
        rect: Rect,
        style: &CanvasStyle,
        pointer: Option<Pos2>,
    ) {
        if let Some((start, end)) = self.drag {
            paint_band(painter, style, start, end);
        }
        match &self.pending {
            Some(Pending::Window(corner)) => {
                if let Some(end) = pointer {
                    let start = self.camera.world_to_screen(*corner, rect);
                    paint_band(painter, style, start, end);
                }
            }
            Some(Pending::Lasso { path, kind }) => {
                let screen: Vec<Pos2> = path
                    .iter()
                    .map(|p| self.camera.world_to_screen(*p, rect))
                    .collect();
                paint_lasso(painter, style, &screen, kind.unwrap_or(LassoKind::Window));
            }
            None => {}
        }
    }
}

/// A box between two screen corners: Window to the right, Crossing to the
/// left.
fn paint_band(painter: &Painter, style: &CanvasStyle, start: Pos2, end: Pos2) {
    let band = Rect::from_two_pos(start, end);
    let crossing = end.x < start.x;
    let color = if crossing {
        style.crossing
    } else {
        style.window
    };
    painter.rect_filled(band, 0.0, color.gamma_multiply(TINT));
    if crossing {
        let outline = [
            band.left_top(),
            band.right_top(),
            band.right_bottom(),
            band.left_bottom(),
            band.left_top(),
        ];
        dashed(painter, &outline, color);
    } else {
        painter.rect_stroke(band, 0.0, Stroke::new(1.0_f32, color), StrokeKind::Inside);
    }
}

/// A lasso along its screen path: a tinted, closed outline for Window and
/// Crossing, an open line for Fence.
fn paint_lasso(painter: &Painter, style: &CanvasStyle, path: &[Pos2], kind: LassoKind) {
    if path.len() < 2 {
        return;
    }
    let color = match kind {
        LassoKind::Window => style.window,
        _ => style.crossing,
    };
    if kind == LassoKind::Fence {
        dashed(painter, path, color);
        return;
    }
    if let Some(fill) = fill_mesh(path, color.gamma_multiply(TINT)) {
        painter.add(Shape::mesh(fill));
    }
    let mut outline = path.to_vec();
    outline.extend(path.first().copied());
    if kind == LassoKind::Crossing {
        dashed(painter, &outline, color);
    } else {
        painter.add(Shape::line(outline, Stroke::new(1.0_f32, color)));
    }
}

fn dashed(painter: &Painter, points: &[Pos2], color: Color32) {
    painter.extend(Shape::dashed_line(
        points,
        Stroke::new(1.0_f32, color),
        6.0,
        4.0,
    ));
}

/// The inside of a closed screen path, which may cross itself, as triangles.
fn fill_mesh(path: &[Pos2], color: Color32) -> Option<Mesh> {
    let mut builder = Path::builder();
    let mut points = path.iter().map(|p| point(p.x, p.y));
    builder.begin(points.next()?);
    for p in points {
        builder.line_to(p);
    }
    builder.end(true);
    let mut buffers: VertexBuffers<Pos2, u32> = VertexBuffers::new();
    FillTessellator::new()
        .tessellate_path(
            &builder.build(),
            &FillOptions::default(),
            &mut BuffersBuilder::new(&mut buffers, |v: FillVertex| {
                let p = v.position();
                Pos2::new(p.x, p.y)
            }),
        )
        .ok()?;
    Some(Mesh {
        indices: buffers.indices,
        vertices: buffers
            .vertices
            .into_iter()
            .map(|pos| Vertex {
                pos,
                uv: WHITE_UV,
                color,
            })
            .collect(),
        ..Mesh::default()
    })
}
