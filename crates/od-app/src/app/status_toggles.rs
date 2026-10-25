//! The mode buttons at the right of the status bar, drawn as icons in
//! AutoCAD's order, with the object snap menu behind a right-click.

use eframe::egui;
use od_ui::icon_set::{self, Icon};
use od_ui::icons;

use super::OpenDraftApp;

const ICON_SIZE: f32 = 17.0;
const ROW_HEIGHT: f32 = 22.0;

/// Which button a toggle is, for the menus that hang off two of them.
#[derive(Clone, Copy, PartialEq)]
enum Menu {
    None,
    ObjectSnap,
}

/// Status bar buttons, left to right: icon, command id, tooltip, menu.
const MODE_TOGGLES: &[(char, &str, &str, Menu)] = &[
    (
        icons::STATUS_GRID_DISPLAY,
        "view.grid",
        "Grid display (F7)",
        Menu::None,
    ),
    (
        icons::STATUS_GRID_SNAP,
        "view.snap_grid",
        "Snap to grid",
        Menu::None,
    ),
    (
        icons::STATUS_ORTHO,
        "view.ortho",
        "Ortho mode (F8)",
        Menu::None,
    ),
    (
        icons::STATUS_POLAR,
        "view.polar",
        "Polar tracking (F10)",
        Menu::None,
    ),
    (
        icons::STATUS_OSNAP,
        "view.snap",
        "Object snap (F3); right-click for the modes",
        Menu::ObjectSnap,
    ),
    (
        icons::STATUS_OTRACK,
        "view.otrack",
        "Object snap tracking (F11)",
        Menu::None,
    ),
];

/// The running object snaps in the object snap menu, in AutoCAD's order.
const SNAP_MENU: &[(char, &str, &str)] = &[
    (icons::OSNAP_END, "Endpoint", "view.snap_endpoint"),
    (icons::OSNAP_MID, "Midpoint", "view.snap_midpoint"),
    (icons::OSNAP_CENTRE, "Center", "view.snap_center"),
    (
        icons::OSNAP_GEO_CENTRE,
        "Geometric Center",
        "view.snap_geometric_center",
    ),
    (icons::OSNAP_NODE, "Node", "view.snap_node"),
    (icons::OSNAP_QUADRANT, "Quadrant", "view.snap_quadrant"),
    (
        icons::OSNAP_INTERSECTION,
        "Intersection",
        "view.snap_intersection",
    ),
    (icons::OSNAP_EXTENSION, "Extension", "view.snap_extension"),
    (icons::OSNAP_INSERTION, "Insertion", "view.snap_insertion"),
    (
        icons::OSNAP_PERPENDICULAR,
        "Perpendicular",
        "view.snap_perpendicular",
    ),
    (icons::OSNAP_TANGENT, "Tangent", "view.snap_tangent"),
    (icons::OSNAP_NEAREST, "Nearest", "view.snap_nearest"),
    (
        icons::OSNAP_APPARENT_INTERSECTION,
        "Apparent Intersection",
        "view.snap_apparent_intersection",
    ),
    (icons::OSNAP_PARALLEL, "Parallel", "view.snap_parallel"),
];

/// One menu row: a tick when on, the snap's icon and its name. Returns
/// whether it was clicked.
fn snap_row(
    ui: &mut egui::Ui,
    theme: &od_ui::Theme,
    icon: char,
    label: &str,
    checked: bool,
) -> bool {
    let c = &theme.colors;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), ROW_HEIGHT),
        egui::Sense::click(),
    );
    if response.hovered() {
        ui.painter().rect_filled(rect, 2.0, c.hover);
    }
    if checked {
        let m = rect.left_center() + egui::vec2(10.0, 0.0);
        let stroke = egui::Stroke::new(1.6_f32, c.text);
        ui.painter().line_segment(
            [m + egui::vec2(-3.5, 0.5), m + egui::vec2(-1.0, 3.5)],
            stroke,
        );
        ui.painter().line_segment(
            [m + egui::vec2(-1.0, 3.5), m + egui::vec2(4.0, -3.5)],
            stroke,
        );
    }
    icon_set::paint(
        ui.painter(),
        rect.left_center() + egui::vec2(24.0, 0.0),
        egui::Align2::LEFT_CENTER,
        icon,
        16.0,
        c.text,
    );
    ui.painter().text(
        rect.left_center() + egui::vec2(48.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(theme.font_size_body),
        c.text,
    );
    response.clicked()
}

