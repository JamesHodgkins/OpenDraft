//! The tabs of the Dimension Style dialog. Each one shows the controls whose
//! values the style keeps and says so about the rest, rather than showing
//! controls that do nothing.

use eframe::egui::{self, RichText};
use od_core::units::MAX_PRECISION;
use od_core::{AngleFormat, Arrow, Document, LengthFormat, Units};
use od_ui::{widgets, Theme};

use crate::dim_style_draft::{alignment, Draft};

const LABEL_WIDTH: f32 = 150.0;
const FIELD_WIDTH: f32 = 170.0;

/// AutoCAD's tab names, in its order.
pub(super) const TABS: [&str; 7] = [
    "Lines",
    "Symbols and Arrows",
    "Text",
    "Fit",
    "Primary Units",
    "Alternate Units",
    "Tolerances",
];

const VERTICAL: [&str; 5] = ["Centered", "Above", "Outside", "JIS", "Below"];
const HORIZONTAL: [&str; 5] = [
    "Centered",
    "At Ext Line 1",
    "At Ext Line 2",
    "Over Ext Line 1",
    "Over Ext Line 2",
];
const SEPARATORS: [(char, &str); 3] = [
    ('.', "'.' (Period)"),
    (',', "',' (Comma)"),
    (' ', "' ' (Space)"),
];
/// `DIMATFIT` values in the order AutoCAD lists the choices.
const FIT_ORDER: [u8; 4] = [3, 1, 2, 0];
const FIT_CHOICES: [&str; 5] = [
    "Either text or arrows (best fit)",
    "Arrows",
    "Text",
    "Both text and arrows",
    "Always keep text between ext lines",
];
const MOVED_TEXT: [&str; 3] = [
    "Beside the dimension line",
    "Over dimension line, with leader",
    "Over dimension line, without leader",
];

/// A caption and a control on one row.
fn row(ui: &mut egui::Ui, theme: &Theme, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [LABEL_WIDTH, 24.0],
            egui::Label::new(RichText::new(label).color(theme.colors.text)),
        );
        add(ui);
    });
    ui.add_space(4.0);
}

/// A drop-down over `labels`, choosing an index.
fn choice(ui: &mut egui::Ui, theme: &Theme, id: &str, labels: &[&str], value: &mut usize) {
    let shown = labels.get(*value).copied().unwrap_or("");
    widgets::dropdown(ui, theme, id, shown, FIELD_WIDTH, |ui| {
        for (i, label) in labels.iter().enumerate() {
            ui.selectable_value(value, i, *label);
        }
    });
}

/// A length: a number that cannot go below zero.
fn length(ui: &mut egui::Ui, theme: &Theme, value: &mut f64) {
    widgets::number_field(ui, theme, value, "", 0.05);
    if !value.is_finite() || *value < 0.0 {
        *value = 0.0;
    }
}

fn radios(ui: &mut egui::Ui, labels: &[&str], value: &mut usize) {
    for (i, label) in labels.iter().enumerate() {
        if ui.radio(*value == i, *label).clicked() {
            *value = i;
        }
        ui.add_space(2.0);
    }
}

pub(super) fn lines(ui: &mut egui::Ui, theme: &Theme, draft: &mut Draft) {
    let s = &mut draft.style;
    widgets::group_box(ui, theme, "Dimension lines", |ui| {
        ui.horizontal(|ui| {
            ui.label("Suppress:");
            ui.checkbox(&mut s.suppress_line[0], "Dim line 1");
            ui.checkbox(&mut s.suppress_line[1], "Dim line 2");
        });
    });
    ui.add_space(8.0);
    widgets::group_box(ui, theme, "Extension lines", |ui| {
        row(ui, theme, "Extend beyond dim lines:", |ui| {
            length(ui, theme, &mut s.ext_extend)
        });
        row(ui, theme, "Offset from origin:", |ui| {
            length(ui, theme, &mut s.ext_offset)
        });
        ui.horizontal(|ui| {
            ui.label("Suppress:");
            ui.checkbox(&mut s.suppress_ext[0], "Ext line 1");
            ui.checkbox(&mut s.suppress_ext[1], "Ext line 2");
        });
    });
    ui.add_space(8.0);
    note(ui, theme, "Colour, linetype, lineweight, baseline spacing and fixed-length extension lines are not kept by the style yet.");
}

