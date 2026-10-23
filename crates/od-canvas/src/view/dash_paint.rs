//! Stroking an entity with its linetype pattern.

use od_core::linetype::dash::{dash_path, Piece};
use od_core::linetype::paths::curve_outlines;
use od_core::{Object, StrokeElement};

use super::*;

/// A pattern whose repeat is shorter than this on screen is drawn solid: a
/// repeat of 6 px is a 4 px dash and a 2 px gap, which already reads as a
/// texture, and every dash is a shape to build. The cost of 10,000 dashed
/// objects all in view was 41 ms at a 4 px repeat, 26 ms at 7 px, 23 ms at 8 px
/// and 8 ms solid (`bench_features`). This is how it looks at this zoom,
/// never what the drawing holds or a plot shows.
const MIN_REPEAT_PX: f64 = 6.0;
/// A path that would need more dashes than this is drawn solid.
const MAX_MARKS: f64 = 20_000.0;

/// Whether a pattern whose repeat is `period` drawing units long is drawn
/// with its dashes at this zoom (`pixels per unit`), or as a solid line.
fn repeat_shows(period: f64, zoom: f64) -> bool {
    period.is_finite() && period * zoom >= MIN_REPEAT_PX
}

impl View {
    /// Stroke an object's entity with its linetype when it has a pattern that
    /// shows at this zoom, and as a solid line otherwise.
    pub(super) fn stroke_object(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        document: &Document,
        object: &Object,
        stroke: Stroke,
    ) {
        if let Some(pieces) = object.dimension_picture() {
            for piece in pieces.iter() {
                match piece {
                    // The words take the dimension's own text style.
                    Entity::MText(text) => {
                        let stock;
                        let style = match document.text_style(text.style) {
                            Some(s) => s,
                            None => {
                                stock = od_core::TextStyle::standard();
                                &stock
                            }
                        };
                        let layout = text.layout(style);
                        self.fill_text(painter, rect, text, &layout, &|_| stroke.color);
                    }
                    other => self.stroke_entity(painter, rect, other, stroke),
                }
            }
            return;
        }
        let continuous = object.attrs.continuous_generation;
        let dashed = document
            .dash_pattern(object)
            .is_some_and(|(elements, scale)| {
                let entity = &object.entity;
                self.stroke_dashed(painter, rect, entity, elements, scale, continuous, stroke)
            });
        if !dashed {
            self.stroke_entity(painter, rect, &object.entity, stroke);
        }
    }

    /// Stroke `entity` laid out with `elements` at `scale` (`continuous`: one
    /// pattern along a whole polyline, not one per segment). Returns false,
    /// drawing nothing, when it should be drawn solid instead (not a kind that
    /// takes a linetype, or a pattern too fine or too long to show).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn stroke_dashed(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        entity: &Entity,
        elements: &[StrokeElement],
        scale: f64,
        continuous: bool,
        stroke: Stroke,
    ) -> bool {
        let period: f64 = elements.iter().map(|e| e.length()).sum::<f64>() * scale;
        if !repeat_shows(period, self.camera.zoom) {
            return false;
        }
        // Half a pixel of sag is invisible, so that is the tolerance.
        let Ok(Some(outlines)) = curve_outlines(entity, 0.5 / self.camera.zoom, !continuous) else {
            return false;
        };
        let too_long = outlines.iter().any(|(points, _)| {
            let length: f64 = points
                .windows(2)
                .map(|w| (w[1].x - w[0].x).hypot(w[1].y - w[0].y))
                .sum();
            length / period * elements.len() as f64 > MAX_MARKS
        });
        if too_long {
            return false;
        }
        let to_screen = |p: Point| self.camera.world_to_screen(p, rect);
        for (points, closed) in outlines {
            let Some(pieces) = dash_path(&points, closed, elements, scale) else {
                // Too short to hold a repeat: one unbroken line.
                let path: Vec<Pos2> = points.iter().map(|p| to_screen(*p)).collect();
                painter.add(if closed {
                    egui::Shape::closed_line(path, stroke)
                } else {
                    egui::Shape::line(path, stroke)
                });
                continue;
            };
            for piece in pieces {
                match piece {
                    Piece::Dash(run) => {
                        let path: Vec<Pos2> = run.iter().map(|p| to_screen(*p)).collect();
                        painter.add(egui::Shape::line(path, stroke));
                    }
                    Piece::Dot { at, .. } => {
                        painter.circle_filled(
                            to_screen(at),
                            (stroke.width / 2.0).max(0.75),
                            stroke.color,
                        );
                    }
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_repeat_shows_from_six_pixels_up() {
        // DASHED at LTSCALE 1 in millimetres is 19.05 units.
        assert!(repeat_shows(19.05, 1.0));
        assert!(repeat_shows(19.05, 0.4), "7.6 px");
        assert!(!repeat_shows(19.05, 0.3), "5.7 px");
        assert!(repeat_shows(6.0, 1.0), "exactly six");
        assert!(!repeat_shows(5.999, 1.0));
    }

    #[test]
    fn a_pattern_that_is_not_a_length_never_shows() {
        for period in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(!repeat_shows(period, 1.0));
        }
        assert!(!repeat_shows(0.0, 100.0), "no repeat to draw");
    }
}
