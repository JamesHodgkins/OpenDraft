//! The Settings dialog window and applying what it edits.

use eframe::egui;
use od_ui::widgets;
use od_ui::Theme;

use super::OpenDraftApp;
use crate::locations::Location;
use crate::settings::Settings;
use crate::settings_ui::{self, FolderRequest};

#[derive(Clone, Copy)]
enum DialogAction {
    Ok,
    Apply,
    Cancel,
}

impl OpenDraftApp {
    /// The dialogs and managers that open over the drawing, each of which
    /// shows itself only while its flag is set.
    pub(super) fn tool_windows(&mut self, ctx: &egui::Context) {
        self.settings_window(ctx);
        self.units_window(ctx);
        self.point_style_window(ctx);
        self.power_select_window(ctx);
        self.linetype_window(ctx);
        self.text_style_window(ctx);
        self.dim_manager_window(ctx);
        self.dim_style_window(ctx);
        self.mleader_windows(ctx);
        self.page_setup_window(ctx);
    }

    /// Settings dialog, opened by `manage.settings`. Edits a draft copy;
    /// OK / Apply commit it, Cancel / Escape / X discard it.
    pub(super) fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.state.show_settings {
            self.settings_draft = None;
            return;
        }
        let mut draft = self
            .settings_draft
            .take()
            .unwrap_or_else(|| self.state.settings.clone());
        let mut open = true;
        let mut tab = self.settings_tab;
        let mut action = None;
        let dirty = draft != self.state.settings;
        let document = self.state.docs[self.state.active].title.clone();
        let mut connections: Vec<crate::automation::ConnectionRow> = self
            .automation
            .as_ref()
            .map(|host| {
                host.grants()
                    .into_iter()
                    .map(|(id, g)| crate::automation::ConnectionRow {
                        id,
                        client: g.client_name,
                        write: g.scopes.contains(&od_api::Scope::DrawingWrite),
                        commands: g.scopes.contains(&od_api::Scope::CommandInvoke),
                        files: g.scopes.contains(&od_api::Scope::FileWrite),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let shown: Vec<(bool, bool, bool)> = connections
            .iter()
            .map(|c| (c.write, c.commands, c.files))
            .collect();
        let mut request = None;
        let screen = ctx.screen_rect();
        // Fixed, so the dialog does not change size between tabs: tall enough
        // for Display and Drafting, the longest pages, and smaller on short
        // screens.
        let page_height = (screen.height() - 230.0).clamp(100.0, 400.0);
        let width = (screen.width() - 80.0).clamp(360.0, 880.0);

        widgets::dialog(ctx, &self.theme, "Settings", &mut open, width, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Current drawing:").color(self.theme.colors.text_weak),
                );
                ui.label(document);
            });
            ui.add_space(8.0);
            widgets::tabbed_page(
                ui,
                &self.theme,
                &mut tab,
                settings_ui::TABS,
                page_height,
                |ui, i| {
                    egui::ScrollArea::vertical()
                        .max_height(page_height)
                        .show(ui, |ui| {
                            settings_ui::page(
                                ui,
                                &self.theme,
                                i,
                                &mut draft,
                                &mut connections,
                                &mut request,
                            );
                        });
                },
            );
            ui.add_space(12.0);
            // Wrapped in horizontal so the right-to-left layout is one row
            // tall instead of stretching the dialog.
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_enabled_ui(dirty, |ui| {
                        if widgets::secondary_button(ui, &self.theme, "Apply").clicked() {
                            action = Some(DialogAction::Apply);
                        }
                    });
                    if widgets::secondary_button(ui, &self.theme, "Cancel").clicked() {
                        action = Some(DialogAction::Cancel);
                    }
                    if widgets::primary_button(ui, &self.theme, "OK").clicked() {
                        action = Some(DialogAction::Ok);
                    }
                });
            });
        });

        self.settings_tab = tab;
        // The write switch is the user's, and takes effect at once.
        if let Some(host) = &self.automation {
            for (row, was) in connections.iter().zip(&shown) {
                if row.write != was.0 {
                    host.set_write(row.id, row.write);
                }
                if row.commands != was.1 {
                    host.set_commands(row.id, row.commands);
                }
                if row.files != was.2 {
                    host.set_files(row.id, row.files);
                }
            }
        }
        if let Some(request) = request {
            match request {
                FolderRequest::Approve => draft.approve_folder(&mut *self.state.picker),
                FolderRequest::Choose(location) => {
                    draft.choose_folder(location, &mut *self.state.picker);
                }
            }
            // The list above was drawn before the folder came back.
            ctx.request_repaint();
        }
        if !open {
            action = Some(DialogAction::Cancel);
        }
        match action {
            Some(DialogAction::Apply) => {
                self.apply_settings(ctx, draft.sanitized());
                self.settings_draft = Some(self.state.settings.clone());
            }
            Some(DialogAction::Ok) => {
                self.apply_settings(ctx, draft.sanitized());
                self.state.show_settings = false;
                self.settings_draft = None;
            }
            Some(DialogAction::Cancel) => {
                self.state.show_settings = false;
                self.settings_draft = None;
            }
            None => self.settings_draft = Some(draft),
        }
    }

    /// Make `new` the active settings, updating the theme and UI scale if
    /// they changed. Saved with the rest of the app state on exit.
    fn apply_settings(&mut self, ctx: &egui::Context, new: Settings) {
        if new.palette != self.state.settings.palette {
            self.theme = Theme::with_palette(new.palette);
            self.theme.apply(ctx);
        }
        if new.ui_scale_percent != self.state.settings.ui_scale_percent {
            ctx.set_zoom_factor(new.zoom_factor());
        }
        self.state.templates_dir = Location::Templates.resolve(&new);
        self.state.settings = new;
    }
}
