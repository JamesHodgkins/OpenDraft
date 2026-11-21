//! The dock area and the panels inside it: drawing view and properties.

use eframe::egui;
use opendraft_canvas::{CanvasStyle, Click, GridSettings, RegionSelect};
use opendraft_commands::Registry;
use opendraft_ui::Theme;

use super::command_line_strip::StripRows;
use super::OpenDraftApp;
use crate::dock::{self, PanelTab};
use crate::state::AppState;

impl OpenDraftApp {
    /// The dock area: the drawing view and the side panels as tabs.
    pub(super) fn panels(&mut self, ctx: &egui::Context) {
        if self.state.home {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(self.theme.colors.window_bg))
                .show(ctx, |ui| self.home_page(ui));
            return;
        }
        // As a strip the command line keeps two lines of history above its
        // input row, and collapses to that row alone.
        let strip = dock::command_line_strip(&self.dock);
        let rows = StripRows::measure(ctx, &self.theme, strip);
        let (least, collapsed) = match strip {
            Some(_) => (rows.height(2), rows.height(0)),
            None => (dock::COMMAND_LINE_MIN_HEIGHT, dock::COLLAPSED_HEIGHT),
        };
        let colors = &self.theme.colors;
        let mut viewer = PanelViewer {
            theme: &self.theme,
            style: CanvasStyle {
                background: colors.canvas_bg,
                grid_minor: colors.text.gamma_multiply(0.03),
                grid_major: colors.text.gamma_multiply(0.06),
                axis_x: egui::Color32::from_rgb(70, 190, 110).gamma_multiply(0.3),
                axis_y: egui::Color32::from_rgb(220, 80, 80).gamma_multiply(0.3),
                entity: colors.text,
                hover: colors.accent.gamma_multiply(0.25),
                selected: colors.accent.lerp_to_gamma(egui::Color32::WHITE, 0.35),
                selected_glow: colors.accent.gamma_multiply(0.5),
                snap: egui::Color32::from_rgb(255, 214, 90),
                window: colors.accent,
                crossing: egui::Color32::from_rgb(70, 190, 110),
            },
            grid: GridSettings {
                visible: self.state.settings.show_grid,
                spacing: self.state.settings.grid_spacing,
            },
            snap_modes: self.state.settings.snap,
            registry: &self.registry,
            run: None,
            automation: &mut self.automation,
            state: &mut self.state,
            cursor: None,
            click: None,
            double_click: None,
            region: None,
            prompted: None,
            picked: None,
            path: None,
            pick_shift: false,
            grips: Vec::new(),
            prompt_enter: false,
            snap_menu: None,
            context_menu: None,
            refused: false,
            placed_snap: None,
            dynamic_suppressed: self.dynamic_suppressed,
            command_line_strip: strip.map(|side| (side, rows)),
            drawing_alone: dock::drawing_alone(&self.dock),
        };
        let style = dock::style(&self.theme, &ctx.style(), least);
        let shares = dock::split_shares(&self.dock);
        // Before the dock is shown, where a panel may be closed by its tab.
        self.places.note(&self.dock, Some(ctx));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(self.theme.colors.window_bg))
            .show(ctx, |ui| {
                dock::show_area(ui, &mut self.dock, style, &self.theme, &mut viewer);
            });
        let PanelViewer {
            cursor,
            click,
            double_click,
            region,
            prompted,
            picked,
            path,
            pick_shift,
            grips,
            prompt_enter,
            snap_menu,
            context_menu,
            refused,
            placed_snap,
            run,
            ..
        } = viewer;
        dock::keep_minimum_sizes(&mut self.dock, &shares, least);
        dock::tuck_command_line(&mut self.dock);

        if let Some(collapse) = self.state.cmdline.resize.take() {
            let mut saved = self.state.cmdline.saved_split;
            dock::resize_command_line(&mut self.dock, collapse, &mut saved, collapsed);
            self.state.cmdline.saved_split = saved;
        }
        if let Some(at) = prompted.filter(|_| !self.state.defer_one_shot(placed_snap)) {
            let has_draft = {
                let input = &self.state.docs[self.state.active].dynamic_input;
                !input.draft.is_empty()
                    || !input.locks.is_empty()
                    || input.locked_coordinate.is_some()
            };
            if has_draft
                && self
                    .state
                    .waiting()
                    .is_some_and(|ask| ask.wants != opendraft_commands::Wants::Object)
            {
                self.state.docs[self.state.active].dynamic_input.cursor = Some(at);
                self.state.commit_dynamic_input();
            } else {
                // One answer per click: once a point is taken, the next
                // prompt may want an object, and must not get this click too.
                let wants_object = self.state.waiting().map(|a| a.wants)
                    == Some(opendraft_commands::Wants::Object);
                if wants_object {
                    self.state.answer_object(at, picked, pick_shift);
                } else {
                    self.state.answer_point(at);
                }
                self.state.answer_field_pick(picked);
            }
        }
        if let Some(points) = path {
            self.state.answer_path(points, pick_shift);
        }
        if refused {
            self.state.refuse_one_shot();
        }
        if let Some(at) = snap_menu {
            self.state.snap_menu = Some((at.x, at.y));
        }
        if let Some(at) = context_menu.filter(|_| !self.state.command_running()) {
            self.state.context_menu = Some((at.x, at.y));
        }
        if prompt_enter {
            let input = &self.state.docs[self.state.active].dynamic_input;
            if !input.draft.is_empty()
                || !input.locks.is_empty()
                || input.locked_coordinate.is_some()
            {
                self.state.commit_dynamic_input();
            } else {
                self.state.answer_enter();
            }
        }
        if !grips.is_empty() {
            self.state.start_grip_edit(grips);
        }
        if let Some(found) = region {
            self.state.apply_region(found);
        }
        if let Some(id) = run {
            self.run(id);
        }
        if let Some(click) = click {
            self.state.click_select(click.entity, click.remove);
        }
        if let Some(id) = double_click {
            // Double-clicking a text edits it, as TEXTEDIT would, and
            // a block reference opens its block in the Block Editor, or its
            // attribute editor when it carries attributes.
            let doc = &mut self.state.docs[self.state.active];
            let command = match doc.session.document().get(id) {
                Some(opendraft_core::Entity::MText(_) | opendraft_core::Entity::Text(_)) => {
                    Some("annotate.textedit")
                }
                Some(opendraft_core::Entity::Insert(i)) if !i.attributes.is_empty() => {
                    Some("block.attedit")
                }
                Some(opendraft_core::Entity::Insert(_)) => Some("block.edit"),
                _ => None,
            };
            if let Some(command) = command.filter(|_| !self.state.command_running()) {
                let doc = &mut self.state.docs[self.state.active];
                doc.selection.clear();
                doc.selection.insert(id);
                self.run(command);
            }
        }
        if cursor != self.state.cursor {
            // The status bar is drawn before the canvas, so show the new
            // position on the next frame.
            ctx.request_repaint();
        }
        self.state.cursor = cursor;
    }
}

