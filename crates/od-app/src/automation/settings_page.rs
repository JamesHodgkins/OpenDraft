//! The Automation page of the Settings dialog.

use eframe::egui;
use od_ui::{widgets, Theme};

use crate::settings::Settings;

/// One open connection as the page shows it.
pub struct ConnectionRow {
    pub id: u64,
    pub client: String,
    pub write: bool,
    /// May run the commands marked safe (zoom, grid and snap, select all)
    /// and start commands for the user to finish.
    pub commands: bool,
    /// May open, save and export files in the approved folders.
    pub files: bool,
}

pub fn page(
    ui: &mut egui::Ui,
    theme: &Theme,
    draft: &mut Settings,
    connections: &mut [ConnectionRow],
    request: &mut Option<crate::settings_ui::FolderRequest>,
) {
    widgets::group_box(ui, theme, "MCP server (experimental)", |ui| {
        ui.checkbox(
            &mut draft.automation_enabled,
            "Let AI assistants work with the open drawings",
        );
        ui.label(
            egui::RichText::new(
                "Starts a local, per-user endpoint that the opendraft-mcp program connects to. \
                 A connection can only read until you tick Allow changes for it below; \
                 each change it makes is one undo step. Turn this off to disconnect everything. \
                 (Setting OD_AUTOMATION=1 turns it on for one run without saving.)",
            )
            .size(theme.font_size_small)
            .color(theme.colors.text_weak),
        );
    });
    ui.add_space(6.0);
    widgets::group_box(
        ui,
        theme,
        "Connect an assistant",
        |ui| match super::setup::sidecar_path() {
            Some(program) => {
                ui.label(
                    egui::RichText::new(format!("Program: {}", program.display()))
                        .size(theme.font_size_small),
                );
                if widgets::secondary_button(ui, theme, "Copy MCP config").clicked() {
                    ui.ctx().copy_text(super::setup::config_snippet(&program));
                }
                ui.label(
                    egui::RichText::new(
                        "Paste it into your assistant's MCP settings (see docs/mcp.md), then restart the assistant.",
                    )
                    .size(theme.font_size_small)
                    .color(theme.colors.text_weak),
                );
            }
            None => {
                ui.label(
                    egui::RichText::new(
                        "The opendraft-mcp program is not next to this one. Build it with cargo build --release -p od-mcp, or use a release that includes it.",
                    )
                    .size(theme.font_size_small)
                    .color(theme.colors.text_weak),
                );
            }
        },
    );
    ui.add_space(6.0);
    widgets::group_box(ui, theme, "Connections", |ui| {
        if connections.is_empty() {
            ui.label(
                egui::RichText::new("None")
                    .size(theme.font_size_small)
                    .color(theme.colors.text_weak),
            );
        }
        for row in connections.iter_mut() {
            ui.horizontal(|ui| {
                ui.label(&row.client);
                ui.checkbox(&mut row.write, "Allow changes");
                ui.checkbox(&mut row.commands, "Allow commands");
                ui.checkbox(&mut row.files, "Allow files");
            });
        }
    });
    ui.add_space(6.0);
    widgets::group_box(ui, theme, "Approved folders", |ui| {
        ui.label(
            egui::RichText::new(
                "A connection with Allow files can open and write files only inside these folders. Replacing an existing file asks you first.",
            )
            .size(theme.font_size_small)
            .color(theme.colors.text_weak),
        );
        let mut remove = None;
        for (i, root) in draft.automation_roots.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(root.display().to_string());
                if ui.small_button("Remove").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            draft.automation_roots.remove(i);
        }
        if widgets::secondary_button(ui, theme, "Add folder...").clicked() {
            *request = Some(crate::settings_ui::FolderRequest::Approve);
        }
    });
}
