//! What a recent drawing offers besides opening: Pin, More and the menu
//! (right-click opens the same one). Shared by the list rows and the cards.

use eframe::egui;
use od_ui::{icon_set, menu};

use super::Pick;
use crate::home_scan::{FileState, RecentEntry};
use crate::recent::MAX_PINNED;

/// Width and height of the Pin and More buttons at the end of a row.
pub(super) const BUTTON: f32 = 28.0;
const BUTTON_ICON: f32 = 16.0;
const MENU_WIDTH: f32 = 170.0;

/// The Pin and More buttons (More in `more_rect`, Pin to its left) and the
/// menu, for the drawing whose card or row is `response`. Call it after that
/// is drawn, so Tab visits the drawing, then Pin, then More.
pub(super) fn entry_controls(
    ui: &mut egui::Ui,
    theme: &od_ui::Theme,
    entry: &RecentEntry,
    pinned_full: bool,
    response: &egui::Response,
    more_rect: egui::Rect,
    pick: &mut Option<Pick>,
) {
    // Right-click opens the same menu as More, at the pointer.
    let at = response
        .interact_pointer_pos()
        .unwrap_or_else(|| response.rect.center());
    let menu = menu::popup_at(
        ui,
        theme,
        ui.id().with("context"),
        response.secondary_clicked(),
        egui::Rect::from_min_size(at, egui::Vec2::ZERO),
        MENU_WIDTH,
        |ui| menu_items(ui, theme, entry, pinned_full),
    );
    if menu.is_some() {
        *pick = menu;
    }
    let pin_rect = more_rect.translate(egui::vec2(-BUTTON, 0.0));
    let pin_disabled = !entry.pinned && pinned_full;
    let pin = icon_button(ui, theme, pin_rect, od_ui::icons::HOME_PIN, entry.pinned, {
        !pin_disabled
    });
    let pin = if pin_disabled {
        pin.on_hover_text(format!("You can pin up to {MAX_PINNED} drawings"))
    } else {
        pin.on_hover_text(if entry.pinned { "Unpin" } else { "Pin" })
    };
    if pin.clicked() {
        *pick = Some(if entry.pinned {
            Pick::Unpin(entry.id)
        } else {
            Pick::Pin(entry.id)
        });
    }
    let more = icon_button(ui, theme, more_rect, od_ui::icons::HOME_MORE, false, true)
        .on_hover_text("More actions");
    let menu = menu::popup(ui, theme, ui.id().with("more"), &more, MENU_WIDTH, |ui| {
        menu_items(ui, theme, entry, pinned_full)
    });
    if menu.is_some() {
        *pick = menu;
    }
}

/// A small icon button in `rect`, lit while `on`. Its id comes from `icon`,
/// not from where it was added, so focus stays with it when the list moves.
fn icon_button(
    ui: &mut egui::Ui,
    theme: &od_ui::Theme,
    rect: egui::Rect,
    icon: char,
    on: bool,
    enabled: bool,
) -> egui::Response {
    let c = &theme.colors;
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let response = ui.interact(rect, ui.id().with(icon), sense);
    if enabled && (response.hovered() || on) {
        ui.painter().rect_filled(rect, 4.0, c.hover);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(2.0_f32, c.accent),
            egui::StrokeKind::Inside,
        );
    }
    let colour = match (enabled, on) {
        (false, _) => c.text_disabled,
        (true, true) => c.text,
        (true, false) => c.text_weak,
    };
    icon_set::paint(
        ui.painter(),
        rect.center(),
        egui::Align2::CENTER_CENTER,
        icon,
        BUTTON_ICON,
        colour,
    );
    response
}

/// The things that can be done to a recent drawing, as the app's other menus
/// draw them. Copy path is done here; the rest is for the caller to carry out.
fn menu_items(
    ui: &mut egui::Ui,
    theme: &od_ui::Theme,
    entry: &RecentEntry,
    pinned_full: bool,
) -> Option<Pick> {
    let mut chosen = None;
    if menu::menu_row(ui, theme, "Open", false) {
        chosen = Some(Pick::Recent(entry.id));
    }
    if entry.pinned {
        if menu::menu_row(ui, theme, "Unpin", false) {
            chosen = Some(Pick::Unpin(entry.id));
        }
    } else {
        let label = if pinned_full {
            format!("Pin (up to {MAX_PINNED} drawings)")
        } else {
            "Pin".to_owned()
        };
        let clicked = ui
            .add_enabled_ui(!pinned_full, |ui| menu::menu_row(ui, theme, &label, false))
            .inner;
        if clicked {
            chosen = Some(Pick::Pin(entry.id));
        }
    }
    if menu::menu_row(ui, theme, "Copy path", false) {
        ui.ctx().copy_text(entry.path.display().to_string());
        // Done already; this closes the menu.
        chosen = Some(Pick::Copied);
    }
    if entry.path.parent().is_some_and(std::path::Path::is_dir)
        && menu::menu_row(ui, theme, "Show in folder", false)
    {
        chosen = Some(Pick::ShowInFolder(entry.path.clone()));
    }
    if entry.state == FileState::Missing && menu::menu_row(ui, theme, "Locate...", false) {
        chosen = Some(Pick::Locate(entry.id));
    }
    ui.separator();
    if menu::menu_row(ui, theme, "Remove from list", false) {
        chosen = Some(Pick::Forget(entry.id));
    }
    chosen
}
