//! Geometry for the lightweight live placement preview.

use eframe::egui;
use od_core::{plot_settings::flags, PlotArea};

use crate::layouts::PageDraft;

pub(super) fn content_rect(draft: &PageDraft, printable: egui::Rect, scale: f32) -> egui::Rect {
    if draft.plot.flags & flags::PLOT_CENTERED != 0 {
        return printable.shrink(printable.width().min(printable.height()) * 0.08);
    }
    match draft.plot.area {
        PlotArea::Window { first, second } => {
            let x1 = first.x.min(second.x) as f32 * scale;
            let x2 = first.x.max(second.x) as f32 * scale;
            let y1 = first.y.min(second.y) as f32 * scale;
            let y2 = first.y.max(second.y) as f32 * scale;
            egui::Rect::from_min_max(
                printable.left_top() + egui::vec2(x1, y1),
                printable.left_top() + egui::vec2(x2, y2),
            )
        }
        PlotArea::Layout => printable,
        _ => printable.shrink(printable.width().min(printable.height()) * 0.08),
    }
    .translate(egui::vec2(
        draft.plot.origin_mm.x as f32 * scale,
        -draft.plot.origin_mm.y as f32 * scale,
    ))
}
