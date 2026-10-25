//! The fixed chrome around the dock: title bar, ribbon, document tabs and
//! status bar.

use eframe::egui;
use od_core::units::format_length;
use od_ui::menu;
use od_ui::widgets;
use od_ui::{document_tabs, DocumentTabEvent};

use super::update_notice::update_notice;
use super::{OpenDraftApp, Source, QUICK_ACCESS};
use crate::text::plural;

const READOUT_WIDTH: f32 = 165.0;

/// What was picked in the application menu.
enum MenuChoice {
    Command(&'static str),
    /// The index of a recent file.
    Recent(usize),
}

impl OpenDraftApp {
    /// App menu button, quick access toolbar and title.
    pub(super) fn title_bar(&mut self, ctx: &egui::Context) {
        let mut clicked = None;
        let mut recent = None;
        let mut go_home = false;
        let update_view = self.updates.view(
            self.state.settings.update_checks,
            crate::updates::now_secs(),
        );
        egui::TopBottomPanel::top("title_bar")
            .frame(
                egui::Frame::new()
                    .fill(self.theme.colors.window_bg)
                    .inner_margin(egui::Margin::symmetric(8, 4)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (rect, menu_button) =
                        ui.allocate_exact_size(egui::vec2(26.0, 26.0), egui::Sense::click());
                    let mut hole = self.theme.colors.window_bg;
                    if menu_button.hovered() {
                        hole = ui.visuals().widgets.hovered.weak_bg_fill;
                        ui.painter().rect_filled(rect, 4.0, hole);
                    }
                    super::badge::paint(ui.painter(), rect.shrink(3.0), hole);
                    match self.app_menu(ui, &menu_button) {
                        Some(MenuChoice::Command(id)) => clicked = Some(id),
                        Some(MenuChoice::Recent(i)) => recent = Some(i),
                        None => {}
                    }
                    ui.separator();
                    for (icon, id) in QUICK_ACCESS {
                        let enabled = self.registry.is_enabled(id, &self.state);
                        let tip = match self.registry.get(id) {
                            Some(c) => match c.shortcut {
                                Some(s) => format!("{} ({s})", c.label),
                                None => c.label.to_owned(),
                            },
                            None => (*id).to_owned(),
                        };
                        let response = ui
                            .add_enabled_ui(enabled, |ui| {
                                widgets::icon_button(ui, &self.theme, *icon, &tip)
                            })
                            .inner;
                        if response.clicked() {
                            clicked = Some(*id);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        go_home = update_notice(ui, &self.theme, &update_view);
                    });
                });
            });
        if let Some(id) = clicked {
            self.run(id);
        }
        if let Some(i) = recent {
            self.state.open_recent(i);
        }
        if go_home {
            self.state.show_home();
        }
    }

    /// The application menu behind the badge button: the file commands and the
    /// recent drawings.
    fn app_menu(&self, ui: &egui::Ui, anchor: &egui::Response) -> Option<MenuChoice> {
        const ITEMS: &[(&str, &str)] = &[
            ("New", "file.new"),
            ("Open...", "file.open"),
            ("Save", "file.save"),
            ("Save As...", "file.save_as"),
            ("Save as Template...", "file.save_template"),
            ("Export DXF...", "file.export_dxf"),
            ("Plot layout...", "file.plot"),
        ];
        let theme = &self.theme;
        let recent = self.state.recent.paths();
        menu::popup(ui, theme, egui::Id::new("app_menu"), anchor, 220.0, |ui| {
            let mut picked = None;
            for (label, id) in ITEMS {
                if menu::menu_row(ui, theme, label, false) {
                    picked = Some(MenuChoice::Command(id));
                }
            }
            if !recent.is_empty() {
                ui.separator();
                ui.label(egui::RichText::new("Recent").color(theme.colors.text_weak));
                for (i, path) in recent.iter().enumerate() {
                    let name = path.file_name().map_or_else(
                        || path.display().to_string(),
                        |n| n.to_string_lossy().into_owned(),
                    );
                    if menu::menu_row(ui, theme, &name, false) {
                        picked = Some(MenuChoice::Recent(i));
                    }
                }
            }
            ui.separator();
            for (label, id) in [
                ("Units", "format.units"),
                ("Settings", "manage.settings"),
                ("Exit", "file.exit"),
            ] {
                if menu::menu_row(ui, theme, label, false) {
                    picked = Some(MenuChoice::Command(id));
                }
            }
            picked
        })
    }

    pub(super) fn ribbon(&mut self, ctx: &egui::Context) {
        // The Block Editor brings its own tab, and it opens on it.
        let editing = self.state.in_block_editor();
        let showing = self
            .ribbon
            .tabs
            .last()
            .is_some_and(|t| t.title == crate::ribbon_def::BLOCK_EDITOR_TAB);
        if editing != showing {
            self.ribbon = if editing {
                crate::ribbon_def::block_editor_ribbon()
            } else {
                crate::ribbon_def::default_ribbon()
            };
            self.ribbon_state.active_tab = if editing {
                self.ribbon.tabs.len() - 1
            } else {
                0
            };
        }
        let mut clicked = None;
        let mut edit = None;
        egui::TopBottomPanel::top("ribbon")
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                let source = Source {
                    registry: &self.registry,
                    state: &self.state,
                    dock: &self.dock,
                };
                let out = self
                    .ribbon
                    .show(ui, &self.theme, &mut self.ribbon_state, &source);
                clicked = out.clicked;
                edit = crate::ribbon_slots::show(ui, &self.theme, &self.state, &out.slots);
            });
        if let Some(edit) = edit {
            self.state.apply_slot_edit(edit);
        }
        if let Some(id) = clicked {
            self.run(id);
        }
    }

    pub(super) fn document_tabs(&mut self, ctx: &egui::Context) {
        let mut event = None;
        egui::TopBottomPanel::top("document_tabs")
            .frame(
                egui::Frame::new()
                    .fill(self.theme.colors.window_bg)
                    .inner_margin(egui::Margin {
                        left: 8,
                        right: 8,
                        top: 4,
                        bottom: 0,
                    }),
            )
            .show(ctx, |ui| {
                let titles: Vec<String> = self
                    .state
                    .docs
                    .iter()
                    .filter(|d| !d.placeholder)
                    .map(|d| d.tab_title())
                    .collect();
                let selected = (!self.state.home).then_some(self.state.active);
                event = document_tabs(ui, &self.theme, &titles, selected);
            });
        match event {
            Some(DocumentTabEvent::Home) => self.state.show_home(),
            Some(DocumentTabEvent::Select(i)) => self.state.show_document(i),
            Some(DocumentTabEvent::New) => self.run("file.new"),
            Some(DocumentTabEvent::Close(i)) => self.state.request_close(i),
            None => {}
        }
    }

    pub(super) fn status_bar(&mut self, ctx: &egui::Context) {
        let mut toggled = None;
        egui::TopBottomPanel::bottom("status_bar")
            .frame(
                egui::Frame::new()
                    .fill(self.theme.colors.window_bg)
                    .inner_margin(egui::Margin::symmetric(8, 3)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    self.status_message(ui);
                    if self.state.file_work_stalled()
                        && widgets::secondary_button(ui, &self.theme, "Cancel").clicked()
                    {
                        self.state.cancel_file_work();
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        self.coordinate_readout(ui);
                        ui.separator();
                        toggled = self.mode_toggles(ui);
                        ui.separator();
                        if !self.state.home {
                            let count = self.state.session().document().len_here();
                            ui.label(
                                egui::RichText::new(plural(count, "object"))
                                    .color(self.theme.colors.text_weak),
                            );
                        }
                    });
                });
            });
        if let Some(command) = toggled {
            self.run(command);
        }
    }

    /// What the running command asks for, otherwise the last feedback.
    fn status_message(&self, ui: &mut egui::Ui) {
        let command_status = self.state.command_status();
        let message = if let Some(status) = &command_status {
            status
        } else if self.state.message.is_empty() {
            "Ready"
        } else {
            &self.state.message
        };
        // Leave room for the readout and toggles on the right; a long message
        // is cut short rather than running into them.
        let room = (ui.available_width() - READOUT_WIDTH - 360.0).max(120.0);
        ui.scope(|ui| {
            ui.set_max_width(room);
            ui.add(
                egui::Label::new(egui::RichText::new(message).color(self.theme.colors.text_weak))
                    .truncate(),
            );
        });
    }

    /// Fixed width so the toggles to its left do not jump as the readout
    /// changes length.
    fn coordinate_readout(&self, ui: &mut egui::Ui) {
        ui.allocate_ui_with_layout(
            egui::vec2(READOUT_WIDTH, ui.spacing().interact_size.y),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.set_min_width(READOUT_WIDTH);
                let text = match self.state.cursor {
                    Some(p) => {
                        let units = self.state.units();
                        format!(
                            "X: {}   Y: {}",
                            format_length(&units, p.x),
                            format_length(&units, p.y)
                        )
                    }
                    None => "X: ----   Y: ----".to_owned(),
                };
                ui.label(egui::RichText::new(text).color(self.theme.colors.text_weak));
            },
        );
    }

    /// "name* - OpenDraft", following the active drawing; a debug build or
    /// beta says so (`OpenDraft Dev`).
    pub(super) fn update_window_title(&mut self, ctx: &egui::Context) {
        let app = crate::profile::name();
        let title = if self.state.home {
            format!("Home - {app}")
        } else {
            format!("{} - {app}", self.state.docs[self.state.active].tab_title())
        };
        if title != self.window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
    }
}
