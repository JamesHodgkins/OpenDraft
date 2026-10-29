//! Application shell: title bar, ribbon, document tabs, properties panel,
//! canvas and status bar. Everything the user can do goes through the command
//! registry (see `commands.rs`); this file only draws UI and dispatches.

mod automation_update;
mod badge;
mod chrome;
mod command_line;
mod consent_dialog;
mod context_menu;
mod dim_manager;
mod dim_preview;
mod dim_style_dialog;
mod dim_style_tabs;
mod dynamic_boxes;
mod dynamic_input;
mod dynamic_overlay;
#[cfg(test)]
mod dynamic_tests;
mod file_dialogs;
mod home_page;
mod linetype_dialog;
mod mleader_manager;
mod mleader_style_dialog;
mod mleader_style_tabs;
mod overkill_dialog;
mod page_setup_dialog;
mod panels;
mod power_select_dialog;
mod purge_dialog;
mod settings_dialog;
mod snap_menu;
mod status_toggles;
mod text_style_dialog;
mod text_style_preview;
mod units_dialog;
mod update_notice;

use eframe::egui;
use egui_dock::DockState;
use od_commands::Registry;
use od_ui::icons;
use od_ui::ribbon::{CommandSource, Ribbon, RibbonState};
use od_ui::shortcuts::{modifier_count, parse_shortcut};
use od_ui::Theme;

use crate::dock::{self, PanelTab};
use crate::locations::Location;
use crate::recent::RecentFiles;
use crate::settings::Settings;
use crate::state::AppState;
use crate::{commands, ribbon_def};

/// Quick access toolbar: icon, command id.
const QUICK_ACCESS: &[(char, &str)] = &[
    (icons::FILE_NEW, "file.new"),
    (icons::FILE_OPEN, "file.open"),
    (icons::FILE_SAVE, "file.save"),
    (icons::EDIT_UNDO, "edit.undo"),
    (icons::EDIT_REDO, "edit.redo"),
];

pub struct OpenDraftApp {
    theme: Theme,
    /// Working copy edited by the Settings dialog while it is open.
    settings_draft: Option<Settings>,
    settings_tab: usize,
    /// Working copy edited by the Units dialog while it is open.
    units_draft: Option<od_core::Units>,
    /// While Direction Control is open: the base angle to restore on Cancel.
    units_direction: Option<f64>,
    /// Working copy edited by the Linetype Manager while it is open.
    linetype_draft: Option<linetype_dialog::Draft>,
    /// Working copy edited by the Text Style dialog while it is open.
    text_style_draft: Option<crate::text_style_draft::Draft>,
    /// Text typed into New Text Style while it is open.
    text_style_new: Option<String>,
    /// Working copy edited by the Dimension Style dialog while it is open.
    dim_style_draft: Option<crate::dim_style_draft::Draft>,
    /// What the Dimension Style Manager remembers while it is open.
    dim_manager: Option<dim_manager::Ui>,
    mleader_windows: mleader_manager::Windows,
    registry: Registry<AppState>,
    /// Parsed shortcuts, most modifiers first so `Ctrl+Shift+Z` wins over `Ctrl+Z`.
    shortcuts: Vec<(egui::KeyboardShortcut, &'static str)>,
    state: AppState,
    ribbon: Ribbon,
    ribbon_state: RibbonState,
    dock: DockState<PanelTab>,
    /// The window title last sent, so it is only set when it changes.
    window_title: String,
    /// When recovery copies were last written.
    last_autosave: std::time::Instant,
    /// Whether the file worker has been told how to wake the window.
    io_waker_set: bool,
    f12_pressed_at: Option<f64>,
    dynamic_suppressed: bool,
    /// Ctrl is down, and nothing else has been pressed since it went down.
    ctrl_alone: bool,
    ctrl_down: bool,
    dynamic_keys: Vec<egui::Event>,
    /// The automation core (always there) and the MCP socket in front of it.
    automation: automation_update::Automation,
    /// `OD_AUTOMATION=1` turned the endpoint on for this run only (nothing
    /// is saved), for scripts and tests.
    automation_forced: bool,
    /// What Home knows of the recent files, and the pictures it shows for them.
    home_scan: crate::home_scan::HomeScan,
    home_previews: crate::preview_textures::PreviewTextures,
    templates: home_page::TemplateList,
    /// Whether a newer version exists, once the user has allowed asking.
    updates: crate::updates::Updates,
}

impl OpenDraftApp {
    pub fn new(
        theme: Theme,
        settings: Settings,
        dock: DockState<PanelTab>,
        recent: RecentFiles,
        updates: crate::updates::Updates,
        open: Vec<std::path::PathBuf>,
    ) -> Self {
        crate::system_fonts::start_scan();
        let registry = commands::build_registry();
        let mut state = AppState::new();
        // Read fresh each launch, so redrawn icons show without a rebuild.
        crate::icon_library::load(&mut state.cmdline);
        state.recent = recent;
        state.templates_dir = Location::Templates.resolve(&settings);
        let recovery = Location::Recovery.resolve(&settings);
        state.settings = settings;
        let home_previews = crate::preview_textures::PreviewTextures::start(&mut state.previews);
        if open.is_empty() {
            state.start_on_home();
        }
        state.open_paths(&open);
        // A crashed run's autosave copies are offered back on the Home page.
        if let Some(root) = recovery {
            state.start_recovery(&root);
        }
        let mut shortcuts: Vec<_> = registry
            .iter()
            .filter_map(|c| Some((parse_shortcut(c.shortcut?)?, c.id)))
            .collect();
        shortcuts.sort_by_key(|(s, _)| std::cmp::Reverse(modifier_count(s)));
        Self {
            updates,
            home_previews,
            templates: Default::default(),
            theme,
            settings_draft: None,
            settings_tab: 0,
            units_draft: None,
            units_direction: None,
            linetype_draft: None,
            text_style_draft: None,
            text_style_new: None,
            dim_style_draft: None,
            dim_manager: None,
            mleader_windows: Default::default(),
            registry,
            shortcuts,
            state,
            ribbon: ribbon_def::default_ribbon(),
            ribbon_state: RibbonState::default(),
            dock,
            window_title: String::new(),
            last_autosave: std::time::Instant::now(),
            io_waker_set: false,
            f12_pressed_at: None,
            dynamic_suppressed: false,
            ctrl_alone: false,
            ctrl_down: false,
            dynamic_keys: Vec::new(),
            automation: automation_update::Automation::new(),
            automation_forced: std::env::var_os("OD_AUTOMATION").is_some_and(|v| v == "1"),
            home_scan: Default::default(),
        }
    }

