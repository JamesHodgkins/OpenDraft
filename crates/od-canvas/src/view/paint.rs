//! Painting the viewport: entities and the overlays above them.

use super::*;
use crate::grips::gripped;

/// Which grips are lit.
pub(super) struct GripMarks<'a> {
    /// The grip under the pointer.
    pub hovered: Option<GripHit>,
    /// Grips picked with Shift, waiting to be used.
    pub warm: &'a [GripHit],
}

/// A grip picked with Shift, waiting to be used.
/// Half the length of the cross that marks a hovered circle or arc's centre.
const CENTRE_HINT_ARM: f32 = 5.0;

const WARM_GRIP: egui::Color32 = egui::Color32::from_rgb(230, 80, 80);

impl View {
    /// Rubber band for a box selection drag.
    pub(super) fn paint_selection_box(&self, painter: &egui::Painter, style: &CanvasStyle) {
        let Some((start, end)) = self.drag else {
            return;
        };
        let band = Rect::from_two_pos(start, end);
        let crossing = end.x < start.x;
        let color = if crossing {
            style.crossing
        } else {
            style.window
        };
        painter.rect_filled(band, 0.0, color.gamma_multiply(0.15));
        if crossing {
            painter.extend(egui::Shape::dashed_line(
                &[
                    band.left_top(),
                    band.right_top(),
                    band.right_bottom(),
                    band.left_bottom(),
                    band.left_top(),
                ],
                Stroke::new(1.0_f32, color),
                6.0,
                4.0,
            ));
        } else {
            painter.rect_stroke(band, 0.0, Stroke::new(1.0_f32, color), StrokeKind::Inside);
        }
    }

    /// A command's ghost at the pointer, with a rubber band from its base.
    pub(super) fn paint_prompt_preview(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        style: &CanvasStyle,
        prompt: &PointPrompt,
        to: Point,
    ) {
        if let Some(ghost) = &prompt.ghost {
            let region_color = match prompt.ghost_style {
                GhostStyle::Normal => None,
                GhostStyle::Window => Some(style.window),
                GhostStyle::Crossing => Some(style.crossing),
            };
            let stroke = Stroke::new(1.5_f32, style.selected.gamma_multiply(0.8));
            for entity in ghost(to) {
                match (&entity, region_color) {
                    (Entity::Polyline { points, closed, .. }, Some(color)) => {
                        self.paint_region_path(painter, rect, points, *closed, color, prompt);
                    }
                    _ => self.stroke_entity(painter, rect, &entity, stroke),
                }
            }
        }
        if let Some(marker) = &prompt.marker {
            let stroke = Stroke::new(1.5_f32, style.selected);
            for entity in marker(1.0 / self.camera.zoom) {
                self.stroke_entity(painter, rect, &entity, stroke);
            }
        }
        if let Some(base) = prompt.base {
            let (from, to) = (
                self.camera.world_to_screen(base, rect),
                self.camera.world_to_screen(to, rect),
            );
            painter.line_segment([from, to], Stroke::new(1.0_f32, style.snap));
            painter.circle_filled(from, 3.0, style.snap);
        }
    }

    /// A selection region being drawn: solid for Window, dashed for
    /// Crossing and Fence, with a dot at each vertex.
    pub(super) fn paint_region_path(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        points: &[Point],
        closed: bool,
        color: Color32,
        prompt: &PointPrompt,
    ) {
        let mut path: Vec<Pos2> = points
            .iter()
            .map(|p| self.camera.world_to_screen(*p, rect))
            .collect();
        if closed && path.len() > 2 {
            path.push(path[0]);
        }
        let stroke = Stroke::new(1.0_f32, color);
        if prompt.ghost_style == GhostStyle::Crossing {
            painter.extend(egui::Shape::dashed_line(&path, stroke, 6.0, 4.0));
        } else {
            painter.add(egui::Shape::line(path.clone(), stroke));
        }
        for v in &path {
            painter.circle_filled(*v, 2.5, color);
        }
    }

