//! The tabs of the Modify Multileader Style dialog, in AutoCAD's order. Each
//! shows the controls whose values the style keeps and says so about the
//! rest, rather than showing controls that do nothing.

use eframe::egui::{self, RichText};
use od_core::mleader::{
    LeaderAttachment, MLeaderContentType, MLeaderLineType, MLeaderStyle, TextAngle, MAX_POINTS,
};
use od_core::{Arrow, Attr, Color, Document, Lineweight};
use od_ui::{widgets, Theme};

use super::mleader_style_dialog::Draft;
use crate::colour_ui::{self, Pick};
use crate::linetype_ui;

const LABEL_WIDTH: f32 = 140.0;
const FIELD_WIDTH: f32 = 190.0;
const SMALL_WIDTH: f32 = 90.0;

pub(super) const TABS: [&str; 3] = ["Leader Format", "Leader Structure", "Content"];

/// The angles a segment can be held to, in degrees.
const ANGLES: [f64; 6] = [0.0, 15.0, 30.0, 45.0, 60.0, 90.0];

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

fn note(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    ui.add(egui::Label::new(
        RichText::new(text)
            .size(theme.font_size_small)
            .color(theme.colors.text_weak),
    ));
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
    widgets::number_field_width(ui, theme, value, "", 0.05, SMALL_WIDTH);
    if !value.is_finite() || *value < 0.0 {
        *value = 0.0;
    }
}

fn weight_label(weight: Lineweight) -> String {
    format!("{:.2} mm", weight.mm())
}

fn colour_row(ui: &mut egui::Ui, theme: &Theme, id: &str, label: &str, value: &mut Attr<Color>) {
    row(ui, theme, label, |ui| {
        let (swatch, text) = match value {
            Attr::ByLayer => (None, "ByLayer".to_owned()),
            Attr::Set(color) => (Some(*color), colour_ui::name(*color)),
        };
        let at = egui::vec2(FIELD_WIDTH, 24.0);
        let picked = colour_ui::field(ui, theme, ui.id().with(id), at, swatch, &text, true);
        match picked {
            Some(Pick::ByLayer) => *value = Attr::ByLayer,
            Some(Pick::Color(c)) => *value = Attr::Set(c),
            None => {}
        }
    });
}

fn linetype_row(ui: &mut egui::Ui, theme: &Theme, value: &mut Attr<String>, doc: &Document) {
    row(ui, theme, "Linetype:", |ui| {
        let shown = match value {
            Attr::ByLayer => "ByLayer".to_owned(),
            Attr::Set(name) => name.clone(),
        };
        widgets::dropdown(ui, theme, "ml_linetype", &shown, FIELD_WIDTH, |ui| {
            if ui
                .selectable_label(matches!(value, Attr::ByLayer), "ByLayer")
                .clicked()
            {
                *value = Attr::ByLayer;
            }
            for record in linetype_ui::choices(doc) {
                let on = matches!(value, Attr::Set(n) if n.eq_ignore_ascii_case(&record.name));
                if ui.selectable_label(on, &record.name).clicked() {
                    *value = Attr::Set(record.name.clone());
                }
            }
        });
    });
}

fn lineweight_row(ui: &mut egui::Ui, theme: &Theme, value: &mut Attr<Lineweight>) {
    row(ui, theme, "Lineweight:", |ui| {
        let shown = match value {
            Attr::ByLayer => "ByLayer".to_owned(),
            Attr::Set(w) => weight_label(*w),
        };
        widgets::dropdown(ui, theme, "ml_lineweight", &shown, FIELD_WIDTH, |ui| {
            if ui
                .selectable_label(matches!(value, Attr::ByLayer), "ByLayer")
                .clicked()
            {
                *value = Attr::ByLayer;
            }
            for weight in Lineweight::STANDARD {
                let on = matches!(value, Attr::Set(w) if w == weight);
                if ui.selectable_label(on, weight_label(*weight)).clicked() {
                    *value = Attr::Set(*weight);
                }
            }
        });
    });
}