    /// Run a command by id, reporting commands that have no implementation yet.
    fn run(&mut self, id: &str) {
        use od_commands::ExecuteError;
        match self.registry.execute(id, &mut self.state) {
            Ok(()) => {
                if let Some(command) = self.registry.get(id).filter(|c| c.id != "edit.cancel") {
                    let name = crate::cmdline::echo_name(command.id, command.label);
                    self.state.cmdline.echo_command(name);
                    self.state.cmdline.note_recent(command.id);
                }
            }
            Err(ExecuteError::Disabled(_)) => {}
            Err(ExecuteError::Unknown(id)) => {
                self.state.message = format!("Not implemented yet: {id}");
            }
        }
    }

    /// Typing anywhere (with no text field focused) goes to the command
    /// line, as in AutoCAD. So does a bare Enter: it answers the current
    /// prompt (finishes Copy, accepts a default) or repeats the last command.
    fn route_typing_to_command_line(&mut self, ctx: &egui::Context) {
        if self.route_dynamic_input(ctx) {
            return;
        }
        if self.state.home
            || self.state.mtext_editing()
            || self.state.modal_open()
            || ctx.wants_keyboard_input()
        {
            return;
        }
        let command_line_active = dock::is_open(&self.dock, PanelTab::CommandLine)
            && !self.state.cmdline.input.is_empty();
        // egui drops focus from the command line on Escape, so the shortcuts
        // would take it as a cancel; typed text is cleared first instead.
        if command_line_active
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.state.cmdline.input.clear();
            self.state.cmdline.focus = true;
            ctx.request_repaint();
            return;
        }
        // A canvas click may take widget focus without ending the command-line
        // entry. The next keyboard event returns ownership to the existing
        // entry, including navigation and submission keys that carry no text.
        if command_line_active
            && ctx.input(|i| {
                i.events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Key { pressed: true, .. }))
            })
        {
            self.state.cmdline.focus = true;
            return;
        }
        // Space acts like Enter, except where text is being asked for.
        let wants_text = self
            .state
            .dynamic_ask()
            .is_some_and(|ask| ask.wants == od_commands::Wants::Text);
        self.watch_ctrl(ctx);
        let enter = ctx.input(|i| {
            i.key_pressed(egui::Key::Enter) || (!wants_text && i.key_pressed(egui::Key::Space))
        });
        if enter {
            if let Some(id) = self.state.cmd_submit("") {
                self.run(id);
            }
        }
        if !dock::is_open(&self.dock, PanelTab::CommandLine) {
            return;
        }
        let typed = ctx.input(|i| {
            i.events
                .iter()
                .any(|e| matches!(e, egui::Event::Text(t) if !t.trim().is_empty()))
        });
        if typed {
            self.state.cmdline.focus = true;
        }
    }

    /// Ctrl pressed and released on its own cycles the options of a grip being
    /// edited (Stretch, Lengthen). Ctrl with a key or a click is a shortcut.
    fn watch_ctrl(&mut self, ctx: &egui::Context) {
        let (down, other) = ctx.input(|i| {
            let other = i.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::Key { pressed: true, key, .. } if !matches!(key, egui::Key::Escape)
                ) || matches!(e, egui::Event::PointerButton { pressed: true, .. })
            });
            (i.modifiers.ctrl, other)
        });
        if down && !self.ctrl_down {
            self.ctrl_alone = true;
        }
        if other {
            self.ctrl_alone = false;
        }
        if !down && self.ctrl_down && self.ctrl_alone {
            self.state.cycle_grip_option();
        }
        self.ctrl_down = down;
    }

    /// Drawings dropped on the window open; closing the window (or the Exit
    /// command) first asks about unsaved changes.
    fn handle_files_and_exit(&mut self, ctx: &egui::Context) {
        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        if !dropped.is_empty() {
            self.state.open_paths(&dropped);
        }
        let closing = ctx.input(|i| i.viewport().close_requested());
        if closing && !self.state.exit_ok {
            self.state.try_exit();
        }
        if std::mem::take(&mut self.state.exit_requested) {
            self.state.try_exit();
        }
        // The window goes once every question is answered and no file is
        // still being written; until then a close request is held back.
        if self.state.can_quit() {
            if !closing {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        } else if closing {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.state.exit_ok && self.state.io_busy() {
                self.state.message = "Closing when the files are written...".into();
            }
        }
    }

    /// Write recovery copies every few minutes (see Settings > Files). The
    /// app only runs a frame when something happens, so ask for one now and
    /// then to keep the clock ticking while the user is idle.
    fn autosave_when_due(&mut self, ctx: &egui::Context) {
        let minutes = self.state.settings.autosave_minutes;
        if minutes == 0 {
            return;
        }
        let interval = std::time::Duration::from_secs(u64::from(minutes) * 60);
        if self.last_autosave.elapsed() >= interval {
            self.state.autosave();
            self.last_autosave = std::time::Instant::now();
        }
        ctx.request_repaint_after(std::time::Duration::from_secs(30));
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        // Don't steal keys while the user is typing in a text field.
        if self.state.mtext_editing()
            || !self.state.cmdline.input.is_empty()
            || ctx.wants_keyboard_input()
        {
            return;
        }
        let clipboard = crate::clipboard::shortcut(ctx);
        if let Some(id) = clipboard {
            return self.run(id);
        }
        let fired = ctx.input_mut(|input| {
            self.shortcuts
                .iter()
                .find(|(shortcut, id)| {
                    *id != "view.dynamic_input" && input.consume_shortcut(shortcut)
                })
                .map(|(_, id)| *id)
        });
        if let Some(id) = fired {
            self.run(id);
        }
    }
}