pub(super) fn symbols(ui: &mut egui::Ui, theme: &Theme, draft: &mut Draft) {
    let s = &mut draft.style;
    widgets::group_box(ui, theme, "Arrowheads", |ui| {
        let mut kind = Arrow::ALL.iter().position(|a| *a == s.arrow).unwrap_or(0);
        let labels = Arrow::ALL.map(Arrow::label);
        row(ui, theme, "Arrowheads:", |ui| {
            choice(ui, theme, "dim_arrow", &labels, &mut kind)
        });
        s.arrow = Arrow::ALL.get(kind).copied().unwrap_or(Arrow::Closed);
        row(ui, theme, "Arrow size:", |ui| {
            length(ui, theme, &mut s.arrow_size)
        });
    });
    ui.add_space(8.0);
    widgets::group_box(ui, theme, "Center marks", |ui| {
        let mut mark = usize::from(s.centre_mark > 0.0);
        radios(ui, &["None", "Mark"], &mut mark);
        if mark == 0 {
            s.centre_mark = 0.0;
        } else if s.centre_mark <= 0.0 {
            s.centre_mark = 2.5;
        }
        ui.add_enabled_ui(mark == 1, |ui| {
            row(ui, theme, "Size:", |ui| {
                length(ui, theme, &mut s.centre_mark)
            });
        });
    });
    ui.add_space(8.0);
    widgets::group_box(ui, theme, "Arc length symbol", |ui| {
        let mut at = usize::from(s.arc_symbol.min(2));
        radios(
            ui,
            &["Preceding dimension text", "Above dimension text", "None"],
            &mut at,
        );
        s.arc_symbol = at as u8;
    });
    ui.add_space(8.0);
    note(ui, theme, "Separate first, second and leader arrowheads, dimension breaks and jogs are not kept by the style yet.");
}

pub(super) fn text(ui: &mut egui::Ui, theme: &Theme, draft: &mut Draft, document: &Document) {
    let want = &mut draft.want_text_styles;
    let s = &mut draft.style;
    widgets::group_box(ui, theme, "Text appearance", |ui| {
        row(ui, theme, "Text style:", |ui| {
            let styles = document.text_styles();
            let name = styles
                .iter()
                .find(|t| t.id == s.text_style)
                .map_or("Standard", |t| t.name.as_str());
            widgets::dropdown(
                ui,
                theme,
                "dim_text_style",
                name,
                FIELD_WIDTH - 36.0,
                |ui| {
                    for style in styles {
                        ui.selectable_value(&mut s.text_style, style.id, &style.name);
                    }
                },
            );
            if widgets::secondary_button(ui, theme, "...").clicked() {
                *want = true;
            }
        });
        row(ui, theme, "Text height:", |ui| {
            widgets::number_field(ui, theme, &mut s.text_height, "", 0.05);
            s.text_height = s.text_height.max(1e-6);
        });
    });
    ui.add_space(8.0);
    widgets::group_box(ui, theme, "Text placement", |ui| {
        let mut vertical = usize::from(s.text_above.min(4));
        row(ui, theme, "Vertical:", |ui| {
            choice(ui, theme, "dim_text_vertical", &VERTICAL, &mut vertical)
        });
        s.text_above = vertical as u8;
        let mut horizontal = usize::from(s.text_just.min(4));
        row(ui, theme, "Horizontal:", |ui| {
            choice(
                ui,
                theme,
                "dim_text_horizontal",
                &HORIZONTAL,
                &mut horizontal,
            )
        });
        s.text_just = horizontal as u8;
        row(ui, theme, "Offset from dim line:", |ui| {
            length(ui, theme, &mut s.text_gap)
        });
    });
    ui.add_space(8.0);
    widgets::group_box(ui, theme, "Text alignment", |ui| {
        let mut how = alignment::of(s);
        radios(
            ui,
            &["Horizontal", "Aligned with dimension line", "ISO standard"],
            &mut how,
        );
        alignment::set(s, how);
    });
    ui.add_space(6.0);
    note(ui, theme, "Text colour, fill colour, fraction height scale, frames and view direction are not kept by the style yet.");
}

pub(super) fn fit(ui: &mut egui::Ui, theme: &Theme, draft: &mut Draft) {
    let s = &mut draft.style;
    widgets::group_box(ui, theme, "Fit options", |ui| {
        widgets::field_label(
            ui,
            theme,
            "If there isn't enough room to place both text and arrows inside extension lines, the first thing to move outside is:",
        );
        ui.add_space(4.0);
        let mut at = if s.text_inside {
            4
        } else {
            FIT_ORDER.iter().position(|c| *c == s.fit).unwrap_or(0)
        };
        radios(ui, &FIT_CHOICES, &mut at);
        if at == 4 {
            s.text_inside = true;
        } else {
            s.text_inside = false;
            s.fit = FIT_ORDER.get(at).copied().unwrap_or(3);
        }
    });
    ui.add_space(8.0);
    widgets::group_box(ui, theme, "Text placement", |ui| {
        widgets::field_label(
            ui,
            theme,
            "When text is not in the default position, place it:",
        );
        ui.add_space(4.0);
        let mut at = usize::from(s.text_move.min(2));
        radios(ui, &MOVED_TEXT, &mut at);
        s.text_move = at as u8;
    });
    ui.add_space(8.0);
    widgets::group_box(ui, theme, "Scale for dimension features", |ui| {
        row(ui, theme, "Use overall scale of:", |ui| {
            widgets::number_field(ui, theme, &mut s.scale, "", 0.1);
            s.scale = s.scale.max(1e-6);
        });
    });
    ui.add_space(8.0);
    widgets::group_box(ui, theme, "Fine tuning", |ui| {
        ui.checkbox(&mut s.force_line, "Draw dim line between ext lines");
    });
}

