//! The Model and layout tabs under the drawing, as in AutoCAD. Reports what
//! was clicked; the application state switches spaces.

use eframe::egui::{RichText, Ui};
use od_canvas::Space;
use od_core::{Document, LayoutId};
use od_ui::Theme;

/// What the user asked for with the tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabAction {
    Model,
    Layout(LayoutId),
    New,
    Copy(LayoutId),
    Delete(LayoutId),
}

pub fn show(ui: &mut Ui, theme: &Theme, document: &Document, space: Space) -> Option<TabAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let tab = |ui: &mut Ui, text: &str, active: bool| {
            let color = if active {
                theme.colors.text
            } else {
                theme.colors.text_weak
            };
            ui.selectable_label(active, RichText::new(text).color(color))
        };
        if tab(ui, "Model", space == Space::Model).clicked() {
            action = Some(TabAction::Model);
        }
        for layout in document.layouts() {
            let response = tab(ui, &layout.name, space.layout() == Some(layout.id));
            if response.clicked() {
                action = Some(TabAction::Layout(layout.id));
            }
            response.context_menu(|ui| {
                for (label, act) in [
                    ("New layout", TabAction::New),
                    ("Copy", TabAction::Copy(layout.id)),
                    ("Delete", TabAction::Delete(layout.id)),
                ] {
                    if ui.button(label).clicked() {
                        action = Some(act);
                        ui.close_menu();
                    }
                }
            });
        }
        if ui.small_button("+").on_hover_text("New layout").clicked() {
            action = Some(TabAction::New);
        }
    });
    action
}

/// Do what a tab asked for.
pub fn apply(state: &mut crate::state::AppState, action: TabAction) {
    match action {
        TabAction::Model => state.enter_model(),
        TabAction::Layout(id) => state.enter_layout(id),
        TabAction::New => state.new_layout(),
        TabAction::Copy(id) => state.duplicate_layout(id),
        TabAction::Delete(id) => state.delete_layout(id),
    }
}
