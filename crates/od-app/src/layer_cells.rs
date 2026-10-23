//! Text in the Layers panel's name and linetype cells.

use eframe::egui::{self, FontId, Rect, Ui};
use od_ui::Theme;

/// Space left and right of the text in a cell.
const CELL_PAD: f32 = 6.0;

pub(crate) fn cell_text(ui: &Ui, theme: &Theme, rect: Rect, text: &str) {
    fit_text(ui, theme, rect, text, theme.colors.text);
}

/// Paints `text` in `rect` on one row, ending in an ellipsis when it does not
/// fit so it never runs into the next column. Returns whether it was cut.
pub(crate) fn fit_text(
    ui: &Ui,
    theme: &Theme,
    rect: Rect,
    text: &str,
    color: egui::Color32,
) -> bool {
    let font = FontId::proportional(theme.font_size_body);
    let room = (rect.width() - CELL_PAD * 2.0).max(0.0);
    let full = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font.clone(), color);
    let cut = full.size().x > room;
    let mut job = egui::text::LayoutJob::simple(text.to_owned(), font, color, room);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = ui.painter().layout_job(job);
    let pos = egui::pos2(
        rect.left() + CELL_PAD,
        rect.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(pos, galley, color);
    cut
}