pub(super) fn primary(ui: &mut egui::Ui, theme: &Theme, draft: &mut Draft) {
    let Draft {
        style: s,
        prefix,
        suffix,
        ..
    } = draft;
    let mut post_changed = false;
    widgets::group_box(ui, theme, "Linear dimensions", |ui| {
        let mut format = LengthFormat::ALL
            .iter()
            .position(|f| *f == s.length_format)
            .unwrap_or(1);
        let names = LengthFormat::ALL.map(LengthFormat::label);
        row(ui, theme, "Unit format:", |ui| {
            choice(ui, theme, "dim_unit_format", &names, &mut format)
        });
        s.length_format = LengthFormat::ALL.get(format).copied().unwrap_or_default();
        row(ui, theme, "Precision:", |ui| {
            let now = Units::length_precision_label(s.length_format, s.precision);
            widgets::dropdown(ui, theme, "dim_precision", &now, FIELD_WIDTH, |ui| {
                for p in 0..=MAX_PRECISION {
                    let label = Units::length_precision_label(s.length_format, p);
                    ui.selectable_value(&mut s.precision, p, label);
                }
            });
        });
        let decimal = s.length_format == LengthFormat::Decimal;
        ui.add_enabled_ui(decimal, |ui| {
            row(ui, theme, "Decimal separator:", |ui| {
                let shown = SEPARATORS
                    .iter()
                    .find(|(c, _)| *c == s.decimal_separator)
                    .map_or("'.' (Period)", |(_, label)| *label);
                widgets::dropdown(ui, theme, "dim_separator", shown, FIELD_WIDTH, |ui| {
                    for (c, label) in SEPARATORS {
                        ui.selectable_value(&mut s.decimal_separator, c, label);
                    }
                });
            });
        });
        row(ui, theme, "Round off:", |ui| {
            length(ui, theme, &mut s.round_to)
        });
        row(ui, theme, "Prefix:", |ui| {
            post_changed |= widgets::text_field(ui, theme, prefix, "").changed();
        });
        row(ui, theme, "Suffix:", |ui| {
            post_changed |= widgets::text_field(ui, theme, suffix, "").changed();
        });
    });
    ui.add_space(8.0);
    widgets::group_box(ui, theme, "Measurement scale", |ui| {
        row(ui, theme, "Scale factor:", |ui| {
            widgets::number_field(ui, theme, &mut s.linear_factor, "", 0.05);
            if !s.linear_factor.is_finite() || s.linear_factor == 0.0 {
                s.linear_factor = 1.0;
            }
        });
    });
    ui.add_space(8.0);
    widgets::group_box(ui, theme, "Angular dimensions", |ui| {
        let mut format = AngleFormat::ALL
            .iter()
            .position(|f| *f == s.angle_format)
            .unwrap_or(0);
        let names = AngleFormat::ALL.map(AngleFormat::label);
        row(ui, theme, "Units format:", |ui| {
            choice(ui, theme, "dim_angle_format", &names, &mut format)
        });
        s.angle_format = AngleFormat::ALL.get(format).copied().unwrap_or_default();
        row(ui, theme, "Precision:", |ui| {
            let now = Units::angle_precision_label(s.angle_format, s.angle_precision);
            widgets::dropdown(ui, theme, "dim_angle_precision", &now, FIELD_WIDTH, |ui| {
                for p in 0..=MAX_PRECISION {
                    let label = Units::angle_precision_label(s.angle_format, p);
                    ui.selectable_value(&mut s.angle_precision, p, label);
                }
            });
        });
    });
    ui.add_space(8.0);
    widgets::group_box(ui, theme, "Zero suppression", |ui| {
        ui.add_enabled_ui(s.length_format == LengthFormat::Decimal, |ui| {
            ui.checkbox(&mut s.trim_zeros, "Trailing");
        });
    });
    if post_changed {
        draft.sync_post();
    }
}

/// Alternate units and tolerances are not part of the style yet.
pub(super) fn unsupported(ui: &mut egui::Ui, theme: &Theme, what: &str) {
    note(
        ui,
        theme,
        &format!(
            "{what} are not kept by the dimension style yet, so there is nothing to set here."
        ),
    );
}

fn note(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    ui.add(egui::Label::new(
        RichText::new(text)
            .size(theme.font_size_small)
            .color(theme.colors.text_weak),
    ));
}
