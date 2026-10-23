//! Painting hatches: pattern strokes, solid fills, and the boundary when a
//! hatch is selected, hovered, or cannot be worked out.

use egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke};
use od_core::hatch::{HatchEntity, HatchFill, HatchLimits};
use od_core::{EntityId, Point};

use super::{View, HIGHLIGHT_WIDTH};
use crate::hatch_cache::{Outcome, SolidMesh};

/// Pattern lines closer than this on screen are not drawn one by one: the
/// area is tinted instead.
const DENSE_PX: f64 = 2.5;
/// How far a curve may stray on screen, as for every other entity.
const TOLERANCE_PX: f64 = 0.25;
/// A dot's radius on screen.
const DOT_RADIUS_PX: f32 = 1.0;

/// The extra drawing for a hatch that is selected or under the pointer: its
/// boundary, in a wide translucent line under a thinner one.
#[derive(Clone, Copy)]
pub(super) struct Highlight {
    pub glow: Color32,
    pub edge: Color32,
    pub width: f32,
}

/// The colour a solid or gradient fill is drawn in: a gradient's first stop
/// (other gradients than flat colour are not drawn yet), otherwise the
/// object's own colour.
fn solid_color(hatch: &HatchEntity, own: Color32) -> Color32 {
    match &hatch.fill {
        HatchFill::Gradient(g) => g
            .colors
            .first()
            .map_or(own, |(_, [r, gr, b])| Color32::from_rgb(*r, *gr, *b)),
        _ => own,
    }
}

impl View {
    fn fill_mesh(&self, painter: &Painter, rect: Rect, mesh: &SolidMesh, tint: Color32) {
        let mut out = Mesh::default();
        for v in &mesh.vertices {
            let p = Point::new(
                mesh.origin.x + f64::from(v[0]),
                mesh.origin.y + f64::from(v[1]),
            );
            out.colored_vertex(self.camera.world_to_screen(p, rect), tint);
        }
        out.indices.clone_from(&mesh.indices);
        if !out.is_empty() {
            painter.add(Shape::mesh(out));
        }
    }

    fn outline(&self, painter: &Painter, rect: Rect, contour: &[Point], stroke: Stroke) {
        let path: Vec<Pos2> = contour
            .iter()
            .map(|p| self.camera.world_to_screen(*p, rect))
            .collect();
        painter.add(Shape::closed_line(path, stroke));
    }

    /// Paints `hatch` in `color`. `id` names its cache entry (`None` for a
    /// preview).
    pub(super) fn paint_hatch(
        &self,
        painter: &Painter,
        rect: Rect,
        id: Option<EntityId>,
        hatch: &HatchEntity,
        color: Color32,
        highlight: Option<Highlight>,
    ) {
        let limits = if id.is_some() {
            HatchLimits::default()
        } else {
            HatchLimits::preview()
        };
        let tolerance = TOLERANCE_PX / self.camera.zoom;
        let outcome = self
            .hatch_cache
            .borrow_mut()
            .get(id, hatch, tolerance, &limits);
        let visible = self.camera.visible_world(rect);
        match &*outcome {
            Outcome::Worked(w) => {
                let tint = highlight.map_or(color, |h| h.edge);
                let dense = w.spacing.is_some_and(|s| s * self.camera.zoom < DENSE_PX);
                if hatch.is_solid() {
                    if let Some(mesh) = w.mesh() {
                        self.fill_mesh(painter, rect, mesh, solid_color(hatch, color));
                    }
                } else if dense {
                    if let Some(mesh) = w.mesh() {
                        self.fill_mesh(painter, rect, mesh, tint.gamma_multiply(0.45));
                    }
                } else {
                    let stroke = Stroke::new(1.0_f32, tint);
                    let shapes: Vec<Shape> = w
                        .evaluated
                        .strokes
                        .iter()
                        .filter(|s| {
                            s[0].x.max(s[1].x) >= visible.min.x
                                && s[0].x.min(s[1].x) <= visible.max.x
                                && s[0].y.max(s[1].y) >= visible.min.y
                                && s[0].y.min(s[1].y) <= visible.max.y
                        })
                        .map(|s| {
                            Shape::line_segment(
                                [
                                    self.camera.world_to_screen(s[0], rect),
                                    self.camera.world_to_screen(s[1], rect),
                                ],
                                stroke,
                            )
                        })
                        .collect();
                    painter.extend(shapes);
                    let dots: Vec<Shape> = w
                        .evaluated
                        .dots
                        .iter()
                        .map(|d| {
                            Shape::circle_filled(
                                self.camera.world_to_screen(*d, rect),
                                DOT_RADIUS_PX,
                                tint,
                            )
                        })
                        .collect();
                    painter.extend(dots);
                }
                if let Some(h) = highlight {
                    for contour in &w.evaluated.fill {
                        self.outline(painter, rect, contour, Stroke::new(HIGHLIGHT_WIDTH, h.glow));
                        self.outline(painter, rect, contour, Stroke::new(h.width, h.edge));
                    }
                }
            }
            // Boundary only, with a note: the hatch is intact, it is just
            // not drawn (too dense, or a boundary that does not close).
            Outcome::Failed { error, boundary } => {
                let stroke = Stroke::new(1.0_f32, highlight.map_or(color, |h| h.edge));
                for contour in boundary {
                    self.outline(painter, rect, contour, stroke);
                }
                if let Some(b) = boundary.first().and_then(|c| c.first()) {
                    painter.text(
                        self.camera.world_to_screen(*b, rect),
                        egui::Align2::LEFT_BOTTOM,
                        format!("Hatch not drawn: {error}"),
                        egui::FontId::proportional(11.0),
                        color,
                    );
                }
            }
        }
    }
}
