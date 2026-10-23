//! Themed widgets. These wrap egui widgets with theme-driven styling.

use egui::{
    Align2, Button, CornerRadius, FontId, Margin, Rect, Response, RichText, Sense, Stroke, Ui,
};

use crate::{icons::DIALOG_CLOSE, Theme};

/// Filled accent-blue button for the primary action in a dialog or panel.
pub fn primary_button(ui: &mut Ui, theme: &Theme, text: &str) -> Response {
    let c = &theme.colors;
    ui.scope(|ui| {
        let w = &mut ui.style_mut().visuals.widgets;
        let states = [
            (&mut w.inactive, c.accent),
            (&mut w.hovered, c.accent_hover),
            (&mut w.active, c.accent_pressed),
        ];
        for (state, fill) in states {
            state.weak_bg_fill = fill;
            state.bg_fill = fill;
            state.bg_stroke = Stroke::NONE;
            state.fg_stroke = Stroke::new(1.0_f32, c.text_on_accent);
        }
        ui.add(
            Button::new(RichText::new(text).strong())
                .corner_radius(CornerRadius::same(theme.pill_radius))
                .min_size(egui::vec2(theme.unit * 9.0, theme.control_height)),
        )
    })
    .inner
}

/// Outlined, transparent pill button for secondary actions (Close, Cancel).
pub fn secondary_button(ui: &mut Ui, theme: &Theme, text: &str) -> Response {
    let c = &theme.colors;
    ui.scope(|ui| {
        let w = &mut ui.style_mut().visuals.widgets;
        w.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
        w.inactive.bg_stroke = Stroke::new(1.5_f32, c.text);
        w.hovered.bg_stroke = Stroke::new(1.5_f32, c.text);
        w.active.bg_stroke = Stroke::new(1.5_f32, c.text);
        ui.add(
            Button::new(RichText::new(text).strong())
                .corner_radius(CornerRadius::same(theme.pill_radius))
                .min_size(egui::vec2(theme.unit * 9.0, theme.control_height)),
        )
    })
    .inner
}

pub use crate::icon_set::{icon_button, icon_text};

/// Text tabs with an underline on the active one. Returns true if the
/// selection changed.
pub fn tab_strip(ui: &mut Ui, theme: &Theme, selected: &mut usize, labels: &[&str]) -> bool {
    let c = &theme.colors;
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (i, label) in labels.iter().enumerate() {
            let is_selected = *selected == i;
            let font = FontId::proportional(theme.font_size_tab);
            let galley = ui.painter().layout_no_wrap(
                (*label).to_owned(),
                font,
                if is_selected { c.text } else { c.text_weak },
            );
            let pad = egui::vec2(theme.unit * 1.25, theme.unit * 0.5);
            let (rect, response) =
                ui.allocate_exact_size(galley.size() + pad * 2.0, egui::Sense::click());
            let color = if is_selected || response.hovered() {
                c.text
            } else {
                c.text_weak
            };
            ui.painter().galley(rect.min + pad, galley, color);
            if is_selected {
                let underline = egui::Rect::from_min_max(
                    egui::pos2(rect.left() + theme.unit, rect.bottom() - 2.0),
                    egui::pos2(rect.right() - theme.unit, rect.bottom()),
                );
                ui.painter().rect_filled(underline, 0.0, c.text);
            }
            if response.clicked() && !is_selected {
                *selected = i;
                changed = true;
            }
        }
    });
    changed
}

/// Style the current (scoped) `ui` so inputs look like dark wells with a thin
/// border and a blue focus ring. Call inside `ui.scope`.
fn input_well_style(ui: &mut Ui, theme: &Theme) {
    let c = &theme.colors;
    let v = ui.visuals_mut();
    v.selection.stroke = Stroke::new(1.0_f32, c.accent_hover);
    let w = &mut v.widgets;
    let states = [
        (&mut w.inactive, c.border),
        (&mut w.hovered, c.border_strong),
        (&mut w.active, c.accent_hover),
        (&mut w.open, c.accent_hover),
    ];
    for (state, border) in states {
        state.weak_bg_fill = c.input_bg;
        state.bg_fill = c.input_bg;
        state.bg_stroke = Stroke::new(1.0_f32, border);
    }
}

