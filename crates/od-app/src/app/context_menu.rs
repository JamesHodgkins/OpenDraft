//! The menu that a right click on the drawing opens when no command is
//! running: repeat, recent commands, the usual edits on what is selected,
//! and a few tools. Rows that have nothing behind them yet are greyed.

use eframe::egui;
use od_ui::icons;

use super::OpenDraftApp;

const WIDTH: f32 = 200.0;
const SUB_WIDTH: f32 = 170.0;
const ROW_HEIGHT: f32 = 24.0;
const ICON_COLUMN: f32 = 28.0;

/// What a row does when it is picked.
#[derive(Clone, Copy)]
enum Action {
    /// A registered command.
    Run(&'static str),
    SelectSimilar,
    DeselectAll,
}

enum Kind {
    Rule,
    Item(Action),
    /// A row with nothing behind it yet.
    Soon,
    Sub(Vec<Row>),
}

/// What one drawn column of rows reports.
struct Column {
    picked: Option<Action>,
    rect: egui::Rect,
    /// Where the submenu of the row the pointer is on should start.
    anchor: Option<egui::Pos2>,
}

struct Row {
    icon: Option<char>,
    label: String,
    kind: Kind,
}

impl Row {
    fn new(icon: Option<char>, label: impl Into<String>, kind: Kind) -> Self {
        Self {
            icon,
            label: label.into(),
            kind,
        }
    }

    fn rule() -> Self {
        Self::new(None, "", Kind::Rule)
    }

    fn run(icon: Option<char>, label: &str, id: &'static str) -> Self {
        Self::new(icon, label, Kind::Item(Action::Run(id)))
    }

    fn soon(icon: Option<char>, label: &str) -> Self {
        Self::new(icon, label, Kind::Soon)
    }

    fn sub(label: &str, rows: Vec<Row>) -> Self {
        Self::new(None, label, Kind::Sub(rows))
    }
}

fn soon_all(labels: &[&str]) -> Vec<Row> {
    labels.iter().map(|l| Row::soon(None, l)).collect()
}

impl OpenDraftApp {
    /// The rows for the menu as it is now: the editing rows only appear when
    /// something is selected.
    fn context_rows(&self) -> Vec<Row> {
        let selected = !self.state.selection().is_empty();
        let label_of = |id: &str| self.registry.get(id).map(|c| c.label);
        let repeat = match self.state.cmdline.last_command.and_then(label_of) {
            Some(label) => Row::new(
                Some(icons::CONTEXT_REPEAT),
                format!("Repeat {}", label.to_uppercase()),
                Kind::Item(Action::Run(
                    self.state.cmdline.last_command.unwrap_or_default(),
                )),
            ),
            None => Row::soon(Some(icons::CONTEXT_REPEAT), "Repeat"),
        };
        let recent: Vec<Row> = self
            .state
            .cmdline
            .recent
            .iter()
            .filter_map(|id| Some(Row::run(None, label_of(id)?, id)))
            .collect();
        let recent = if recent.is_empty() {
            vec![Row::soon(None, "(none)")]
        } else {
            recent
        };
        let mut rows = vec![
            repeat,
            Row::sub("Recent Input", recent),
            Row::rule(),
            Row::sub(
                "Clipboard",
                vec![
                    Row::run(None, "Cut", "edit.cut"),
                    Row::run(None, "Copy", "edit.copy_clip"),
                    Row::run(None, "Copy with Base Point", "edit.copy_base"),
                    Row::run(None, "Paste", "edit.paste"),
                    Row::run(None, "Paste as Block", "edit.paste_block"),
                    Row::run(None, "Paste to Original Coordinates", "edit.paste_original"),
                ],
            ),
        ];
        if selected {
            rows.extend([
                Row::rule(),
                Row::sub(
                    "Isolate",
                    soon_all(&["Isolate Objects", "Hide Objects", "End Object Isolation"]),
                ),
                Row::rule(),
                Row::run(Some(icons::CONTEXT_MODIFY_ERASE), "Erase", "modify.erase"),
                Row::run(Some(icons::MODIFY_MOVE), "Move", "modify.move"),
                Row::run(Some(icons::MODIFY_COPY), "Copy Selection", "modify.copy"),
                Row::run(Some(icons::MODIFY_SCALE), "Scale", "modify.scale"),
                Row::run(Some(icons::MODIFY_ROTATE), "Rotate", "modify.rotate"),
                Row::sub(
                    "Draw Order",
                    soon_all(&[
                        "Bring to Front",
                        "Send to Back",
                        "Bring Above Objects",
                        "Send Under Objects",
                    ]),
                ),
                Row::sub("Group", soon_all(&["Group", "Ungroup"])),
                Row::rule(),
                Row::new(
                    Some(icons::CONTEXT_SELECT_SIMILAR),
                    "Select Similar",
                    Kind::Item(Action::SelectSimilar),
                ),
                Row::new(
                    Some(icons::CONTEXT_DESELECT_ALL),
                    "Deselect All",
                    Kind::Item(Action::DeselectAll),
                ),
            ]);
        } else {
            rows.extend([
                Row::rule(),
                Row::run(Some(icons::EDIT_UNDO), "Undo", "edit.undo"),
                Row::run(Some(icons::EDIT_REDO), "Redo", "edit.redo"),
                Row::rule(),
                Row::run(
                    Some(icons::CONTEXT_EDIT_SELECT_ALL),
                    "Select All",
                    "edit.select_all",
                ),
                Row::run(
                    Some(icons::CONTEXT_VIEW_ZOOM_EXTENTS),
                    "Zoom Extents",
                    "view.zoom_extents",
                ),
            ]);
        }
        rows.extend([
            Row::rule(),
            Row::run(Some(icons::SELECT_POWER), "Power Select...", "select.power"),
            Row::soon(Some(icons::CONTEXT_QUICKCALC), "QuickCalc"),
            Row::soon(Some(icons::CONTEXT_FIND), "Find..."),
            Row::run(
                Some(icons::CONTEXT_VIEW_PANEL_PROPERTIES),
                "Properties",
                "view.panel_properties",
            ),
        ]);
        rows
    }