pub(super) fn leader_format(ui: &mut egui::Ui, theme: &Theme, draft: &mut Draft, doc: &Document) {
    let s = &mut draft.style;
    widgets::group_box(ui, theme, "General", |ui| {
        let mut kind = MLeaderLineType::ALL
            .iter()
            .position(|t| *t == s.leader_type)
            .unwrap_or(0);
        let labels = MLeaderLineType::ALL.map(MLeaderLineType::label);
        row(ui, theme, "Type:", |ui| {
            choice(ui, theme, "ml_leader_type", &labels, &mut kind)
        });
        s.leader_type = MLeaderLineType::ALL[kind];
        colour_row(ui, theme, "ml_leader_color", "Color:", &mut s.leader_color);
        linetype_row(ui, theme, &mut s.leader_linetype, doc);
        lineweight_row(ui, theme, &mut s.leader_lineweight);
    });
    ui.add_space(8.0);
    widgets::group_box(ui, theme, "Arrowhead", |ui| {
        let mut kind = Arrow::ALL.iter().position(|a| *a == s.arrow).unwrap_or(0);
        let labels = Arrow::ALL.map(Arrow::label);
        row(ui, theme, "Symbol:", |ui| {
            choice(ui, theme, "ml_arrow", &labels, &mut kind)
        });
        s.arrow = Arrow::ALL[kind];
        row(ui, theme, "Size:", |ui| {
            length(ui, theme, &mut s.arrow_size)
        });
    });
    ui.add_space(8.0);
    widgets::group_box(ui, theme, "Leader break", |ui| {
        row(ui, theme, "Break size:", |ui| {
            length(ui, theme, &mut s.break_gap)
        });
    });
    ui.add_space(8.0);
    note(ui, theme, "ByBlock colour, linetype and lineweight and custom arrowhead blocks are not kept by the style yet.");
}

/// A checkbox that holds a segment to one of [`ANGLES`] when ticked.
fn angle_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    id: &str,
    label: &str,
    value: &mut Option<f64>,
    enabled: bool,
) {
    ui.horizontal(|ui| {
        ui.add_enabled_ui(enabled, |ui| {
            let mut on = value.is_some();
            ui.add_sized(
                [LABEL_WIDTH + 10.0, 24.0],
                egui::Checkbox::new(&mut on, label),
            );
            match (on, *value) {
                (true, None) => *value = Some(0.0),
                (false, Some(_)) => *value = None,
                _ => {}
            }
            ui.add_enabled_ui(on, |ui| {
                let degrees = value.map_or(0.0, f64::to_degrees);
                let shown = format!("{}", (degrees * 100.0).round() / 100.0);
                widgets::dropdown(ui, theme, id, &shown, SMALL_WIDTH, |ui| {
                    for angle in ANGLES {
                        if ui
                            .selectable_label(degrees == angle, format!("{angle}"))
                            .clicked()
                        {
                            *value = Some(angle.to_radians());
                        }
                    }
                });
            });
        });
    });
    ui.add_space(4.0);
}

pub(super) fn leader_structure(ui: &mut egui::Ui, theme: &Theme, draft: &mut Draft) {
    let s = &mut draft.style;
    widgets::group_box(ui, theme, "Constraints", |ui| {
        ui.horizontal(|ui| {
            let mut limited = s.max_points.is_some();
            ui.add_sized(
                [LABEL_WIDTH + 10.0, 24.0],
                egui::Checkbox::new(&mut limited, "Maximum leader points"),
            );
            match (limited, s.max_points) {
                (true, None) => s.max_points = Some(2),
                (false, Some(_)) => s.max_points = None,
                _ => {}
            }
            if let Some(points) = &mut s.max_points {
                let mut shown = f64::from(*points);
                widgets::number_field_width(ui, theme, &mut shown, "", 0.2, SMALL_WIDTH);
                *points = shown.clamp(2.0, f64::from(MAX_POINTS)).round() as u32;
            }
        });
        ui.add_space(4.0);
        angle_row(
            ui,
            theme,
            "ml_first",
            "First segment angle",
            &mut s.first_angle,
            true,
        );
        angle_row(
            ui,
            theme,
            "ml_second",
            "Second segment angle",
            &mut s.second_angle,
            true,
        );
    });
    ui.add_space(8.0);
    widgets::group_box(ui, theme, "Landing settings", |ui| {
        ui.checkbox(&mut s.dogleg, "Automatically include landing");
        ui.add_space(4.0);
        ui.add_enabled_ui(s.dogleg, |ui| {
            row(ui, theme, "Landing distance:", |ui| {
                length(ui, theme, &mut s.dogleg_length)
            });
        });
    });
    ui.add_space(8.0);
    widgets::group_box(ui, theme, "Scale", |ui| {
        row(ui, theme, "Scale:", |ui| {
            widgets::number_field_width(ui, theme, &mut s.overall_scale, "", 0.05, SMALL_WIDTH);
            if !s.overall_scale.is_finite() || s.overall_scale <= 0.0 {
                s.overall_scale = 1.0;
            }
        });
    });
    ui.add_space(8.0);
    note(
        ui,
        theme,
        "Annotative styles and scaling multileaders to the layout are not kept by the style yet.",
    );
}