/// Small gray caption above a field, e.g. `Width`.
pub fn field_label(ui: &mut Ui, theme: &Theme, text: &str) -> Response {
    ui.label(
        RichText::new(text)
            .size(theme.font_size_small)
            .color(theme.colors.text_weak),
    )
}

/// Single-line text input as a dark well. Fills the available width.
pub fn text_field(ui: &mut Ui, theme: &Theme, text: &mut String, hint: &str) -> Response {
    ui.scope(|ui| {
        input_well_style(ui, theme);
        ui.add(
            egui::TextEdit::singleline(text)
                .hint_text(hint)
                .margin(Margin::symmetric(8, 5))
                .desired_width(f32::INFINITY),
        )
    })
    .inner
}

/// Numeric drag field with a unit suffix, e.g. `12.5 mm`, at the default width.
pub fn number_field(
    ui: &mut Ui,
    theme: &Theme,
    value: &mut f64,
    suffix: &str,
    speed: f64,
) -> Response {
    number_field_width(ui, theme, value, suffix, speed, theme.unit * 9.0)
}

/// Like [`number_field`] with an explicit minimum width; pass
/// `ui.available_width()` to fill a row.
pub fn number_field_width(
    ui: &mut Ui,
    theme: &Theme,
    value: &mut f64,
    suffix: &str,
    speed: f64,
    width: f32,
) -> Response {
    ui.scope(|ui| {
        input_well_style(ui, theme);
        ui.spacing_mut().interact_size.x = width;
        ui.add(
            egui::DragValue::new(value)
                .speed(speed)
                .suffix(if suffix.is_empty() {
                    String::new()
                } else {
                    format!(" {suffix}")
                }),
        )
    })
    .inner
}

/// Dropdown styled as an input well with a chevron. `width` is the total
/// width of the closed control; pass `ui.available_width()` to fill a row.
pub fn dropdown(
    ui: &mut Ui,
    theme: &Theme,
    id_salt: impl std::hash::Hash,
    selected: &str,
    width: f32,
    add_items: impl FnOnce(&mut Ui),
) -> Response {
    ui.scope(|ui| {
        input_well_style(ui, theme);
        ui.spacing_mut().button_padding = egui::vec2(theme.unit * 1.25, theme.unit * 0.5);
        egui::ComboBox::from_id_salt(id_salt)
            .selected_text(selected)
            .width(width)
            .icon(|ui, rect, visuals, _is_open, _above_or_below| {
                crate::icon_set::paint(
                    ui.painter(),
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    crate::icons::DROPDOWN_ARROW,
                    14.0,
                    visuals.fg_stroke.color,
                );
            })
            .show_ui(ui, add_items)
            .response
    })
    .inner
}

/// Square icon toggle (e.g. orientation). Selected state is filled blue.
pub fn icon_toggle(
    ui: &mut Ui,
    theme: &Theme,
    icon: char,
    selected: bool,
    tooltip: &str,
) -> Response {
    let c = &theme.colors;
    let side = theme.control_height;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(side, side), Sense::click());
    let fill = if selected {
        c.accent
    } else if response.hovered() {
        c.hover
    } else {
        c.panel_raised
    };
    let radius = CornerRadius::same(theme.radius);
    ui.painter().rect_filled(rect, radius, fill);
    if !selected {
        ui.painter().rect_stroke(
            rect,
            radius,
            Stroke::new(1.0_f32, c.border),
            egui::StrokeKind::Inside,
        );
    }
    crate::icon_set::paint(
        ui.painter(),
        rect.center(),
        Align2::CENTER_CENTER,
        icon,
        18.0,
        if selected { c.text_on_accent } else { c.text },
    );
    response.on_hover_text(tooltip)
}

const CARD_SIZE: f32 = 148.0;

