//! The Dimension Style dialog's preview: the sample part and its dimensions
//! (`dim_style_draft::sample`) drawn black on white, the way AutoCAD shows it.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Shape, Stroke};
use od_canvas::{paint_text, Camera};
use od_core::{arc, bulge, Bounds, Document, Entity, Point, TextStyle};
use od_ui::Theme;

/// What is the part rather than a dimension: its outline and the hole.
const PART_PIECES: usize = 2;
const MARGIN_PX: f64 = 14.0;
const PART: Color32 = Color32::from_rgb(190, 140, 20);

pub(super) fn show(
    ui: &mut egui::Ui,
    theme: &Theme,
    size: egui::Vec2,
    entities: &[Entity],
    document: &Document,
) {
    show_with(ui, theme, size, entities, document, PART_PIECES);
}

/// The preview with nothing drawn as a part: every piece black (the
/// multileader dialogs).
pub(super) fn show_plain(
    ui: &mut egui::Ui,
    theme: &Theme,
    size: egui::Vec2,
    entities: &[Entity],
    document: &Document,
) {
    show_with(ui, theme, size, entities, document, 0);
}

/// The first `part_pieces` of `entities` are the part, in its own colour.
fn show_with(
    ui: &mut egui::Ui,
    theme: &Theme,
    size: egui::Vec2,
    entities: &[Entity],
    document: &Document,
    part_pieces: usize,
) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, Color32::WHITE);
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0_f32, theme.colors.border),
        egui::StrokeKind::Inside,
    );
    let Some(camera) = fit(entities, rect) else {
        return;
    };
    for (i, entity) in entities.iter().enumerate() {
        let colour = if i < part_pieces {
            PART
        } else {
            Color32::BLACK
        };
        draw(&painter, &camera, rect, entity, colour, document);
    }
}

/// A camera that frames everything with a margin.
fn fit(entities: &[Entity], rect: Rect) -> Option<Camera> {
    let bounds = entities.iter().map(Entity::bounds).reduce(Bounds::union)?;
    let (w, h) = (bounds.max.x - bounds.min.x, bounds.max.y - bounds.min.y);
    if !(w.is_finite() && h.is_finite()) || w <= 0.0 || h <= 0.0 {
        return None;
    }
    let zoom = ((f64::from(rect.width()) - 2.0 * MARGIN_PX) / w)
        .min((f64::from(rect.height()) - 2.0 * MARGIN_PX) / h);
    Some(Camera {
        center: Point::new(
            (bounds.min.x + bounds.max.x) / 2.0,
            (bounds.min.y + bounds.max.y) / 2.0,
        ),
        zoom,
    })
}

fn draw(
    painter: &egui::Painter,
    camera: &Camera,
    rect: Rect,
    entity: &Entity,
    colour: Color32,
    document: &Document,
) {
    let to = |p: Point| -> Pos2 { camera.world_to_screen(p, rect) };
    let stroke = Stroke::new(1.0_f32, colour);
    let tolerance = 0.5 / camera.zoom;
    match entity {
        Entity::Line { start, end } => {
            painter.line_segment([to(*start), to(*end)], stroke);
        }
        Entity::Circle { center, radius } => {
            painter.circle_stroke(to(*center), (radius * camera.zoom) as f32, stroke);
        }
        Entity::Arc {
            center,
            radius,
            start,
            end,
        } => {
            let path = arc::flatten(*center, *radius, *start, *end, tolerance);
            painter.add(Shape::line(path.into_iter().map(to).collect(), stroke));
        }
        Entity::Polyline {
            points,
            closed,
            bulges,
            widths,
        } => {
            // A tapering wide segment is an arrowhead: a filled triangle.
            if let (2, Some(&(wide, _))) = (points.len(), widths.first()) {
                if wide > 0.0 {
                    painter.add(Shape::convex_polygon(
                        triangle(points[0], points[1], wide).map(to).to_vec(),
                        colour,
                        Stroke::NONE,
                    ));
                    return;
                }
            }
            let mut path = bulge::flatten_path(points, bulges, *closed, tolerance);
            if *closed && path.len() > 1 {
                path.push(path[0]);
            }
            painter.add(Shape::line(path.into_iter().map(to).collect(), stroke));
        }
        Entity::Rectangle { min, max } => {
            let corners = [
                *min,
                Point::new(max.x, min.y),
                *max,
                Point::new(min.x, max.y),
                *min,
            ];
            painter.add(Shape::line(corners.map(to).to_vec(), stroke));
        }
        Entity::MText(text) => {
            let stock;
            let style = match document.text_style(text.style) {
                Some(s) => s,
                None => {
                    stock = TextStyle::standard();
                    &stock
                }
            };
            let layout = text.layout(style);
            paint_text(painter, camera, rect, text, &layout, &|_| colour);
        }
        _ => {}
    }
}

/// The arrowhead from its `tail` (of width `base`) to its `tip`.
fn triangle(tail: Point, tip: Point, base: f64) -> [Point; 3] {
    let (dx, dy) = (tip.x - tail.x, tip.y - tail.y);
    let len = dx.hypot(dy).max(1e-12);
    let (nx, ny) = (-dy / len * base / 2.0, dx / len * base / 2.0);
    [
        Point::new(tail.x + nx, tail.y + ny),
        tip,
        Point::new(tail.x - nx, tail.y - ny),
    ]
}
