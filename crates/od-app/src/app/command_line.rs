//! The command line panel: log, prompt with clickable options, input and the
//! suggestion list.

use eframe::egui;
use od_ui::{Colors, Theme};

use crate::cmdline::Suggestion;

use super::panels::PanelViewer;

/// Space the input bar extends above the row it holds.
const INPUT_BAR_PAD: f32 = 4.0;

impl PanelViewer<'_> {
    /// The command line: a scrolling log above one input row that shows the
    /// current prompt (with clickable options) and the text being typed.
    pub(super) fn command_line(&mut self, ui: &mut egui::Ui) {
        use crate::cmdline::{CmdOption, ALIASES};

        let theme = self.theme;
        let input_id = egui::Id::new("command_line_input");
        // Commands that can run now, for filtering suggestions.
        let runnable: Vec<&str> = ALIASES
            .iter()
            .map(|a| a.2)
            .filter(|id| self.registry.is_enabled(id, &*self.state))
            .collect();
        let enabled = |id: &str| runnable.contains(&id);

        let prompt = self.state.cmd_prompt();
        // Typed text keeps its spaces; everywhere else a space is Enter.
        let wants_text = self
            .state
            .dynamic_ask()
            .is_some_and(|ask| ask.wants == od_commands::Wants::Text);
        let had_focus = ui.memory(|m| m.has_focus(input_id));
        let requested_focus = std::mem::take(&mut self.state.cmdline.focus);
        if requested_focus {
            ui.memory_mut(|m| m.request_focus(input_id));
        }
        let owns_keyboard = had_focus || requested_focus;
        let mut chosen: Option<CmdOption> = None;
        let mut input = std::mem::take(&mut self.state.cmdline.input);

        // Arrow keys and Tab drive the suggestion list while typing.
        if owns_keyboard {
            let count = self.state.cmdline.listed;
            if count > 0 {
                let cl = &mut self.state.cmdline;
                ui.input_mut(|i| {
                    if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                        cl.selected = (cl.selected + 1) % count;
                    }
                    if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                        cl.selected = (cl.selected + count - 1) % count;
                    }
                });
            }
        }
        let mut submit = false;
        let mut accept_tab = false;
        let mut escape = false;
        // Judged on the whole panel, before the margin: short panels (collapsed or
        // dragged small) show only the input row.
        let expanded = ui.available_height() > 90.0;
        let mut text_rect = egui::Rect::NOTHING;
        let mut row_rect = egui::Rect::NOTHING;
        // The input line sits on a darker bar than the log above it, which
        // keeps the blue prompt and option letters readable. Reserve its place
        // in the paint order now and fill it once the row has been laid out.
        let panel = ui.max_rect();
        let bar = ui.painter().add(egui::Shape::Noop);
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(8, 4))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let row_height = ui.spacing().interact_size.y.max(22.0);
                let log_height = (ui.available_height() - row_height - 6.0).max(0.0);
                if expanded {
                    egui::ScrollArea::vertical()
                        .max_height(log_height)
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            for line in &self.state.cmdline.log {
                                ui.label(
                                    egui::RichText::new(line)
                                        .size(theme.font_size_small)
                                        .color(theme.colors.text_weak),
                                );
                            }
                        });
                }
                // Collapsed: keep the input line at the bottom of the panel, where it
                // sits when expanded, so toggling does not move it.
                if !expanded {
                    let spare = ui.available_height() - row_height - 2.0;
                    if spare > 0.0 {
                        ui.add_space(spare);
                    }
                }
                row_rect = ui
                    .horizontal(|ui| {
                        let (icon, tip) = if expanded {
                            (
                                od_ui::icons::COMMAND_LINE_COLLAPSE,
                                "Collapse the command history",
                            )
                        } else {
                            (
                                od_ui::icons::COMMAND_LINE_EXPAND,
                                "Show the command history",
                            )
                        };
                        let chevron = ui
                            .add(
                                od_ui::icon_set::Icon::new(icon, 13.0)
                                    .clickable()
                                    .color(theme.colors.text_weak),
                            )
                            .on_hover_text(tip)
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        if chevron.clicked() {
                            self.state.cmdline.resize = Some(expanded);
                        }
                        if !prompt.command.is_empty() {
                            ui.label(
                                egui::RichText::new(prompt.command.as_ref())
                                    .strong()
                                    .color(theme.colors.accent_text),
                            );
                        }
                        // With no options the colon joins the text directly.
                        let text = if prompt.options.is_empty() && !prompt.command.is_empty() {
                            format!("{}:", prompt.text)
                        } else {
                            prompt.text.to_string()
                        };
                        ui.label(egui::RichText::new(text).color(theme.colors.text));
                        if !prompt.options.is_empty() {
                            ui.label(egui::RichText::new("[").color(theme.colors.text_weak));
                            for option in &prompt.options {
                                if option_chip(ui, &theme.colors, option).clicked() {
                                    chosen = Some(option.clone());
                                }
                            }
                            ui.label(egui::RichText::new("]").color(theme.colors.text_weak));
                        }
                        if !prompt.command.is_empty() && !prompt.options.is_empty() {
                            ui.label(egui::RichText::new(":").color(theme.colors.text_weak));
                        }
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut input)
                                .id(input_id)
                                .frame(false)
                                .lock_focus(true)
                                .desired_width(f32::INFINITY),
                        );
                        text_rect = response.rect;
                        let (enter, tab, esc) = ui.input(|i| {
                            (
                                i.key_pressed(egui::Key::Enter),
                                i.key_pressed(egui::Key::Tab),
                                i.key_pressed(egui::Key::Escape),
                            )
                        });
                        if response.changed() {
                            self.state.cmdline.selected = 0;
                            // Space acts like Enter, except inside "x, y" or "d<a",
                            // or when text is being typed.
                            submit |= !wants_text
                                && input.ends_with(' ')
                                && !input.contains(',')
                                && !input.contains('<');
                        }
                        submit |= owns_keyboard && enter;
                        accept_tab = owns_keyboard && tab;
                        escape = (owns_keyboard || response.has_focus()) && esc;
                    })
                    .response
                    .rect;
            });
        let bar_rect = egui::Rect::from_min_max(
            egui::pos2(panel.left(), row_rect.top() - INPUT_BAR_PAD),
            panel.right_bottom(),
        );
        ui.painter().set(
            bar,
            egui::Shape::rect_filled(bar_rect, 0.0, theme.colors.input_bg),
        );

        let suggestions = self.state.cmd_suggestions(&input, enabled);
        self.state.cmdline.listed = suggestions.len();
        let selected = self
            .state
            .cmdline
            .selected
            .min(suggestions.len().saturating_sub(1));
        let mut picked: Option<&'static str> = None;

        if accept_tab {
            if let Some(s) = suggestions.get(selected) {
                input = s.alias.to_owned();
            }
        }
        if escape {
            if input.is_empty() {
                picked = Some("edit.cancel");
            }
            input.clear();
        }
        if submit && !escape {
            let text = std::mem::take(&mut input);
            picked = match suggestions.get(selected) {
                Some(s) => Some(s.command),
                None => self.state.cmd_submit(&text),
            };
            self.state.cmdline.focus = true;
        }
        if let Some(option) = chosen {
            self.state.cmd_choose(option);
            self.state.cmdline.focus = true;
        }

        // Suggestion list above the input row. Pressing a row takes focus from
        // the input, so the list stays up while the pointer is on it or the
        // click would never land.
        let popup_id = egui::Id::new("command_suggestions");
        let on_popup = ui.ctx().memory(|m| m.area_rect(popup_id)).is_some_and(|r| {
            ui.input(|i| i.pointer.interact_pos())
                .is_some_and(|p| r.contains(p))
        });
        if (owns_keyboard || on_popup) && !suggestions.is_empty() && !submit {
            if let Some(command) =
                suggestion_popup(ui.ctx(), theme, &suggestions, selected, panel, text_rect)
            {
                picked = Some(command);
                input.clear();
                self.state.cmdline.focus = true;
            }
        }

        self.state.cmdline.input = input;
        if picked.is_some() {
            self.run = picked;
        }
    }
}

