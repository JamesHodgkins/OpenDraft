//! The menu that Shift+right-click (or Ctrl+right-click) opens at a point
//! prompt: one-shot object snaps for the next point only.

use eframe::egui;

use super::OpenDraftApp;
use crate::snap_once::parse;

/// Menu rows: a label and the word a user would type, or `None` for a rule.
const ROWS: &[Option<(&str, &str)>] = &[
    Some(("Temporary Track Point", "tt")),
    Some(("Tracking", "tk")),
    Some(("From", "fro")),
    Some(("Mid Between 2 Points", "m2p")),
    None,
    Some(("Endpoint", "end")),
    Some(("Midpoint", "mid")),
    Some(("Intersection", "int")),
    Some(("Apparent Intersection", "app")),
    Some(("Extended Intersection", "extint")),
    Some(("Geometric Center", "gcen")),
    Some(("Center", "cen")),
    Some(("Quadrant", "qua")),
    Some(("Tangent", "tan")),
    Some(("Perpendicular", "per")),
    Some(("Extension", "ext")),
    Some(("Parallel", "par")),
    Some(("Node", "nod")),
    Some(("Insertion", "ins")),
    Some(("Nearest", "nea")),
    None,
    Some(("None", "non")),
];

impl OpenDraftApp {
    /// Show the override menu where it was asked for, and start the snap that
    /// is picked. It closes on a pick, a click elsewhere, or when no point is
    /// being asked for any more.
    pub(super) fn snap_menu(&mut self, ctx: &egui::Context) {
        let Some((x, y)) = self.state.snap_menu else {
            return;
        };
        if self.state.waiting().is_none() {
            self.state.snap_menu = None;
            return;
        }
        let mut word = None;
        let mut settings = false;
        let area = egui::Area::new(egui::Id::new("snap_menu"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(x, y))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(190.0);
                    // Justified: each row takes the whole width, so a press
                    // anywhere along it picks it.
                    ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                        for row in ROWS {
                            match row {
                                None => {
                                    ui.separator();
                                }
                                Some((label, typed)) => {
                                    if ui.selectable_label(false, *label).clicked() {
                                        word = Some(*typed);
                                    }
                                }
                            }
                        }
                    });
                    ui.separator();
                    settings = ui
                        .with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                            ui.selectable_label(false, "Snap Settings...").clicked()
                        })
                        .inner;
                });
            });
        let outside = ctx.input(|i| i.pointer.primary_clicked())
            && ctx
                .pointer_latest_pos()
                .is_some_and(|p| !area.response.rect.contains(p));
        if let Some(wish) = word.and_then(parse) {
            self.state.begin_one_shot(wish);
        }
        if settings {
            self.settings_tab = 1;
            self.state.show_settings = true;
        }
        if word.is_some() || settings || outside {
            self.state.snap_menu = None;
        }
    }
}
