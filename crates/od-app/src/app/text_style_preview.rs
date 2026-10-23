//! The preview in the Text Style dialog: "AaBb123" in the selected style,
//! black on white, as AutoCAD shows it.

use eframe::egui::{self, Color32, Sense};
use od_canvas::{paint_text, Camera};
use od_core::{MText, Point, TextStyle};
use od_ui::Theme;

use crate::text_style_draft::Draft;

const LIST_WIDTH: f32 = 190.0;
const PREVIEW_HEIGHT: f32 = 84.0;
const PREVIEW_TEXT: &str = "AaBb123";

/// The selected style's sample text.
pub(super) fn preview(ui: &mut egui::Ui, theme: &Theme, draft: &Draft) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(LIST_WIDTH, PREVIEW_HEIGHT), Sense::hover());
    let painter = ui.painter().with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, Color32::WHITE);
    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0_f32, theme.colors.border),
        egui::StrokeKind::Inside,
    );
    let Some(found) = draft.styles.iter().find(|s| s.id == draft.selected) else {
        return;
    };
    // A half-typed value must not stop the preview.
    let style = TextStyle {
        width_factor: found.width_factor.clamp(0.01, 100.0),
        oblique_radians: found.oblique_radians.clamp(-1.48, 1.48),
        ..found.clone()
    };
    let text = MText::new(Point::new(0.0, 0.0), 1.0, PREVIEW_TEXT, style.id);
    let layout = text.layout(&style);
    if layout.frame_width <= 0.0 || layout.frame_height <= 0.0 {
        return;
    }
    let (width, height) = (
        f64::from(rect.width()) - 16.0,
        f64::from(rect.height()) - 16.0,
    );
    let zoom = (width / layout.frame_width).min(height / layout.frame_height * 0.8);
    let middle = text.world_point(
        &layout,
        layout.frame_width / 2.0,
        -layout.frame_height / 2.0,
    );
    let camera = Camera {
        center: middle,
        zoom,
    };
    paint_text(&painter, &camera, rect, &text, &layout, &|_| Color32::BLACK);
}
