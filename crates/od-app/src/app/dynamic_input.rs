//! Keyboard ownership and submission for cursor-local entry.
use eframe::egui::{self, Event, Key, Modifiers};
use od_commands::{DynamicDimensions, GhostStyle, Prompt, Wants};

use super::{panels::PanelViewer, OpenDraftApp};
use crate::dynamic_input::{parse_field, Field};
use crate::state::AppState;

pub(super) fn field_id(uid: u64, field: Field) -> egui::Id {
    egui::Id::new(("dynamic_input", uid, field))
}

pub(super) fn owns_focus(ctx: &egui::Context, state: &AppState) -> bool {
    let doc = &state.docs[state.active];
    [
        Field::X,
        Field::Y,
        Field::Coordinate,
        Field::Distance,
        Field::Angle,
        Field::Number,
        Field::Text,
        Field::Keyword,
    ]
    .iter()
    .any(|field| ctx.memory(|m| m.has_focus(field_id(doc.uid, *field))))
}

impl AppState {
    pub(crate) fn commit_dynamic_input(&mut self) {
        let Some(ask) = self.dynamic_ask() else {
            return;
        };
        let units = self.units();
        // A distance alone, typed while the pointer follows a path, is
        // measured along the path from where it starts.
        let input = &self.docs[self.active].dynamic_input;
        let along = (ask.wants == Wants::Point
            && input.active == Some(Field::Distance)
            && input.locks.is_empty())
        .then(|| parse_field(Field::Distance, &input.draft, &units))
        .flatten()
        .and_then(|n| self.track_point_at(n))
        .map(|p| format!("#{},{}", p.x, p.y));
        let result = match along {
            Some(text) => self.answer_dynamic(&text),
            None => self.docs[self.active]
                .dynamic_input
                .submission(&ask, &self.settings.dynamic_input, &units)
                .map_err(str::to_owned)
                .and_then(|text| self.answer_dynamic(&text)),
        };
        match result {
            Ok(()) => self.docs[self.active].dynamic_input.clear(),
            Err(why) => {
                if self.cmdline.log.last() != Some(&why) {
                    self.cmdline.say(&why);
                }
                let input = &mut self.docs[self.active].dynamic_input;
                input.error = Some(why);
                input.request_focus = true;
            }
        }
    }

    pub(crate) fn dynamic_ask(&self) -> Option<Prompt> {
        self.waiting().cloned().or_else(|| {
            self.sysvar_prompt().map(|p| Prompt {
                wants: Wants::Text,
                message: p.text.into_owned(),
                keywords: Vec::new(),
                base: None,
                ghost: None,
                ghost_style: GhostStyle::Normal,
                rubber_band: true,
                marker: None,
                default_hint: None,
                allow_empty: false,
                pick_locked: false,
                mtext: None,
                mtext_of: None,
                overkill: None,
                dynamic_dimensions: DynamicDimensions::None,
            })
        })
    }

    pub(crate) fn idle_pointer_ask(&self) -> Option<Prompt> {
        (self.settings.dynamic_input.pointer_visibility == 2
            && self.settings.dynamic_input.pointer()
            && !self.command_running()
            && self.cmdline.sysvar.is_none())
        .then(|| Prompt {
            wants: Wants::Point,
            message: String::new(),
            keywords: Vec::new(),
            base: None,
            ghost: None,
            ghost_style: GhostStyle::Normal,
            rubber_band: true,
            marker: None,
            default_hint: None,
            allow_empty: false,
            pick_locked: false,
            mtext: None,
            mtext_of: None,
            overkill: None,
            dynamic_dimensions: DynamicDimensions::None,
        })
    }
}

