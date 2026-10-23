//! Painting a block reference: each piece in its own colour, after the
//! layer-0 inheritance the document applies (`Document::insert_pieces`).

use super::*;

impl View {
    /// Paint the pieces of a block reference that is neither selected nor
    /// hovered (those are drawn whole in the highlight colour). Pieces on
    /// layers frozen in this viewport are left out.
    pub(super) fn paint_insert_pieces(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        style: &CanvasStyle,
        document: &Document,
        pieces: &[od_core::Object],
        frozen: &[LayerId],
    ) {
        for piece in pieces.iter().filter(|p| !frozen.contains(&p.attrs.layer)) {
            let line = match document.color_of(piece) {
                Color::Foreground => style.entity,
                Color::Rgb(r, g, b) => egui::Color32::from_rgb(r, g, b),
            };
            match &piece.entity {
                Entity::MText(text) => {
                    self.paint_mtext(painter, rect, style, document, text, line, false, false);
                }
                entity => self.stroke_entity(painter, rect, entity, Stroke::new(1.5_f32, line)),
            }
        }
    }
}