/// An option in the prompt: its typed key is highlighted, like AutoCAD.
fn option_chip(
    ui: &mut egui::Ui,
    colors: &Colors,
    option: &crate::cmdline::CmdOption,
) -> egui::Response {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let mut job = egui::text::LayoutJob::default();
    let split = option.key.len().min(option.label.len());
    let (key, rest) = option.label.split_at(split);
    let format = |color| egui::TextFormat {
        font_id: font.clone(),
        color,
        ..Default::default()
    };
    job.append(key, 0.0, format(colors.accent_text));
    job.append(rest, 0.0, format(colors.text));
    ui.add(egui::Label::new(job).sense(egui::Sense::click()))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The suggestion list floating above the input row. Returns the command
/// clicked, if any.
fn suggestion_popup(
    ctx: &egui::Context,
    theme: &Theme,
    suggestions: &[Suggestion],
    selected: usize,
    panel_rect: egui::Rect,
    text_rect: egui::Rect,
) -> Option<&'static str> {
    let mut clicked = None;
    egui::Area::new(egui::Id::new("command_suggestions"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(panel_rect.left(), text_rect.top() - 6.0))
        .pivot(egui::Align2::LEFT_BOTTOM)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .fill(theme.colors.panel_raised)
                .inner_margin(egui::Margin::same(2))
                .show(ui, |ui| {
                    // An Area starts with the viewport's available width. Give
                    // its contents an exact width so rows cannot make the popup
                    // wider than the dock panel. Account for the frame's two
                    // 2 px horizontal margins so its outer edges line up too.
                    ui.set_width((panel_rect.width() - 4.0).max(0.0));
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (i, s) in suggestions.iter().enumerate() {
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), 22.0),
                            egui::Sense::click(),
                        );
                        if i == selected {
                            ui.painter().rect_filled(rect, 2.0, theme.colors.accent);
                        } else if response.hovered() {
                            ui.painter().rect_filled(rect, 2.0, theme.colors.hover);
                        }
                        let color = if i == selected {
                            theme.colors.text_on_accent
                        } else {
                            theme.colors.text
                        };
                        ui.painter().text(
                            rect.left_center() + egui::vec2(8.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            s.label(),
                            egui::FontId::proportional(theme.font_size_body),
                            color,
                        );
                        if response.clicked() {
                            clicked = Some(s.command);
                        }
                    }
                });
        });
    clicked
}
