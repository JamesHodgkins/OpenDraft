//! Small pieces only this dialog needs: a group box whose title is a
//! checkbox, and the tab-type glyphs.

use eframe::egui::{
    self, pos2, vec2, Color32, CornerRadius, FontId, Margin, Rect, Sense, Stroke, Ui, UiBuilder,
};
use od_core::text::TabKind;
use od_ui::Theme;

/// What a greyed control says when pointed at.
pub const NOT_YET: &str = "Not available yet: it needs an AutoCAD-made sample to match";

/// Like `od_ui::widgets::group_box`, but the title is a checkbox that turns
/// the group's contents on and off. `available` false greys the whole group
/// out, checkbox included.
pub fn check_group(
    ui: &mut Ui,
    theme: &Theme,
    title: &str,
    on: &mut bool,
    available: bool,
    add_contents: impl FnOnce(&mut Ui),
) {
    let c = &theme.colors;
    let galley = ui.painter().layout_no_wrap(
        title.to_owned(),
        FontId::proportional(theme.font_size_body),
        c.text,
    );
    let height = galley.size().y;
    let enabled = *on && available;
    ui.add_space(height / 2.0);
    let inner = egui::Frame::new()
        .stroke(Stroke::new(1.0_f32, c.border))
        .corner_radius(CornerRadius::same(theme.radius))
        .inner_margin(Margin {
            left: 12,
            right: 12,
            top: 14,
            bottom: 10,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add_enabled_ui(enabled, add_contents).response
        });
    if !available {
        inner.inner.on_disabled_hover_text(NOT_YET);
    }
    let rect = inner.response.rect;
    // Break the border behind the checkbox, then put it on the border.
    let title_rect = Rect::from_min_size(
        pos2(rect.left() + 6.0, rect.top() - height / 2.0),
        vec2(galley.size().x + 36.0, height),
    );
    ui.painter().rect_filled(title_rect, 0.0, c.panel_bg);
    let mut child = ui.new_child(UiBuilder::new().max_rect(title_rect.shrink2(vec2(4.0, 0.0))));
    child.add_enabled(available, egui::Checkbox::new(on, title));
}

/// The mark AutoCAD draws for a tab type: a stem on a base line that runs
/// right of it (left tab), either side (centre), left of it (right), or
/// either side with a point beside it (decimal).
pub fn tab_glyph(ui: &mut Ui, kind: TabKind, colour: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(16.0, 14.0), Sense::hover());
    let paint = ui.painter();
    let line = Stroke::new(1.5_f32, colour);
    let (top, base) = (rect.top() + 1.0, rect.bottom() - 2.0);
    let mid = rect.center().x;
    let (stem, from, to) = match kind {
        TabKind::Left => (mid - 3.0, mid - 3.0, mid + 5.0),
        TabKind::Center | TabKind::Decimal => (mid, mid - 5.0, mid + 5.0),
        TabKind::Right => (mid + 3.0, mid - 5.0, mid + 3.0),
    };
    paint.line_segment([pos2(stem, top), pos2(stem, base)], line);
    paint.line_segment([pos2(from, base), pos2(to, base)], line);
    if kind == TabKind::Decimal {
        paint.circle_filled(pos2(mid + 4.0, base - 3.0), 1.2, colour);
    }
}
