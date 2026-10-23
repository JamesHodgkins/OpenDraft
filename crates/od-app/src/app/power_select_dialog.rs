//! The Power Select dialog (POWERSELECT, also QSELECT): choose objects by
//! kind and one property. The search itself is `power_select.rs`.

use std::collections::BTreeSet;

use eframe::egui;
use od_core::{EntityId, EntityKind};
use od_ui::{icons, widgets};

use super::OpenDraftApp;
use crate::power_select::{self as ps, Operator, Prop, Scope};
use crate::text::plural;

const WIDTH: f32 = 400.0;
const COMBO: f32 = 190.0;

fn scope_name(scope: Scope) -> &'static str {
    match scope {
        Scope::Drawing => "Entire drawing",
        Scope::Selection => "Current selection",
    }
}

impl OpenDraftApp {
    /// The window opened by `select.power`.
    pub(super) fn power_select_window(&mut self, ctx: &egui::Context) {
        if self.state.power_select.picking {
            // Back from picking the objects to search.
            if !self.state.command_running() {
                self.state.power_select.picking = false;
                self.state.power_select.open = true;
                if !self.state.selection().is_empty() {
                    self.state.power_select.draft.scope = Scope::Selection;
                }
            }
            return;
        }
        if !self.state.power_select.open {
            return;
        }
        let units = self.state.units();
        let selection: BTreeSet<EntityId> = self.state.selection().clone();
        let mut dialog = std::mem::take(&mut self.state.power_select);
        let document = self.state.session().document();
        let draft = &mut dialog.draft;
        if selection.is_empty() {
            draft.scope = Scope::Drawing;
        }
        let kinds = ps::kinds_in(document, draft.scope, &selection);
        if draft.kind.is_some_and(|k| !kinds.contains(&k)) {
            draft.kind = None;
        }
        let props = ps::props_for(document, draft.scope, &selection, draft.kind);
        if !props.iter().any(|(p, _)| *p == draft.prop) {
            draft.prop = props.first().map_or(Prop::Layer, |(p, _)| *p);
        }
        if !draft.prop.operators().contains(&draft.operator) {
            draft.operator = Operator::Equals;
        }
        let values = ps::values_present(document, &units, draft, &selection);
        let now = (draft.scope, draft.kind, draft.prop);
        if dialog.shown != Some(now) {
            dialog.shown = Some(now);
            draft.value = values.first().cloned().unwrap_or_default();
        }
        let usable = ps::value_is_usable(&units, draft);
        let (mut open, mut ok, mut pick, mut cancel) = (true, false, false, false);
        let theme = &self.theme;
        widgets::dialog(ctx, theme, "Power Select", &mut open, WIDTH, |ui| {
            egui::Grid::new("power_select")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Apply to:");
                    ui.horizontal(|ui| {
                        egui::ComboBox::from_id_salt("ps_scope")
                            .width(COMBO)
                            .selected_text(scope_name(draft.scope))
                            .show_ui(ui, |ui| {
                                let drawing = Scope::Drawing;
                                ui.selectable_value(&mut draft.scope, drawing, scope_name(drawing));
                                ui.add_enabled_ui(!selection.is_empty(), |ui| {
                                    let picked = Scope::Selection;
                                    ui.selectable_value(
                                        &mut draft.scope,
                                        picked,
                                        scope_name(picked),
                                    )
                                });
                            });
                        pick = widgets::icon_button(
                            ui,
                            theme,
                            icons::POWER_SELECT_PICK,
                            "Select objects",
                        )
                        .clicked();
                    });
                    ui.end_row();

                    ui.label("Object type:");
                    egui::ComboBox::from_id_salt("ps_kind")
                        .width(COMBO)
                        .selected_text(draft.kind.map_or("Multiple", EntityKind::title))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut draft.kind, None, "Multiple");
                            for kind in &kinds {
                                ui.selectable_value(&mut draft.kind, Some(*kind), kind.title());
                            }
                        });
                    ui.end_row();

                    ui.label("Properties:");
                    egui::Frame::new()
                        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .max_height(150.0)
                                .min_scrolled_height(150.0)
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    ui.set_width(250.0);
                                    let layout =
                                        egui::Layout::top_down_justified(egui::Align::LEFT);
                                    ui.with_layout(layout, |ui| {
                                        for (prop, label) in &props {
                                            ui.selectable_value(&mut draft.prop, *prop, label);
                                        }
                                    });
                                });
                        });
                    ui.end_row();

                    ui.label("Operator:");
                    egui::ComboBox::from_id_salt("ps_operator")
                        .width(COMBO)
                        .selected_text(draft.operator.label())
                        .show_ui(ui, |ui| {
                            for op in draft.prop.operators() {
                                ui.selectable_value(&mut draft.operator, *op, op.label());
                            }
                        });
                    ui.end_row();

                    ui.label("Value:");
                    ui.add_enabled_ui(draft.operator != Operator::All, |ui| {
                        if draft.prop.typed() || draft.operator == Operator::Wildcard {
                            let edit = egui::TextEdit::singleline(&mut draft.value);
                            ui.add(edit.desired_width(COMBO));
                        } else {
                            egui::ComboBox::from_id_salt("ps_value")
                                .width(COMBO)
                                .selected_text(draft.value.clone())
                                .show_ui(ui, |ui| {
                                    for v in &values {
                                        ui.selectable_value(&mut draft.value, v.clone(), v);
                                    }
                                });
                        }
                    });
                    ui.end_row();
                });
            ui.add_space(8.0);
            widgets::group_box(ui, theme, "How to apply:", |ui| {
                ui.radio_value(&mut draft.include, true, "Include in new selection set");
                ui.radio_value(&mut draft.include, false, "Exclude from new selection set");
            });
            ui.add_space(6.0);
            ui.checkbox(&mut draft.append, "Append to current selection set");
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.add_enabled_ui(usable, |ui| {
                    ok = widgets::primary_button(ui, theme, "OK").clicked();
                });
                if widgets::secondary_button(ui, theme, "Cancel").clicked() {
                    cancel = true;
                }
            });
        });
        open &= !cancel;
        if ok {
            let document = self.state.session().document();
            let mut found: BTreeSet<EntityId> =
                ps::run(document, &units, &dialog.draft, &selection)
                    .into_iter()
                    .collect();
            if dialog.draft.append {
                found.extend(selection.iter().copied());
            }
            let count = plural(found.len(), "object");
            self.state.docs[self.state.active].selection = found;
            self.state.message = format!("Power Select: {count} selected");
            self.state.cmdline.say(self.state.message.clone());
            open = false;
        }
        if pick {
            self.state.docs[self.state.active].selection.clear();
            self.state.start_command("QSELECT", |ed| {
                let ids = ed.get_selection("Select objects")?;
                ed.select(ids);
                Ok(())
            });
            dialog.picking = true;
        } else {
            dialog.open = open;
        }
        self.state.power_select = dialog;
    }
}
