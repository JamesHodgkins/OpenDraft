//! The Updates card on the Home page. It is always there and says where
//! things stand: not asked yet, off, up to date, or a newer version. It is the
//! one place the question is asked, and the same choice as Settings > Updates.

use std::time::Duration;

use eframe::egui;
use od_ui::widgets;

use super::Pick;
use crate::text::age_text;
use crate::updates::View;

/// "Checked 3 hours ago", or nothing before the first answer.
fn checked_text(checked: Option<u64>, now: u64) -> String {
    match checked {
        Some(t) => format!(
            "Checked {}.",
            age_text(Duration::from_secs(now.saturating_sub(t)))
        ),
        None => "Not checked yet.".to_owned(),
    }
}

pub(super) fn updates_section(
    ui: &mut egui::Ui,
    theme: &od_ui::Theme,
    view: &View,
    now: u64,
    pick: &mut Option<Pick>,
) {
    let c = &theme.colors;
    widgets::section_header(ui, theme, "Updates");
    egui::Frame::new()
        .fill(c.panel_raised)
        .stroke(egui::Stroke::new(1.0_f32, c.border))
        .corner_radius(egui::CornerRadius::same(theme.radius))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let weak = |ui: &mut egui::Ui, text: String| {
                ui.add(egui::Label::new(egui::RichText::new(text).color(c.text_weak)).wrap());
            };
            let strong = |ui: &mut egui::Ui, text: String| {
                ui.add(egui::Label::new(egui::RichText::new(text).strong().color(c.text)).wrap());
            };
            let version = crate::build_info::display();
            match view {
                View::Disabled => {
                    let why = if crate::updates::handled_by_the_store() {
                        "Flathub keeps OpenDraft up to date."
                    } else {
                        "Update checks are off in development builds."
                    };
                    weak(ui, why.into());
                }
                View::Unasked => {
                    strong(ui, "Check for new versions?".into());
                    weak(
                        ui,
                        "OpenDraft can ask GitHub once a day whether a newer version exists. \
                         It sends no drawings or personal details, and installs nothing. You can \
                         change this later in Settings."
                            .into(),
                    );
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        if widgets::secondary_button(ui, theme, "Check automatically").clicked() {
                            *pick = Some(Pick::UpdatesAllow(true));
                        }
                        if widgets::secondary_button(ui, theme, "Don't check").clicked() {
                            *pick = Some(Pick::UpdatesAllow(false));
                        }
                    });
                }
                View::Off => {
                    strong(ui, "Automatic update checks are off.".into());
                    weak(ui, format!("You are running OpenDraft {version}."));
                    ui.add_space(8.0);
                    if widgets::secondary_button(ui, theme, "Turn on").clicked() {
                        *pick = Some(Pick::UpdatesAllow(true));
                    }
                }
                View::Checking => {
                    strong(ui, "Checking for a new version\u{2026}".into());
                    weak(ui, format!("You are running OpenDraft {version}."));
                }
                View::Current { checked } => {
                    strong(ui, format!("OpenDraft {version} is up to date."));
                    weak(ui, checked_text(*checked, now));
                    ui.add_space(8.0);
                    check_buttons(ui, theme, pick);
                }
                View::Offline { checked } => {
                    strong(ui, "Could not check for a new version.".into());
                    weak(
                        ui,
                        format!("{} It will try again later.", checked_text(*checked, now)),
                    );
                    ui.add_space(8.0);
                    check_buttons(ui, theme, pick);
                }
                View::Available { version: newer } => {
                    strong(ui, format!("Version {newer} is available."));
                    weak(ui, format!("You are running OpenDraft {version}."));
                    ui.add_space(8.0);
                    if widgets::primary_button(ui, theme, "View changes").clicked() {
                        *pick = Some(Pick::UpdatesView(newer.clone()));
                    }
                    ui.add_space(4.0);
                    // Their own row: three buttons are wider than the side column.
                    ui.horizontal_wrapped(|ui| {
                        if widgets::secondary_button(ui, theme, "Remind me later").clicked() {
                            *pick = Some(Pick::UpdatesRemind);
                        }
                        if widgets::secondary_button(ui, theme, "Skip this version").clicked() {
                            *pick = Some(Pick::UpdatesSkip(newer.clone()));
                        }
                    });
                }
            }
        });
}

fn check_buttons(ui: &mut egui::Ui, theme: &od_ui::Theme, pick: &mut Option<Pick>) {
    ui.horizontal_wrapped(|ui| {
        if widgets::secondary_button(ui, theme, "Check now").clicked() {
            *pick = Some(Pick::UpdatesCheckNow);
        }
        if widgets::secondary_button(ui, theme, "Turn off").clicked() {
            *pick = Some(Pick::UpdatesAllow(false));
        }
    });
}