fn text_style_row(ui: &mut egui::Ui, theme: &Theme, draft: &mut Draft, doc: &Document) {
    let want = &mut draft.want_text_styles;
    let c = &mut draft.style.content;
    row(ui, theme, "Text style:", |ui| {
        let styles = doc.text_styles();
        let name = styles
            .iter()
            .find(|t| t.id == c.text_style)
            .map_or("Standard", |t| t.name.as_str());
        widgets::dropdown(ui, theme, "ml_text_style", name, FIELD_WIDTH - 36.0, |ui| {
            for style in styles {
                ui.selectable_value(&mut c.text_style, style.id, &style.name);
            }
        });
        if widgets::secondary_button(ui, theme, "...").clicked() {
            *want = true;
        }
    });
}

fn attachment_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    id: &str,
    label: &str,
    v: &mut LeaderAttachment,
) {
    let mut at = LeaderAttachment::ALL
        .iter()
        .position(|a| a == v)
        .unwrap_or(0);
    let labels = LeaderAttachment::ALL.map(LeaderAttachment::label);
    row(ui, theme, label, |ui| {
        choice(ui, theme, id, &labels, &mut at)
    });
    *v = LeaderAttachment::ALL[at];
}

pub(super) fn content(ui: &mut egui::Ui, theme: &Theme, draft: &mut Draft, doc: &Document) {
    {
        let c = &mut draft.style.content;
        let mut kind = MLeaderContentType::ALL
            .iter()
            .position(|k| *k == c.kind)
            .unwrap_or(0);
        let labels = MLeaderContentType::ALL.map(MLeaderContentType::label);
        row(ui, theme, "Multileader type:", |ui| {
            choice(ui, theme, "ml_content_type", &labels, &mut kind)
        });
        c.kind = MLeaderContentType::ALL[kind];
    }
    let has_text = draft.style.content.kind == MLeaderContentType::MText;
    ui.add_space(4.0);
    ui.add_enabled_ui(has_text, |ui| {
        widgets::group_box(ui, theme, "Text options", |ui| {
            row(ui, theme, "Default text:", |ui| {
                widgets::text_field(ui, theme, &mut draft.style.content.default_text, "");
            });
            text_style_row(ui, theme, draft, doc);
            let c = &mut draft.style.content;
            let mut angle = TextAngle::ALL
                .iter()
                .position(|a| *a == c.text_angle)
                .unwrap_or(0);
            let labels = TextAngle::ALL.map(TextAngle::label);
            row(ui, theme, "Text angle:", |ui| {
                choice(ui, theme, "ml_text_angle", &labels, &mut angle)
            });
            c.text_angle = TextAngle::ALL[angle];
            colour_row(ui, theme, "ml_text_color", "Text color:", &mut c.text_color);
            row(ui, theme, "Text height:", |ui| {
                widgets::number_field_width(ui, theme, &mut c.text_height, "", 0.05, SMALL_WIDTH);
                if !c.text_height.is_finite() || c.text_height <= 0.0 {
                    c.text_height = 1e-6;
                }
            });
            ui.checkbox(&mut c.always_left_justify, "Always left justify");
            ui.checkbox(&mut c.frame, "Frame text");
        });
    });
    ui.add_space(8.0);
    ui.add_enabled_ui(has_text, |ui| {
        let c = &mut draft.style.content;
        widgets::group_box(ui, theme, "Leader connection", |ui| {
            attachment_row(
                ui,
                theme,
                "ml_left",
                "Left attachment:",
                &mut c.left_attachment,
            );
            attachment_row(
                ui,
                theme,
                "ml_right",
                "Right attachment:",
                &mut c.right_attachment,
            );
            row(ui, theme, "Landing gap:", |ui| {
                length(ui, theme, &mut c.landing_gap)
            });
            ui.checkbox(&mut c.extend_to_text, "Extend leader to text");
        });
    });
    ui.add_space(8.0);
    note(
        ui,
        theme,
        "Block content and vertical attachment are not kept by the style yet.",
    );
}

/// The text the preview shows: the style's own, or a stand-in.
pub(super) fn preview_text(style: &MLeaderStyle) -> &str {
    if style.content.default_text.trim().is_empty() {
        "Default Text"
    } else {
        &style.content.default_text
    }
}