    pub(super) fn stroke_entity(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        entity: &Entity,
        stroke: Stroke,
    ) {
        let to_screen = |p: Point| -> Pos2 { self.camera.world_to_screen(p, rect) };
        match entity {
            // A preview of a hatch is its fill in the preview's colour.
            Entity::Hatch(h) => self.paint_hatch(painter, rect, None, h, stroke.color, None),
            Entity::Dimension(d) => {
                for piece in d.picture() {
                    self.stroke_entity(painter, rect, &piece, stroke);
                }
            }
            Entity::Insert(i) => {
                for piece in i.picture() {
                    self.stroke_entity(painter, rect, &piece, stroke);
                }
            }
            Entity::MLeader(m) => {
                for piece in m.picture() {
                    self.stroke_entity(painter, rect, &piece, stroke);
                }
            }
            Entity::Leader(l) => {
                for piece in l.picture() {
                    self.stroke_entity(painter, rect, &piece, stroke);
                }
            }
            // A preview of text (a ghost) is the glyphs in the stock style, in
            // the preview's colour.
            Entity::MText(m) => {
                let layout = m.layout(&od_core::TextStyle::standard());
                self.fill_text(painter, rect, m, &layout, &|_| stroke.color);
            }
            Entity::Point { at } => self.stroke_point(painter, rect, *at, stroke),
            Entity::Xline { .. } | Entity::Ray { .. } => {
                if let Some((a, b)) = self.visible_part(rect, entity) {
                    painter.line_segment([to_screen(a), to_screen(b)], stroke);
                }
            }
            Entity::Line { start, end } => {
                painter.line_segment([to_screen(*start), to_screen(*end)], stroke);
            }
            Entity::Circle { center, radius } => {
                let r = (radius.abs() * self.camera.zoom) as f32;
                painter.circle_stroke(to_screen(*center), r, stroke);
            }
            Entity::Arc {
                center,
                radius,
                start,
                end,
            } => {
                // Half a pixel of sag is invisible, so that is the tolerance.
                let tolerance = 0.5 / self.camera.zoom;
                let path: Vec<Pos2> = arc::flatten(*center, *radius, *start, *end, tolerance)
                    .into_iter()
                    .map(to_screen)
                    .collect();
                painter.add(egui::Shape::line(path, stroke));
            }
            Entity::Ellipse(e) => {
                // Half a pixel of sag is invisible, so that is the tolerance.
                let mut path: Vec<Pos2> = e
                    .outline(0.5 / self.camera.zoom)
                    .into_iter()
                    .map(to_screen)
                    .collect();
                painter.add(if e.is_full() {
                    path.pop(); // the last point repeats the first
                    egui::Shape::closed_line(path, stroke)
                } else {
                    egui::Shape::line(path, stroke)
                });
            }
            Entity::Spline(s) => {
                let mut path: Vec<Pos2> = s
                    .outline(0.5 / self.camera.zoom)
                    .into_iter()
                    .map(to_screen)
                    .collect();
                painter.add(if s.closed && path.len() > 2 {
                    path.pop(); // the last point repeats the first
                    egui::Shape::closed_line(path, stroke)
                } else {
                    egui::Shape::line(path, stroke)
                });
            }
            Entity::Rectangle { min, max } => {
                painter.rect_stroke(
                    Rect::from_two_pos(to_screen(*min), to_screen(*max)),
                    0.0,
                    stroke,
                    StrokeKind::Middle,
                );
            }
            Entity::Polyline {
                points,
                closed,
                bulges,
                widths,
            } => {
                // Half a pixel of sag is invisible, so that is the tolerance.
                let tolerance = 0.5 / self.camera.zoom;
                let flat = bulge::flatten_path(points, bulges, *closed, tolerance);
                let path: Vec<Pos2> = flat.iter().map(|p| to_screen(*p)).collect();
                painter.add(if *closed {
                    egui::Shape::closed_line(path, stroke)
                } else {
                    egui::Shape::line(path, stroke)
                });
                // Wide segments are filled in the stroke's colour.
                let wide = width::triangles(points, bulges, widths, *closed, tolerance);
                if !wide.is_empty() {
                    let mut mesh = egui::Mesh::default();
                    for triangle in wide {
                        let base = mesh.vertices.len() as u32;
                        for corner in triangle {
                            mesh.colored_vertex(to_screen(corner), stroke.color);
                        }
                        mesh.add_triangle(base, base + 1, base + 2);
                    }
                    painter.add(egui::Shape::mesh(mesh));
                }
            }
        }
    }