/// Preset card: large icon over a title and subtitle. A selected card gets a
/// dashed blue outline.
pub fn preset_card(
    ui: &mut Ui,
    theme: &Theme,
    icon: char,
    title: &str,
    subtitle: &str,
    selected: bool,
) -> Response {
    let c = &theme.colors;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(CARD_SIZE, CARD_SIZE), Sense::click());
    let radius = CornerRadius::same(theme.radius);
    if selected {
        ui.painter().rect_filled(rect, radius, c.input_bg);
        let corners = [
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
            rect.left_top(),
        ];
        ui.painter().extend(egui::Shape::dashed_line(
            &corners,
            Stroke::new(2.0_f32, c.accent_hover),
            3.0,
            2.0,
        ));
    } else if response.hovered() {
        ui.painter().rect_filled(rect, radius, c.panel_raised);
    }
    let cx = rect.center().x;
    crate::icon_set::paint(
        ui.painter(),
        egui::pos2(cx, rect.top() + 54.0),
        Align2::CENTER_CENTER,
        icon,
        if selected { 30.0 } else { 56.0 },
        c.text_weak,
    );
    ui.painter().text(
        egui::pos2(cx, rect.top() + 108.0),
        Align2::CENTER_CENTER,
        title,
        FontId::proportional(theme.font_size_small),
        c.text,
    );
    ui.painter().text(
        egui::pos2(cx, rect.top() + 125.0),
        Align2::CENTER_CENTER,
        subtitle,
        FontId::proportional(theme.font_size_small - 1.0),
        c.text_weak,
    );
    response
}

/// Template card: a preview area (here a flat tint) over a caption bar with
/// the title and a small badge.
pub fn template_card(
    ui: &mut Ui,
    theme: &Theme,
    tint: egui::Color32,
    title: &str,
    badge: &str,
) -> Response {
    let c = &theme.colors;
    let caption_h = 30.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(CARD_SIZE, CARD_SIZE), Sense::click());
    let preview = egui::Rect::from_min_max(
        rect.min,
        egui::pos2(rect.right(), rect.bottom() - caption_h),
    );
    let caption = egui::Rect::from_min_max(egui::pos2(rect.left(), preview.bottom()), rect.max);
    let top = CornerRadius {
        nw: theme.radius,
        ne: theme.radius,
        sw: 0,
        se: 0,
    };
    let bottom = CornerRadius {
        nw: 0,
        ne: 0,
        sw: theme.radius,
        se: theme.radius,
    };
    ui.painter().rect_filled(preview, top, tint);
    ui.painter().rect_filled(
        caption,
        bottom,
        if response.hovered() {
            c.hover
        } else {
            c.panel_raised
        },
    );
    let font = FontId::proportional(theme.font_size_small);
    let badge_w = ui
        .painter()
        .layout_no_wrap(badge.to_owned(), font.clone(), c.text_weak)
        .size()
        .x;
    ui.painter().text(
        egui::pos2(caption.right() - 10.0, caption.center().y),
        Align2::RIGHT_CENTER,
        badge,
        font.clone(),
        c.text_weak,
    );
    let mut job = egui::text::LayoutJob::simple(
        title.to_owned(),
        font,
        c.text,
        caption.width() - 20.0 - badge_w - 8.0,
    );
    job.wrap.max_rows = 1;
    let galley = ui.painter().layout_job(job);
    let pos = egui::pos2(
        caption.left() + 10.0,
        caption.center().y - galley.size().y / 2.0,
    );
    ui.painter().galley(pos, galley, c.text);
    response
}

/// One `label  [editor]` row for property panels. The label column has a
/// fixed width so editors line up across rows.
pub fn property_row<R>(
    ui: &mut Ui,
    theme: &Theme,
    label: &str,
    add_editor: impl FnOnce(&mut Ui) -> R,
) -> R {
    ui.horizontal(|ui| {
        ui.add_sized(
            [theme.unit * 12.0, theme.control_height],
            egui::Label::new(RichText::new(label).color(theme.colors.text_weak)).truncate(),
        );
        add_editor(ui)
    })
    .inner
}

/// Collapsible group with a small all-caps header.
pub fn section<R>(
    ui: &mut Ui,
    theme: &Theme,
    title: &str,
    default_open: bool,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    egui::CollapsingHeader::new(
        RichText::new(title.to_uppercase())
            .small()
            .color(theme.colors.text_weak),
    )
    .id_salt(title)
    .default_open(default_open)
    .show(ui, add_contents)
    .body_returned
}

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
/// Small all-caps gray heading, e.g. `PRESET DETAILS`.
pub fn section_header(ui: &mut Ui, theme: &Theme, text: &str) -> Response {
    ui.label(
        RichText::new(text.to_uppercase())
            .small()
            .color(theme.colors.text_weak),
    )
}
