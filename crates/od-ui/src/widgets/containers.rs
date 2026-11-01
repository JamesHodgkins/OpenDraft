//! Dialogs, group boxes and tab folders: the widgets that hold others.

use egui::{Align2, CornerRadius, FontId, Margin, Rect, RichText, Sense, Stroke, Ui};

use super::icon_button;
use crate::{icons::DIALOG_CLOSE, Theme};

/// Modal-style dialog window with a themed header (left-aligned title and a
/// close button), a divider and a padded body. Draggable, not resizable, and
/// centered each time it opens.
/// Sets `*open = false` when closed with the X button or Escape. Returns the
/// closure's result while the dialog is visible.
pub fn dialog<R>(
    ctx: &egui::Context,
    theme: &Theme,
    title: &str,
    open: &mut bool,
    width: f32,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    let c = &theme.colors;
    let radius = theme.radius + 3;
    let frame = egui::Frame::new()
        .fill(c.panel_bg)
        .stroke(Stroke::new(1.0_f32, c.border))
        .corner_radius(CornerRadius::same(radius))
        .shadow(ctx.style().visuals.window_shadow);
    let header_height = 36.0;
    let mut close = false;

    let window_id = egui::Id::new(title);
    let layer_id = egui::LayerId::new(egui::Order::Middle, window_id);
    let was_visible = ctx.memory(|memory| memory.areas().visible_last_frame(&layer_id));
    let mut window = egui::Window::new(title)
        .title_bar(false)
        .frame(frame)
        .resizable(false)
        .collapsible(false)
        .pivot(Align2::CENTER_CENTER);
    if !was_visible {
        // Positions are remembered by title, so override the old one whenever
        // a dialog reappears. `current_pos` (not an anchor) keeps it draggable.
        window = window.current_pos(ctx.screen_rect().center());
    }

    let result = window
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            ui.set_width(width);

            egui::Frame::new()
                .fill(c.panel_raised)
                .corner_radius(CornerRadius {
                    nw: radius,
                    ne: radius,
                    sw: 0,
                    se: 0,
                })
                .inner_margin(Margin {
                    left: 16,
                    right: 6,
                    top: 0,
                    bottom: 0,
                })
                .show(ui, |ui| {
                    ui.set_width(width - 22.0);
                    ui.horizontal(|ui| {
                        ui.set_height(header_height);
                        ui.label(RichText::new(title).size(theme.font_size_body + 0.5));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if icon_button(ui, theme, DIALOG_CLOSE, "Close").clicked() {
                                close = true;
                            }
                        });
                    });
                });
            let (line, _) =
                ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), Sense::hover());
            ui.painter().rect_filled(line, 0.0, c.border);

            egui::Frame::new()
                .inner_margin(Margin::same(16))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(theme.unit, theme.unit * 0.5);
                    ui.set_width(width - 32.0);
                    add_contents(ui)
                })
                .inner
        })
        .and_then(|response| response.inner);

    if close
        || (ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !ctx.memory(|m| m.any_popup_open()))
    {
        *open = false;
    }
    result
}

/// Titled group box: a bordered, rounded frame whose title sits on the top
/// border (like a classic dialog "fieldset"). Fills the available width.
pub fn group_box<R>(
    ui: &mut Ui,
    theme: &Theme,
    title: &str,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let c = &theme.colors;
    let galley = ui.painter().layout_no_wrap(
        title.to_owned(),
        FontId::proportional(theme.font_size_body),
        c.text,
    );
    // Room for the title that straddles the top border.
    ui.add_space(galley.size().y / 2.0);
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
            add_contents(ui)
        });
    let rect = inner.response.rect;
    let pos = egui::pos2(rect.left() + 10.0, rect.top() - galley.size().y / 2.0);
    // Break the border behind the title.
    let gap = Rect::from_min_size(
        pos - egui::vec2(4.0, 0.0),
        galley.size() + egui::vec2(8.0, 0.0),
    );
    ui.painter().rect_filled(gap, 0.0, c.panel_bg);
    ui.painter().galley(pos, galley, c.text);
    inner.inner
}

/// Tab folder: tabs attached to a bordered page. The selected tab shares the
/// page's fill so the two read as one piece. `add_page` receives the selected
/// index. `min_page_height` keeps the dialog from resizing between tabs.
pub fn tabbed_page<R>(
    ui: &mut Ui,
    theme: &Theme,
    selected: &mut usize,
    titles: &[&str],
    min_page_height: f32,
    add_page: impl FnOnce(&mut Ui, usize) -> R,
) -> R {
    let c = &theme.colors;
    let font = FontId::proportional(theme.font_size_body);
    let top = CornerRadius {
        nw: theme.radius,
        ne: theme.radius,
        sw: 0,
        se: 0,
    };
    let spacing = ui.spacing().item_spacing;
    let tab_height = 26.0;

    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let mut tab_rects = Vec::with_capacity(titles.len());
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (i, title) in titles.iter().enumerate() {
                let text_w = ui
                    .painter()
                    .layout_no_wrap((*title).to_owned(), font.clone(), c.text)
                    .size()
                    .x;
                let (rect, response) =
                    ui.allocate_exact_size(egui::vec2(text_w + 24.0, tab_height), Sense::click());
                if response.clicked() {
                    *selected = i;
                }
                if *selected != i {
                    let fill = if response.hovered() {
                        c.panel_raised
                    } else {
                        c.window_bg
                    };
                    ui.painter().rect_filled(rect, top, fill);
                    ui.painter().rect_stroke(
                        rect,
                        top,
                        Stroke::new(1.0_f32, c.border),
                        egui::StrokeKind::Inside,
                    );
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        title,
                        font.clone(),
                        c.text_weak,
                    );
                }
                tab_rects.push(rect);
            }
        });

        let result = egui::Frame::new()
            .fill(c.panel_bg)
            .stroke(Stroke::new(1.0_f32, c.border))
            .corner_radius(CornerRadius {
                nw: 0,
                ne: theme.radius,
                sw: theme.radius,
                se: theme.radius,
            })
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = spacing;
                ui.set_width(ui.available_width());
                ui.set_min_height(min_page_height);
                add_page(ui, *selected)
            })
            .inner;

        // Selected tab drawn last so it overlaps (and hides) the page's top
        // border beneath it.
        if let Some(rect) = tab_rects.get(*selected) {
            let joined =
                Rect::from_min_max(rect.min, egui::pos2(rect.right(), rect.bottom() + 1.0));
            ui.painter().rect_filled(joined, top, c.panel_bg);
            ui.painter().rect_stroke(
                *rect,
                top,
                Stroke::new(1.0_f32, c.border),
                egui::StrokeKind::Inside,
            );
            ui.painter().line_segment(
                [
                    egui::pos2(rect.left() + 1.0, rect.bottom()),
                    egui::pos2(rect.right() - 1.0, rect.bottom()),
                ],
                Stroke::new(1.5_f32, c.panel_bg),
            );
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                titles[*selected],
                font.clone(),
                c.text,
            );
        }
        result
    })
    .inner
}