    /// Handles on the selected objects: filled squares, the one under the
    /// pointer lit.
    pub(super) fn paint_grips(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        style: &CanvasStyle,
        document: &Document,
        selection: &BTreeSet<EntityId>,
        marks: GripMarks<'_>,
    ) {
        let half = self.sizes.grip as f32 / 2.0;
        for object in gripped(document, selection) {
            for (index, grip) in object.entity.grips().iter().enumerate() {
                let at = self.camera.world_to_screen(grip.at, rect);
                if !rect.expand(half).contains(at) {
                    continue;
                }
                let this = GripHit {
                    id: object.id,
                    index,
                };
                let fill = if marks.hovered == Some(this) {
                    style.snap
                } else if marks.warm.contains(&this) {
                    WARM_GRIP
                } else {
                    style.selected
                };
                let square = Rect::from_center_size(at, egui::vec2(2.0 * half, 2.0 * half));
                painter.rect_filled(square, 0.0, fill);
                painter.rect_stroke(
                    square,
                    0.0,
                    Stroke::new(1.0_f32, style.background),
                    StrokeKind::Middle,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn paint_entities(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        style: &CanvasStyle,
        document: &Document,
        space: Option<LayoutId>,
        frozen: &[LayerId],
        selection: &BTreeSet<EntityId>,
        hover: Option<EntityId>,
    ) {
        let visible = self.camera.visible_world(rect);
        for object in document.visible_in_rect(space, frozen, visible) {
            let (id, entity) = (&object.id, &object.entity);
            let line = match document.color_of(object) {
                Color::Foreground => style.entity,
                Color::Rgb(r, g, b) => egui::Color32::from_rgb(r, g, b),
            };
            let bounds = object.bounds();
            // Entities smaller than a pixel add nothing but triangles when
            // zoomed far out. Selected ones are always drawn.
            let on_screen =
                (bounds.max.x - bounds.min.x).max(bounds.max.y - bounds.min.y) * self.camera.zoom;
            // A point has no size of its own: its mode gives it one.
            let sized = !matches!(entity, Entity::Point { .. });
            if sized && on_screen < MIN_DRAW_PX && !selection.contains(id) {
                continue;
            }
            if let Entity::MText(text) = entity {
                let (selected, hovered) = (selection.contains(id), hover == Some(*id));
                self.paint_mtext(
                    painter, rect, style, document, text, line, selected, hovered,
                );
                continue;
            }
            if let Entity::Hatch(h) = entity {
                let highlight = if selection.contains(id) {
                    Some(hatch_paint::Highlight {
                        glow: style.selected_glow,
                        edge: style.selected,
                        width: 2.0,
                    })
                } else if hover == Some(*id) {
                    Some(hatch_paint::Highlight {
                        glow: style.hover,
                        edge: line,
                        width: 1.5,
                    })
                } else {
                    None
                };
                self.paint_hatch(painter, rect, Some(*id), h, line, highlight);
                continue;
            }
            let plain = !selection.contains(id) && hover != Some(*id);
            if let Some(pieces) = plain.then(|| document.insert_pieces(object)).flatten() {
                self.paint_insert_pieces(painter, rect, style, document, &pieces, frozen);
                continue;
            }
            // A linetype cuts the line into dashes; the glow follows them.
            let draw = |stroke: Stroke| self.stroke_object(painter, rect, document, object, stroke);
            // Selected and hovered entities get a wide translucent overlay
            // under a thicker, brighter line.
            if selection.contains(id) {
                draw(Stroke::new(HIGHLIGHT_WIDTH, style.selected_glow));
                draw(Stroke::new(2.0_f32, style.selected));
            } else if hover == Some(*id) {
                draw(Stroke::new(HIGHLIGHT_WIDTH, style.hover));
                draw(Stroke::new(1.5_f32, line));
                if let Entity::Circle { center, .. } | Entity::Arc { center, .. } = entity {
                    self.centre_hint(painter, rect, *center, line);
                }
            } else {
                draw(Stroke::new(1.5_f32, line));
            }
        }
    }

    /// A small cross at the centre of a hovered circle or arc, so it can be
    /// found (and snapped to) without hunting for it.
    fn centre_hint(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        center: Point,
        color: egui::Color32,
    ) {
        let at = self.camera.world_to_screen(center, rect);
        let arm = CENTRE_HINT_ARM;
        let stroke = Stroke::new(1.0_f32, color);
        painter.line_segment(
            [at - egui::vec2(arm, 0.0), at + egui::vec2(arm, 0.0)],
            stroke,
        );
        painter.line_segment(
            [at - egui::vec2(0.0, arm), at + egui::vec2(0.0, arm)],
            stroke,
        );
    }
}
