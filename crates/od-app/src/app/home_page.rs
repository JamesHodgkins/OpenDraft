//! The Home page: shown in place of the drawing and panels while the Home tab
//! is selected. Start something, pick up a recent drawing, jump to an open
//! one, and what is new in this version.

use std::path::{Path, PathBuf};

use eframe::egui;
use od_ui::widgets;

mod cards;
mod controls;
mod layout;
mod news;
mod recent;
mod recovery;
mod start;

use super::OpenDraftApp;
use crate::home_scan::ScanRequest;
use crate::recent::{RecentId, RecentView, MAX_PINNED};
use crate::text::plural;
use layout::{layout, Layout, COLUMN_GAP};
use news::whats_new_section;
use recent::{recent_section, row, RowText, Section};
use recovery::recovery_section;
use start::start_section;
pub(super) use start::TemplateList;

/// What the user chose on the page this frame.
pub(super) enum Pick {
    New,
    Open,
    /// The templates dropdown was opened: read the folder again.
    ScanTemplates,
    NewFromTemplate(crate::templates::Template),
    OpenTemplatesFolder,
    Recent(RecentId),
    Pin(RecentId),
    Unpin(RecentId),
    Forget(RecentId),
    Locate(RecentId),
    ShowInFolder(PathBuf),
    /// The path is on the clipboard.
    Copied,
    ClearUnpinned,
    SetView(RecentView),
    RecoverSession(usize),
    DiscardSession(usize),
    DeferRecovery,
    DismissWhatsNew,
    Show(usize),
}