/// Draws each dock tab and collects what the drawing view reports.
pub(super) struct PanelViewer<'a> {
    pub(super) theme: &'a Theme,
    pub(super) state: &'a mut AppState,
    pub(super) style: CanvasStyle,
    pub(super) grid: GridSettings,
    pub(super) snap_modes: opendraft_canvas::SnapModes,
    pub(super) cursor: Option<opendraft_core::Point>,
    pub(super) click: Option<Click>,
    pub(super) double_click: Option<opendraft_core::EntityId>,
    pub(super) region: Option<RegionSelect>,
    pub(super) prompted: Option<opendraft_core::Point>,
    pub(super) picked: Option<opendraft_core::EntityId>,
    pub(super) path: Option<Vec<opendraft_core::Point>>,
    pub(super) pick_shift: bool,
    pub(super) grips: Vec<opendraft_canvas::GripHit>,
    pub(super) prompt_enter: bool,
    pub(super) snap_menu: Option<egui::Pos2>,
    pub(super) context_menu: Option<egui::Pos2>,
    pub(super) refused: bool,
    pub(super) placed_snap: Option<opendraft_canvas::SnapPoint>,
    pub(super) dynamic_suppressed: bool,
    /// The command line is drawn without its tab bar, behind a grip.
    pub(super) command_line_strip: Option<(dock::Strip, StripRows)>,
    /// The drawing view is drawn over its tab bar (see `drawing_tab`).
    pub(super) drawing_alone: bool,
    pub(super) registry: &'a Registry<AppState>,
    pub(super) run: Option<&'static str>,
    pub(super) automation: &'a mut super::automation_update::Automation,
}