/// Lets the ribbon ask the registry about enabled state and shortcuts.
struct Source<'a> {
    registry: &'a Registry<AppState>,
    state: &'a AppState,
    dock: &'a DockState<PanelTab>,
}

impl CommandSource for Source<'_> {
    fn is_enabled(&self, command: &str) -> bool {
        self.registry.is_enabled(command, self.state)
    }

    fn shortcut_text(&self, command: &str) -> Option<&str> {
        self.registry.get(command)?.shortcut
    }

    fn is_active(&self, command: &str) -> bool {
        match dock::panel_of(command) {
            Some(panel) => dock::is_open(self.dock, panel),
            None => self.registry.is_checked(command, self.state),
        }
    }
}

impl eframe::App for OpenDraftApp {
    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        self.capture_dynamic_keys(ctx, input);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(request) = self.state.layout_request.take() {
            dock::apply(&mut self.dock, request);
        }
        if !self.io_waker_set {
            // A finished file job wakes the window to collect it.
            let ctx = ctx.clone();
            self.state.io.set_waker(move || ctx.request_repaint());
            self.io_waker_set = true;
        }
        self.state.poll_io();
        self.tick_updates(ctx);
        self.state.pump_command();
        self.update_automation(ctx);
        self.state.sync_mtext_editor();
        self.handle_files_and_exit(ctx);
        self.route_typing_to_command_line(ctx);
        self.handle_shortcuts(ctx);
        self.title_bar(ctx);
        self.ribbon(ctx);
        self.document_tabs(ctx);
        self.status_bar(ctx);
        self.panels(ctx);
        self.snap_menu(ctx);
        self.context_menu(ctx);
        self.tool_windows(ctx);
        self.confirm_dialog(ctx);
        self.consent_dialog(ctx);
        self.autosave_when_due(ctx);
        self.update_window_title(ctx);
        crate::window_memory::observe(ctx);
        // Cancel appears once file work has taken a while; get a frame then.
        if self.state.io_busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        // A command between prompts is busy for a moment: look again soon.
        if self.state.command_running() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.state.settings.save(storage);
        self.updates.save(storage);
        crate::corner_memory::save(storage);
        crate::rect_memory::save(storage);
        crate::spline_memory::save(storage);
        crate::hatch_memory::save(storage);
        crate::text_memory::save(storage);
        dock::save(&self.dock, storage);
        self.state.recent.save(storage);
        crate::window_memory::save(storage);
    }
}