impl OpenDraftApp {
    pub(super) fn home_page(&mut self, ui: &mut egui::Ui) {
        self.refresh_recent_entries(ui.ctx());
        self.templates.collect();
        let entries = self.home_scan.entries(&self.state.recent);
        let pinned_full = self.state.recent.pinned_count() >= MAX_PINNED;
        let pictures = self.home_previews.update(ui.ctx(), &entries);
        let view = self.state.recent.view;
        let theme = &self.theme;
        let mut pick = None;
        let page = egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                let shown = layout(ui.available_width());
                let (margin, width) = match shown {
                    Layout::Single { width, margin } => (margin, width),
                    Layout::Wide { main, side, margin } => (margin, main + side + COLUMN_GAP),
                };
                let recovery = |ui: &mut egui::Ui, pick: &mut Option<Pick>| {
                    ui.push_id("home_recovery", |ui| {
                        recovery_section(
                            ui,
                            theme,
                            &self.state.pending_recovery,
                            &self.state.recovery_report,
                            pick,
                        )
                    });
                };
                // Last on the page, and only until it is dismissed.
                let news = crate::whats_new::current()
                    .filter(|n| !self.state.recent.whats_new_seen(n.version));
                let announce = |ui: &mut egui::Ui, pick: &mut Option<Pick>| {
                    if let Some(note) = news {
                        ui.push_id("home_whats_new", |ui| {
                            whats_new_section(ui, theme, note, pick)
                        });
                    }
                };
                let start = |ui: &mut egui::Ui, pick: &mut Option<Pick>| {
                    ui.push_id("home_start", |ui| {
                        start_section(
                            ui,
                            theme,
                            &self.templates,
                            self.state.templates_dir.as_deref(),
                            pick,
                        )
                    });
                };
                let recent = |ui: &mut egui::Ui, pick: &mut Option<Pick>| {
                    ui.push_id("home_recent", |ui| {
                        recent_section(
                            ui,
                            &Section {
                                theme,
                                entries: &entries,
                                pinned_full,
                                pictures: &pictures,
                                view,
                            },
                            pick,
                        )
                    });
                };
                ui.horizontal_top(|ui| {
                    ui.add_space(margin);
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        ui.push_id("home_header", |ui| header_section(ui, theme));
                        match shown {
                            Layout::Single { .. } => {
                                recovery(ui, &mut pick);
                                start(ui, &mut pick);
                                ui.push_id("home_open_now", |ui| {
                                    self.open_drawings_section(ui, &mut pick)
                                });
                                recent(ui, &mut pick);
                                ui.add_space(22.0);
                                announce(ui, &mut pick);
                            }
                            Layout::Wide { main, side, .. } => {
                                ui.horizontal_top(|ui| {
                                    ui.vertical(|ui| {
                                        ui.set_width(main);
                                        recovery(ui, &mut pick);
                                        start(ui, &mut pick);
                                        recent(ui, &mut pick);
                                    });
                                    ui.add_space(COLUMN_GAP);
                                    ui.vertical(|ui| {
                                        ui.set_width(side);
                                        ui.push_id("home_open_now", |ui| {
                                            self.open_drawings_section(ui, &mut pick)
                                        });
                                        announce(ui, &mut pick);
                                    });
                                });
                            }
                        }
                        ui.add_space(30.0);
                    });
                });
            });
        // For the layout tests: how wide the page really came out.
        ui.ctx()
            .data_mut(|d| d.insert_temp(egui::Id::new("home_content_width"), page.content_size.x));
        self.apply_home_pick(ui.ctx(), pick);
    }

    /// Have the files behind the recent list looked at (on a worker), and
    /// merge entries that turn out to be one file.
    fn refresh_recent_entries(&mut self, ctx: &egui::Context) {
        let open = self
            .state
            .docs
            .iter()
            .filter_map(|d| Some((d.uid, d.path.clone()?)))
            .collect();
        let request = ScanRequest::new(&self.state.recent, open);
        if let Some(found) = self.home_scan.update(request, ctx) {
            self.state.recent.merge_resolved(&found);
        }
    }

    /// Drawings already open, for jumping between many tabs.
    fn open_drawings_section(&self, ui: &mut egui::Ui, pick: &mut Option<Pick>) {
        let open: Vec<_> = self
            .state
            .docs
            .iter()
            .enumerate()
            .filter(|(_, d)| !d.placeholder)
            .collect();
        if open.is_empty() {
            return;
        }
        widgets::section_header(ui, &self.theme, "Open now");
        for (i, doc) in open {
            let detail = match &doc.path {
                Some(path) => path.display().to_string(),
                None => "Not saved yet".to_owned(),
            };
            let right = plural(doc.session.document().len_here(), "object");
            if row(
                ui,
                &self.theme,
                doc.uid,
                &RowText {
                    name: &doc.tab_title(),
                    detail: &detail,
                    right: &right,
                    dim: false,
                    trailing: 0.0,
                },
            )
            .clicked()
            {
                *pick = Some(Pick::Show(i));
            }
        }
        ui.add_space(22.0);
    }

    fn apply_home_pick(&mut self, ctx: &egui::Context, pick: Option<Pick>) {
        match pick {
            Some(Pick::New) => self.run("file.new"),
            Some(Pick::Open) => self.run("file.open"),
            Some(Pick::ScanTemplates) => self
                .templates
                .refresh(ctx, self.state.templates_dir.clone()),
            Some(Pick::NewFromTemplate(template)) => self.state.new_from_template(&template),
            Some(Pick::OpenTemplatesFolder) => {
                if let Some(folder) = self.state.templates_dir.clone() {
                    let _ = std::fs::create_dir_all(&folder);
                    if let Err(error) = open_folder(&folder) {
                        self.state
                            .tell(format!("Could not open {}: {error}", folder.display()));
                    }
                }
            }
            Some(Pick::Recent(id)) => self.state.open_recent_id(id),
            Some(Pick::Pin(id)) => {
                if self.state.recent.pin(id).is_err() {
                    self.state.message = format!("You can pin up to {MAX_PINNED} drawings");
                }
            }
            Some(Pick::Unpin(id)) => self.state.recent.unpin(id),
            Some(Pick::Forget(id)) => self.state.recent.remove(id),
            Some(Pick::Locate(id)) => self.state.locate_recent(id),
            Some(Pick::ShowInFolder(path)) => {
                if let Err(error) = show_in_folder(&path) {
                    self.state
                        .tell(format!("Could not show {}: {error}", path.display()));
                }
            }
            Some(Pick::ClearUnpinned) => {
                self.state.recent.clear_unpinned();
            }
            Some(Pick::SetView(view)) => self.state.recent.view = view,
            Some(Pick::RecoverSession(i)) => self.state.recover_session(i),
            Some(Pick::DiscardSession(i)) => self.state.discard_session(i),
            Some(Pick::DeferRecovery) => self.state.defer_recovery(),
            Some(Pick::DismissWhatsNew) => self
                .state
                .recent
                .dismiss_whats_new(env!("CARGO_PKG_VERSION")),
            Some(Pick::Copied) => self.state.message = "Path copied".to_owned(),
            Some(Pick::Show(i)) => self.state.show_document(i),
            None => {}
        }
    }
}

/// Open `folder` in the system's file manager.
fn open_folder(folder: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    let program = "explorer";
    #[cfg(not(windows))]
    let program = "xdg-open";
    std::process::Command::new(program)
        .arg(folder)
        .spawn()
        .map(|_| ())
}

/// Reveal `path` in the system's file manager.
fn show_in_folder(path: &Path) -> std::io::Result<()> {
    #[cfg(windows)]
    let child = std::process::Command::new("explorer")
        .arg(format!("/select,{}", path.display()))
        .spawn();
    #[cfg(not(windows))]
    let child = std::process::Command::new("xdg-open")
        .arg(path.parent().unwrap_or(path))
        .spawn();
    child.map(|_| ())
}

/// The title, the version, and how to open a file by dropping it.
fn header_section(ui: &mut egui::Ui, theme: &od_ui::Theme) {
    let c = &theme.colors;
    ui.add_space(36.0);
    ui.label(
        egui::RichText::new("OpenDraft")
            .size(theme.font_size_heading * 1.6)
            .color(c.text),
    );
    ui.label(
        egui::RichText::new(format!(
            "2D drafting  \u{b7}  version {}",
            crate::build_info::display()
        ))
        .color(c.text_weak),
    );
    ui.label(
        egui::RichText::new("Drop a .odx, .dxf or .dwg file on the window to open it.")
            .color(c.text_weak),
    );
    ui.add_space(26.0);
}