impl OpenDraftApp {
    /// Remove navigation keys before egui derives its spatial focus movement.
    pub(super) fn capture_dynamic_keys(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        // With no command running, Up and Down browse the typed lines; egui
        // would move focus between the controls instead.
        if self.state.dynamic_ask().is_none()
            && !self.state.command_running()
            && !self.state.mtext_editing()
            && !self.state.modal_open()
            && self.state.confirm.is_none()
            && (!ctx.wants_keyboard_input()
                || ctx.memory(|m| m.has_focus(egui::Id::new("command_line_input"))))
        {
            raw.events.retain(|event| {
                if matches!(event, Event::Key { key: Key::ArrowDown | Key::ArrowUp, modifiers, .. }
                    if !modifiers.ctrl && !modifiers.alt && !modifiers.command)
                {
                    self.dynamic_keys.push(event.clone());
                    false
                } else {
                    true
                }
            });
        }
        if !self.state.settings.dynamic_input.enabled()
            || self.state.mtext_editing()
            || self.state.dynamic_ask().is_none()
            || self.state.modal_open()
            || self.state.confirm.is_some()
            || self.state.cmdline.focus
            || !self.state.cmdline.input.is_empty()
            || (ctx.wants_keyboard_input() && !owns_focus(ctx, &self.state))
        {
            return;
        }
        raw.events.retain(|event| {
            if matches!(event, Event::Key { key: Key::Tab | Key::ArrowDown | Key::ArrowUp | Key::Escape, modifiers, .. }
                if !modifiers.ctrl && !modifiers.alt && !modifiers.command) {
                self.dynamic_keys.push(event.clone());
                false
            } else { true }
        });
    }

    fn dynamic_f12(&mut self, ctx: &egui::Context) {
        let (pressed, released, now) = ctx.input_mut(|i| {
            let pressed = i.consume_key(Modifiers::NONE, Key::F12);
            (pressed, i.key_released(Key::F12), i.time)
        });
        if pressed && self.f12_pressed_at.is_none() {
            self.f12_pressed_at = Some(now);
        }
        self.dynamic_suppressed = self.f12_pressed_at.is_some_and(|at| now - at >= 0.35);
        if self.f12_pressed_at.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        if released {
            if let Some(at) = self.f12_pressed_at.take() {
                if now - at < 0.35 {
                    self.run("view.dynamic_input");
                }
            }
            self.dynamic_suppressed = false;
        }
    }

