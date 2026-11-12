//! Painting helpers shared by the dynamic-input overlay.
use eframe::egui::{self, Color32, Rect, Stroke, StrokeKind};

pub(super) fn area(
    ctx: &egui::Context,
    id: impl std::hash::Hash,
    rect: Rect,
    content: impl FnOnce(&mut egui::Ui, Rect),
) {
    egui::Area::new(egui::Id::new(("dynamic_area", id)))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .movable(false)
        .constrain(false)
        .fade_in(false)
        .show(ctx, |ui| {
            ui.set_min_size(rect.size());
            ui.set_max_size(rect.size());
            ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
            let local = Rect::from_min_size(ui.min_rect().min, rect.size());
            ui.set_clip_rect(local);
            content(ui, local);
        });
}

pub(super) fn dynamic_filter() -> egui::EventFilter {
    egui::EventFilter {
        tab: true,
        horizontal_arrows: true,
        vertical_arrows: true,
        escape: true,
    }
}

pub(super) fn box_fill(ui: &egui::Ui, rect: Rect, fill: Color32, border: Color32) {
    ui.painter().rect(
        rect,
        0.0,
        fill,
        Stroke::new(1.0_f32, border),
        StrokeKind::Inside,
    );
}