    /// Show the context menu where it was asked for and do what is picked.
    /// It closes on a pick, Esc, a press elsewhere, or when a command starts.
    pub(super) fn context_menu(&mut self, ctx: &egui::Context) {
        let Some((x, y)) = self.state.context_menu else {
            return;
        };
        let unfocused = ctx.input(|i| i.viewport().focused == Some(false));
        if self.state.command_running()
            || self.state.modal_open()
            || self.state.confirm.is_some()
            || unfocused
        {
            self.state.context_menu = None;
            return;
        }
        let rows = self.context_rows();
        let open_id = egui::Id::new("context_menu_open");
        let mut open: Option<usize> = ctx.memory(|m| m.data.get_temp(open_id)).flatten();
        let top = self.menu_column(
            ctx,
            "context_menu",
            egui::pos2(x, y),
            WIDTH,
            &rows,
            &mut open,
        );
        let (mut picked, mut area) = (top.picked, top.rect);
        if let (None, Some(anchor)) = (picked, top.anchor) {
            if let Some(Kind::Sub(inner)) = open.and_then(|o| rows.get(o)).map(|r| &r.kind) {
                let at = egui::pos2(anchor.x, anchor.y - 5.0);
                let sub = self.menu_column(ctx, "context_submenu", at, SUB_WIDTH, inner, &mut None);
                picked = sub.picked;
                area = area.union(sub.rect);
            }
        }
        ctx.memory_mut(|m| m.data.insert_temp(open_id, open));
        let outside = ctx.input(|i| i.pointer.any_pressed())
            && ctx.pointer_latest_pos().is_some_and(|p| !area.contains(p));
        let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if let Some(action) = picked {
            match action {
                Action::Run(id) => self.run(id),
                Action::SelectSimilar => self.state.select_similar(),
                Action::DeselectAll => self.state.docs[self.state.active].selection.clear(),
            }
        }
        if picked.is_some() || outside || escape {
            self.state.context_menu = None;
            ctx.memory_mut(|m| m.data.remove::<Option<usize>>(open_id));
        }
    }