impl egui_dock::TabViewer for PanelViewer<'_> {
    type Tab = PanelTab;

    fn title(&mut self, tab: &mut PanelTab) -> egui::WidgetText {
        tab.title().into()
    }

    fn closeable(&mut self, tab: &mut PanelTab) -> bool {
        tab.closeable()
    }

    fn allowed_in_windows(&self, tab: &mut PanelTab) -> bool {
        tab.closeable()
    }

    fn scroll_bars(&self, tab: &PanelTab) -> [bool; 2] {
        [false, matches!(tab, PanelTab::Properties)]
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut PanelTab) {
        match tab {
            PanelTab::Canvas => self.drawing(ui),
            PanelTab::Properties => self.properties(ui),
            PanelTab::Layers => self.layers(ui),
            PanelTab::CommandLine => self.command_line(ui),
            PanelTab::Assistant => crate::assistant::show_panel(
                ui,
                self.theme,
                &mut self.automation.assistant,
                &self.automation.core,
                self.state,
            ),
        }
    }
}

impl PanelViewer<'_> {
    pub(super) fn canvas(&mut self, ui: &mut egui::Ui) {
        let label_color = self.theme.colors.text_weak;
        self.prepare_dynamic_canvas(ui.ctx());
        self.state.docs[self.state.active].keep_space_valid();
        // A command line docked under the drawing shows them below itself.
        if !matches!(self.command_line_strip, Some((dock::Strip::Below, _))) {
            self.layout_tabs(ui);
        }
        let editing = self.state.mtext_editing() && self.state.run_doc == self.state.active;
        let sizes = self.state.settings.pointer_sizes();
        let crosshair = self.state.settings.crosshair();
        let zoom_speed = self.state.settings.zoom_speed;
        let perf = self.state.settings.performance.clone();
        // `TK` holds each point to an axis from the one before.
        let ortho = match self.state.once.tracking_chain() {
            true => opendraft_canvas::Ortho {
                enabled: true,
                strict: true,
            },
            false => self.state.settings.ortho,
        };
        let doc = &mut self.state.docs[self.state.active];
        doc.view.set_wheel_through_exclusions(editing);
        doc.view
            .set_hidden_entity(doc.mtext_editor.as_ref().and_then(|e| e.replaces));
        let picking = doc.mtext_editor.as_ref().is_some_and(|e| e.field_pick);
        if editing && !picking {
            // The editor owns the pointer over the drawing, except while the
            // user picks an object for a field.
            doc.view
                .set_input_exclusions(vec![ui.available_rect_before_wrap()]);
        }
        doc.view.set_pointer_sizes(sizes);
        doc.view
            .set_selection_style(self.state.settings.selection_style);
        // The picture of the plain entities is kept while the drawing is the
        // same (see `opendraft_canvas::layer_cache`).
        doc.view.set_document_token(Some(doc.session.draw_token()));
        // Shared, so that a new picture after a pan or zoom is made off this
        // thread; weak, so that an edit copies the drawing only while one is.
        let shared = doc.session.shared_document();
        doc.view
            .set_shared_document(std::sync::Arc::downgrade(&shared));
        doc.view.set_cache_config(perf.cache_config());
        doc.view.set_crosshair(crosshair);
        doc.view.set_zoom_speed(zoom_speed);
        doc.view.set_ortho(ortho);
        doc.view.set_tracking(self.state.settings.tracking.clone());
        doc.view
            .set_temp_tracking_points(self.state.once.temps().to_vec());
        doc.view.set_snap_override(self.state.once.over().cloned());
        let started = crate::frame_readout::Readout::start(perf.show_frame_time);
        let output = doc.view.show(
            ui,
            &self.style,
            &self.grid,
            doc.session.document(),
            &doc.selection,
            &self.snap_modes,
        );
        let mut finished = None;
        if let (true, Some(canvas)) = (editing, output.rect) {
            if let Some(editor) = doc.mtext_editor.as_mut() {
                finished = crate::mtext_editor::show(
                    ui,
                    canvas,
                    &doc.view.camera,
                    &self.style,
                    self.theme,
                    editor,
                    doc.session.document().text_styles(),
                );
            }
        }
        self.state.frame_readout.finish(started);
        if let (true, Some(canvas)) = (perf.show_frame_time, output.rect) {
            let doc = &self.state.docs[self.state.active];
            let objects = doc.session.document().objects().len();
            let stats = doc.view.cache_stats();
            self.state
                .frame_readout
                .draw(ui, canvas, self.theme, stats, objects);
        }
        let doc = &mut self.state.docs[self.state.active];
        doc.dynamic_input.canvas = output.rect;
        doc.dynamic_input.cursor = output.preview_cursor.or(doc.dynamic_input.cursor);
        self.cursor = output.cursor;
        self.state.snap_now = output.snapped;
        self.click = output.click;
        self.double_click = output.double_click;
        self.region = output.select;
        self.prompted = output.prompted;
        self.picked = output.picked;
        self.path = output.path;
        self.pick_shift = output.pick_shift;
        self.grips = output.grips;
        self.prompt_enter = output.prompt_enter;
        self.snap_menu = output.snap_menu;
        self.context_menu = output.context_menu;
        self.refused = output.refused;
        self.placed_snap = output.placed_snap;
        let (viewport_edit, enter, leave) = (
            output.viewport_edit,
            output.enter_viewport,
            output.leave_viewport,
        );
        if let Some(outcome) = finished {
            self.state.finish_mtext_editor(outcome);
        }
        self.dynamic_overlay(ui);
        let view_name = match self.state.space() {
            opendraft_canvas::Space::Model => "Top",
            opendraft_canvas::Space::Paper(_) => "Layout",
            opendraft_canvas::Space::Viewport(..) => "Top (viewport)",
        };
        ui.put(
            egui::Rect::from_min_size(ui.min_rect().min, egui::vec2(220.0, 18.0)),
            egui::Label::new(
                egui::RichText::new(format!("[-][{view_name}][2D Wireframe]")).color(label_color),
            ),
        );
        if let Some(viewport) = viewport_edit {
            self.state.apply_viewport_edit(viewport);
        }
        if let Some(id) = enter {
            self.state.enter_viewport(id);
        }
        if leave {
            self.state.leave_viewport();
        }
    }

    /// The Model and layout tabs along the bottom of the drawing.
    fn layout_tabs(&mut self, ui: &mut egui::Ui) {
        let doc = &self.state.docs[self.state.active];
        let (document, space) = (&doc.session.document(), doc.view.space());
        let mut action = None;
        egui::TopBottomPanel::bottom("layout_tabs")
            .frame(
                egui::Frame::NONE
                    .fill(self.theme.colors.tab_strip())
                    .inner_margin(egui::Margin::symmetric(6, 2)),
            )
            .show_inside(ui, |ui| {
                action = crate::layout_tabs::show(ui, self.theme, document, space);
            });
        if let Some(action) = action {
            crate::layout_tabs::apply(self.state, action);
        }
    }

    fn layers(&mut self, ui: &mut egui::Ui) {
        let doc = &self.state.docs[self.state.active];
        let (document, token) = (doc.session.document(), doc.session.draw_token());
        let viewport = self.state.target_viewport_ref();
        if let Some(edit) = crate::layers_ui::show(ui, self.theme, document, token, viewport) {
            self.state.apply_layer_edit(edit);
        }
    }

    fn properties(&mut self, ui: &mut egui::Ui) {
        use crate::properties_ui::Edit;
        match crate::properties_ui::show(ui, self.theme, self.state) {
            Some(Edit::Value {
                field,
                label,
                value,
                kind,
            }) => self.state.set_property(field, label, value, kind),
            Some(Edit::Attr { edit, kind }) => self.state.set_attrs(&edit, kind),
            Some(Edit::Style(edit)) => self.state.set_object_style(&edit),
            Some(Edit::Attribute { tag, value, kind }) => {
                self.state.set_attribute_value(&tag, &value, kind);
            }
            None => {}
        }
    }
}