    pub(super) fn route_dynamic_input(&mut self, ctx: &egui::Context) -> bool {
        ctx.input_mut(|i| i.events.append(&mut self.dynamic_keys));
        self.dynamic_f12(ctx);
        let own_focus = owns_focus(ctx, &self.state);
        let unavailable = self.state.mtext_editing()
            || self.state.modal_open()
            || self.state.confirm.is_some()
            || !self.state.settings.dynamic_input.enabled()
            || self.state.cmdline.focus
            || !self.state.cmdline.input.is_empty()
            || (ctx.wants_keyboard_input() && !own_focus);
        if unavailable {
            self.state.docs[self.state.active].dynamic_input.clear();
            if own_focus {
                ctx.memory_mut(|m| m.surrender_focus(m.focused().unwrap_or(egui::Id::NULL)));
            }
            return false;
        }
        if self.dynamic_suppressed {
            return own_focus;
        }
        let Some(ask) = self.state.dynamic_ask() else {
            return false;
        };
        let settings = self.state.settings.dynamic_input.clone();
        let units = self.state.units();
        let prompt = self.state.cmd_prompt();
        let signature = format!(
            "{:?}:{:?}:{}:{:?}",
            ask.wants,
            ask.base,
            prompt.plain(),
            ask.dynamic_dimensions
        );
        // With several snap points under the pointer and no field being typed
        // in, Tab is the canvas's: it cycles the snap points.
        let doc = &mut self.state.docs[self.state.active];
        let cycles = doc.view.snap_cycles() && !doc.dynamic_input.editing;
        let input = &mut doc.dynamic_input;
        if input.signature != signature {
            input.clear();
            input.signature = signature;
            if own_focus {
                ctx.memory_mut(|m| {
                    if let Some(id) = m.focused() {
                        m.surrender_focus(id);
                    }
                });
            }
        }
        if input.canvas.is_none() || input.cursor.is_none() {
            return false;
        }
        let fields = input.fields(&ask, &settings);
        if input.draft.contains([',', '<']) || input.draft.starts_with(['#', '@']) {
            input.active = Some(Field::Coordinate);
            input.request_focus = input.editing;
        }
        let typed = ctx.input(|i| {
            i.events
                .iter()
                .any(|e| matches!(e, Event::Text(t) | Event::Paste(t) if !t.is_empty()))
        });
        if typed && !input.editing {
            input.begin(fields.first().copied().unwrap_or(Field::Keyword));
        }
        // A last character and Enter/Tab can arrive in one frame. Let TextEdit
        // apply that character before acting on the navigation key next frame.
        if typed {
            ctx.input_mut(|i| {
                i.events.retain(|event| {
                    if matches!(
                        event,
                        Event::Key {
                            key: Key::Enter | Key::Tab,
                            ..
                        }
                    ) {
                        self.dynamic_keys.push(event.clone());
                        false
                    } else {
                        true
                    }
                })
            });
            if !self.dynamic_keys.is_empty() {
                ctx.request_repaint();
            }
        }
        let mut submit = false;
        let mut option = None;
        let mut cancel = false;
        let mut why = None;
        ctx.input_mut(|i| {
            if i.consume_key(Modifiers::NONE, Key::Escape) {
                if input.menu {
                    input.menu = false;
                    input.request_focus = input.editing;
                } else if !input.draft.is_empty()
                    || !input.locks.is_empty()
                    || input.locked_coordinate.is_some()
                    || input.error.is_some()
                {
                    input.clear();
                } else {
                    cancel = true;
                }
            }
            if i.consume_key(Modifiers::NONE, Key::ArrowDown) && !prompt.options.is_empty() {
                if input.menu {
                    input.selected = (input.selected + 1) % prompt.options.len();
                }
                input.menu = true;
            }
            let up = i.consume_key(Modifiers::NONE, Key::ArrowUp);
            if input.menu && up {
                input.selected = (input.selected + prompt.options.len() - 1) % prompt.options.len();
            }
            let reverse = !cycles && i.consume_key(Modifiers::SHIFT, Key::Tab);
            if reverse || (!cycles && i.consume_key(Modifiers::NONE, Key::Tab)) {
                if input.draft.contains([',', '<']) || input.draft.starts_with(['#', '@']) {
                    if let Some(point) = input.constrained_point(&ask, &settings, &units) {
                        input.locked_coordinate = Some(point);
                        input.draft.clear();
                        input.begin(Field::Coordinate);
                    } else {
                        why = Some("Invalid coordinate expression.");
                    }
                } else if input.editing {
                    why = input.cycle(&fields, reverse, &units).err();
                } else {
                    // The first field already looks active, so Tab moves past it.
                    input.begin(fields.first().copied().unwrap_or(Field::Keyword));
                    why = input.cycle(&fields, reverse, &units).err();
                }
            }
            let space_submits = ask.wants != Wants::Text && !input.draft.contains([',', '<']);
            let space = space_submits && i.consume_key(Modifiers::NONE, Key::Space);
            if space {
                i.events
                    .retain(|e| !matches!(e, Event::Text(t) if t == " "));
            }
            if i.consume_key(Modifiers::NONE, Key::Enter) || space {
                if input.menu {
                    option = prompt.options.get(input.selected).cloned();
                } else {
                    submit = true;
                }
            }
        });
        if let Some(why) = why {
            input.error = Some(why.to_owned());
            self.state.cmdline.say(why);
        }
        if cancel {
            self.state.cancel();
        } else if let Some(option) = option {
            self.state.cmd_choose(option);
        } else if submit {
            self.state.commit_dynamic_input();
        }
        true
    }
}

impl PanelViewer<'_> {
    pub(super) fn prepare_dynamic_canvas(&mut self, ctx: &egui::Context) {
        let blocked = self.dynamic_suppressed
            || self.state.modal_open()
            || self.state.confirm.is_some()
            || !self.state.settings.dynamic_input.enabled()
            || (ctx.wants_keyboard_input() && !owns_focus(ctx, self.state));
        let ask = self.state.dynamic_ask();
        let settings = self.state.settings.dynamic_input.clone();
        let units = self.state.units();
        let doc = &mut self.state.docs[self.state.active];
        doc.dynamic_input.origin = ask.as_ref().and_then(|a| a.base).or(doc.view.last_point());
        let input = doc.dynamic_input.clone();
        let constraint = ask.filter(|_| !blocked).map(|ask| {
            std::sync::Arc::new(move |at| {
                let mut input = input.clone();
                input.cursor = Some(at);
                input
                    .constrained_point(&ask, &settings, &units)
                    .unwrap_or(at)
            }) as std::sync::Arc<dyn Fn(od_core::Point) -> od_core::Point + Send + Sync>
        });
        doc.view.set_cursor_constraint(constraint);
        doc.view.set_input_exclusions(if blocked {
            Vec::new()
        } else {
            doc.dynamic_input.hit_rects.clone()
        });
    }
}