    /// Draw one column of rows. Returns the action picked, the area the
    /// column covers, and the submenu row the pointer is on with where its
    /// submenu should start.
    fn menu_column(
        &self,
        ctx: &egui::Context,
        name: &str,
        at: egui::Pos2,
        width: f32,
        rows: &[Row],
        open: &mut Option<usize>,
    ) -> Column {
        let colors = &self.theme.colors;
        let mut picked = None;
        let mut anchor = None;
        let area = egui::Area::new(egui::Id::new(name))
            .order(egui::Order::Foreground)
            .fixed_pos(at)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .inner_margin(egui::Margin::symmetric(2, 4))
                    .show(ui, |ui| {
                        // Grow past the nominal width for a long label (Repeat ...).
                        let font = egui::FontId::proportional(self.theme.font_size_body);
                        let widest = rows
                            .iter()
                            .filter(|row| !matches!(row.kind, Kind::Rule))
                            .map(|row| {
                                ui.painter()
                                    .layout_no_wrap(
                                        row.label.to_string(),
                                        font.clone(),
                                        colors.text,
                                    )
                                    .size()
                                    .x
                            })
                            .fold(0.0_f32, f32::max);
                        let width = width.max(ICON_COLUMN + 6.0 + widest + 28.0);
                        ui.set_width(width);
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (i, row) in rows.iter().enumerate() {
                            if matches!(row.kind, Kind::Rule) {
                                ui.add_space(3.0);
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(width, 1.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().hline(
                                    rect.x_range(),
                                    rect.center().y,
                                    egui::Stroke::new(1.0_f32, colors.border),
                                );
                                ui.add_space(3.0);
                                continue;
                            }
                            let enabled = match row.kind {
                                Kind::Item(Action::Run(id)) => {
                                    self.registry.is_enabled(id, &self.state)
                                }
                                Kind::Item(_) | Kind::Sub(_) => true,
                                _ => false,
                            };
                            let sense = if enabled || matches!(row.kind, Kind::Sub(_)) {
                                egui::Sense::click()
                            } else {
                                egui::Sense::hover()
                            };
                            let (rect, response) =
                                ui.allocate_exact_size(egui::vec2(width, ROW_HEIGHT), sense);
                            let sub = matches!(row.kind, Kind::Sub(_));
                            let live = enabled || sub;
                            if response.hovered() && live {
                                ui.painter().rect_filled(rect, 2.0, colors.hover);
                            }
                            let color = if live {
                                colors.text
                            } else {
                                colors.text_disabled
                            };
                            if let Some(icon) = row.icon {
                                od_ui::icon_set::paint(
                                    ui.painter(),
                                    egui::pos2(rect.left() + ICON_COLUMN / 2.0, rect.center().y),
                                    egui::Align2::CENTER_CENTER,
                                    icon,
                                    14.0,
                                    if live {
                                        colors.text_weak
                                    } else {
                                        colors.text_disabled
                                    },
                                );
                            }
                            ui.painter().text(
                                egui::pos2(rect.left() + ICON_COLUMN + 6.0, rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                &row.label,
                                egui::FontId::proportional(self.theme.font_size_body),
                                color,
                            );
                            if sub {
                                od_ui::icon_set::paint(
                                    ui.painter(),
                                    egui::pos2(rect.right() - 10.0, rect.center().y),
                                    egui::Align2::CENTER_CENTER,
                                    icons::CONTEXT_SUBMENU,
                                    12.0,
                                    colors.text_weak,
                                );
                                if response.hovered() {
                                    *open = Some(i);
                                }
                                if *open == Some(i) {
                                    anchor = Some(egui::pos2(rect.right() + 2.0, rect.top()));
                                }
                            } else if response.hovered() && !name.ends_with("submenu") {
                                *open = None;
                            }
                            if let (true, Kind::Item(action)) = (response.clicked(), &row.kind) {
                                picked = Some(*action);
                            }
                        }
                    });
            });
        Column {
            picked,
            rect: area.response.rect,
            anchor,
        }
    }
}