impl OpenDraftApp {
    /// Mode buttons, lit when on (like AutoCAD's status bar); dimmed while
    /// the command is disabled. Returns the command clicked. The layout adds
    /// right to left, so the table is walked in reverse.
    pub(super) fn mode_toggles(&mut self, ui: &mut egui::Ui) -> Option<&'static str> {
        let mut clicked = None;
        ui.spacing_mut().item_spacing.x = 2.0;
        self.settings_button(ui, &mut clicked);
        for &(icon, command, tip, menu) in MODE_TOGGLES.iter().rev() {
            let colors = &self.theme.colors;
            let on = self.registry.is_checked(command, &self.state);
            let color = if !self.registry.is_enabled(command, &self.state) {
                colors.text_disabled
            } else if on {
                colors.text
            } else {
                colors.text_weak
            };
            // Lit in neutral grey, not the theme's accent, which is green.
            let lit = ui.painter().add(egui::Shape::Noop);
            let response = ui
                .add(
                    Icon::new(icon, ICON_SIZE)
                        .clickable()
                        .color(color)
                        .padding(4.0),
                )
                .on_hover_text(tip)
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if on || response.hovered() {
                let fill = if on { colors.pressed } else { colors.hover };
                ui.painter().set(
                    lit,
                    egui::epaint::RectShape::filled(response.rect, 3.0, fill),
                );
            }
            if response.clicked() {
                clicked = Some(command);
            }
            if menu == Menu::ObjectSnap {
                self.snap_mode_menu(ui, &response, &mut clicked);
            }
        }
        clicked
    }

    /// The object snap modes, opened by a right-click on the button and
    /// shown above it, as the status bar is at the bottom of the window.
    fn snap_mode_menu(
        &mut self,
        ui: &egui::Ui,
        anchor: &egui::Response,
        clicked: &mut Option<&'static str>,
    ) {
        let id = egui::Id::new("snap_mode_menu");
        let mut open = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
        if anchor.secondary_clicked() {
            open = !open;
        }
        if open {
            let mut done = false;
            let area = egui::Area::new(id.with("area"))
                .order(egui::Order::Foreground)
                .pivot(egui::Align2::RIGHT_BOTTOM)
                .fixed_pos(anchor.rect.right_top() - egui::vec2(0.0, 4.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style())
                        .fill(self.theme.colors.panel_raised)
                        .inner_margin(egui::Margin::same(2))
                        .show(ui, |ui| {
                            ui.set_width(220.0);
                            ui.spacing_mut().item_spacing.y = 0.0;
                            for &(icon, label, mode) in SNAP_MENU {
                                let on = self.registry.is_checked(mode, &self.state);
                                if snap_row(ui, &self.theme, icon, label, on) {
                                    *clicked = Some(mode);
                                    done = true;
                                }
                            }
                            ui.separator();
                            if ui.button("Object Snap Settings...").clicked() {
                                self.settings_tab = 1;
                                self.state.show_settings = true;
                                done = true;
                            }
                        });
                });
            let outside = ui.input(|i| {
                i.pointer.any_pressed()
                    && i.pointer.interact_pos().is_some_and(|p| {
                        !area.response.rect.contains(p) && !anchor.rect.contains(p)
                    })
            });
            if done || outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                open = false;
            }
        }
        ui.data_mut(|d| d.insert_temp(id, open));
    }

    /// The gear at the far right: the drafting settings and the Dynamic
    /// Input switch.
    fn settings_button(&mut self, ui: &mut egui::Ui, clicked: &mut Option<&'static str>) {
        let color = self.theme.colors.text_weak;
        let response = ui
            .add(
                Icon::new(icons::STATUS_SETTINGS, ICON_SIZE)
                    .clickable()
                    .color(color)
                    .padding(4.0),
            )
            .on_hover_text("Status bar settings")
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let dynamic = self.registry.is_checked("view.dynamic_input", &self.state);
        let pick = od_ui::menu::popup(
            ui,
            &self.theme,
            egui::Id::new("status_settings_menu"),
            &response,
            200.0,
            |ui| {
                let label = if dynamic {
                    "Dynamic Input (F12): on"
                } else {
                    "Dynamic Input (F12): off"
                };
                if od_ui::menu::menu_row(ui, &self.theme, label, false) {
                    return Some("view.dynamic_input");
                }
                od_ui::menu::menu_row(ui, &self.theme, "Drafting Settings...", false).then_some("")
            },
        );
        match pick {
            Some("") => {
                self.settings_tab = 1;
                self.state.show_settings = true;
            }
            Some(id) => *clicked = Some(id),
            None => {}
        }
    }
}
