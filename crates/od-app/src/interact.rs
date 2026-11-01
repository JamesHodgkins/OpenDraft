//! Commands that ask the user through the `Editor`: starting one, feeding it
//! answers from the canvas and the command line, and keeping the canvas and
//! command line showing what it is waiting for.
//!
//! The command itself runs on its own thread (see `od_commands::Runner`);
//! this is the UI side of that conversation.

use std::borrow::Cow;
use std::sync::Arc;
use std::time::Duration;

use od_canvas::PointPrompt;
use od_commands::{
    Cancelled, Context, Editor, GhostStyle, Outcome, Prompt as Ask, Reply, Runner, Wants,
};
use od_core::Point;

use crate::cmdline::{CmdOption, Prompt};
use crate::coordinates::CoordinateMode;
use crate::prompt_reply::{
    reply_for_point, takes_clicked_points, typed_reply, typed_reply_with_mode,
};
use crate::state::AppState;
use crate::units_state::hint_text;

/// How long to wait, right after an answer, for the command's next prompt.
const NEXT_PROMPT_WAIT: Duration = Duration::from_millis(5);

impl AppState {
    /// Run `command` as the active command, replacing any running one. The
    /// selection at this moment is the command's pickset.
    pub fn start_command(
        &mut self,
        name: &str,
        command: impl FnOnce(&mut Editor) -> Result<(), Cancelled> + Send + 'static,
    ) {
        self.abort_command();
        self.end_sysvar_prompt();
        self.command_serial += 1;
        let pickset = self.docs[self.active].selection.iter().copied().collect();
        let context = Context::default()
            .with(Arc::clone(self.defaults()))
            .with(Arc::clone(&self.block_requests))
            .with(Arc::new(self.field_context()));
        self.runner = Some(Runner::start_with(name, pickset, context, command));
        self.run_doc = self.active;
        self.pump_command();
    }

    /// Whether a command started with `start_command` has not ended.
    pub fn command_running(&self) -> bool {
        self.runner.is_some()
    }

    /// Whether the running command is the one called `name`.
    pub fn is_running(&self, name: &str) -> bool {
        self.runner.as_ref().is_some_and(|r| r.name() == name)
    }

    /// Drop the running command without a word (another command is taking
    /// over). Its thread ends on its own.
    pub fn abort_command(&mut self) {
        if let Some(doc) = self.docs.get_mut(self.active) {
            doc.dynamic_input.clear();
        }
        self.once.clear();
        self.snap_menu = None;
        if let Some(mut runner) = self.runner.take() {
            runner.cancel();
            self.note_command_end(Outcome::Cancelled);
            let last = self.docs.len() - 1;
            self.docs[self.run_doc.min(last)].dynamic_input.clear();
            self.docs[self.run_doc.min(last)]
                .view
                .set_point_prompt(None);
        }
    }

    /// End the running command because something else is about to change
    /// the drawing. Says so, like Esc does.
    pub fn interrupt_command(&mut self) {
        self.cancel_command();
    }

    /// Esc in a running command: back out of a prompt that asked for that
    /// (a pick made from a dialog, which then opens again), else cancel it.
    pub(crate) fn esc_in_command(&mut self) {
        if self.waiting().is_some_and(|ask| ask.esc_backs) {
            self.cmdline.say("*Cancel*");
            self.reply(Reply::Back);
        } else {
            self.cancel_command();
        }
    }

    /// Esc: cancel the running command, as AutoCAD echoes it.
    pub fn cancel_command(&mut self) {
        if self.runner.is_some() {
            self.abort_command();
            self.cmdline.say("*Cancel*");
        }
    }

    /// Do the work the running command has asked for, and show what it asks
    /// next. Called every frame.
    pub fn pump_command(&mut self) {
        self.pump(Duration::ZERO);
    }

    /// As `pump_command`, but give a command that has only just started a
    /// moment to send its first prompt.
    pub(crate) fn pump_command_for(&mut self, patience: Duration) {
        self.pump(patience);
    }

    fn pump(&mut self, patience: Duration) {
        let Some(runner) = self.runner.as_mut() else {
            return;
        };
        if self.run_doc != self.active || self.run_doc >= self.docs.len() {
            return self.cancel_command();
        }
        let doc = &mut self.docs[self.run_doc];
        runner.poll_for(&mut doc.session, patience);
        doc.prune_selection();
        let said = runner.take_said();
        let selections = runner.take_selection_changes();
        match runner.outcome() {
            Some(outcome) => {
                doc.dynamic_input.clear();
                doc.view.set_point_prompt(None);
                self.once.clear();
                self.runner = None;
                self.note_command_end(outcome);
                match outcome {
                    Outcome::Completed => {}
                    Outcome::Cancelled => self.cmdline.say("*Cancel*"),
                    Outcome::Crashed => self.cmdline.say("The command stopped unexpectedly."),
                }
            }
            None => {
                let mut prompt = runner.prompt().and_then(canvas_prompt);
                // Picking an object for a field while the text editor is open.
                if prompt.is_none() && doc.mtext_editor.as_ref().is_some_and(|e| e.field_pick) {
                    prompt = Some(PointPrompt {
                        base: None,
                        ghost: None,
                        ghost_style: od_canvas::GhostStyle::Normal,
                        snap_geometry: Default::default(),
                        rubber_band: false,
                        marker: None,
                        pick: true,
                        pick_locked: false,
                    });
                }
                // After the base of `FRO` or the first of `M2P` the rubber
                // band starts there.
                if let (Some(p), Some(base)) = (prompt.as_mut(), self.once.rubber_base()) {
                    p.base = Some(base);
                }
                // A deferred perpendicular or tangent: the band starts at the
                // point it would be if the pointer were the next point.
                let deferred = self.once.deferred().zip(self.cursor);
                let at = deferred.and_then(|(d, cursor)| {
                    let entity = doc.session.document().get(d.object)?;
                    d.point(entity, cursor)
                });
                if let (Some(p), Some(at)) = (prompt.as_mut(), at) {
                    p.base = Some(at);
                }
                doc.view.set_point_prompt(prompt);
            }
        }
        for line in said {
            self.message.clone_from(&line);
            self.cmdline.say(line);
        }
        for (ids, remove) in selections {
            self.select_ids(ids, remove);
        }
        self.take_block_requests();
    }

    /// Wait for the running command to ask or end (tests).
    #[cfg(test)]
    pub fn settle_command(&mut self) {
        self.pump(Duration::from_secs(5));
    }

    pub(crate) fn waiting(&self) -> Option<&Ask> {
        self.runner.as_ref()?.prompt()
    }

    /// The running command's prompt, in the command line's terms.
    pub(crate) fn runner_prompt(&self) -> Option<Prompt> {
        let runner = self.runner.as_ref()?;
        let Some(ask) = runner.prompt() else {
            return Some(Prompt {
                command: runner.name().to_uppercase().into(),
                text: Cow::Borrowed(""),
                options: Vec::new(),
            });
        };
        let mut text = ask.message.clone();
        // A keyword-only question reads "Erase source objects? <No> [Yes/No]".
        if !ask.keywords.is_empty() && ask.wants != Wants::Keyword && !text.ends_with(" or") {
            text.push_str(" or");
        }
        if let Some(hint) = &ask.default_hint {
            text.push_str(&format!(" <{}>", hint_text(&self.units(), ask.wants, hint)));
        }
        Some(Prompt {
            command: runner.name().to_uppercase().into(),
            text: text.into(),
            options: ask
                .keywords
                .iter()
                .map(|k| CmdOption {
                    label: k.label.clone().into(),
                    key: k.key.clone().into(),
                })
                .collect(),
        })
    }

    /// The instruction for the status bar while a command is waiting.
    pub fn command_status(&self) -> Option<String> {
        let message = &self.waiting()?.message;
        Some(match self.once.status() {
            Some(once) => format!("{message} {once}"),
            None => message.clone(),
        })
    }

    /// Give the waiting prompt an answer and look for the next prompt. A
    /// point may only be one part of a `FRO` or `M2P`; and any answer spends
    /// the one-shot snap override it was given under.
    pub(crate) fn reply(&mut self, reply: Reply) {
        self.docs[self.active].dynamic_input.clear();
        let Some(reply) = self.intercept(reply) else {
            return;
        };
        // A deferred perpendicular or tangent is settled by this point: the
        // command gets the point it works out first, then this one.
        if let Some(first) = self.once.take_first() {
            self.send(Reply::Point(first));
            if !self.waiting().is_some_and(takes_clicked_points) {
                self.cmdline.say("That command takes no second point.");
                return;
            }
        }
        self.send(reply);
    }

    fn send(&mut self, reply: Reply) {
        if let Some(runner) = self.runner.as_mut() {
            let view = &mut self.docs[self.run_doc].view;
            if let Reply::Point(at) = &reply {
                view.set_last_point(*at);
            }
            runner.answer(reply);
            view.set_point_prompt(None);
        }
        self.pump(NEXT_PROMPT_WAIT);
    }

    /// A point placed on the canvas.
    pub fn answer_point(&mut self, at: Point) {
        let Some(ask) = self.waiting() else { return };
        if let Some(reply) = reply_for_point(ask.wants, ask.base, at) {
            if self.once.asking().is_none() {
                self.docs[self.active].view.set_last_point(at);
            }
            self.reply(reply);
        }
    }

    /// A click on the canvas while a command waits for an object: the object
    /// under it, if there is one.
    pub fn answer_object(&mut self, at: Point, picked: Option<od_core::EntityId>, shift: bool) {
        let Some(ask) = self.waiting() else { return };
        if ask.wants == Wants::Object {
            if let Some(id) = picked {
                self.docs[self.active].view.set_last_point(at);
                self.reply(Reply::Object { id, at, shift });
            }
        }
    }

    /// Enter (or a right click): accept the default, or the selection.
    pub fn answer_enter(&mut self) {
        let Some(ask) = self.waiting() else { return };
        if ask.wants == Wants::Selection {
            let allow_empty = ask.allow_empty;
            let ids: Vec<_> = self.selection().iter().copied().collect();
            if ids.is_empty() && !allow_empty {
                self.cmdline.say("Select objects first.");
                return;
            }
            self.reply(Reply::Selection(ids));
        } else {
            self.reply(Reply::Enter);
        }
    }

    /// Pick one of the waiting prompt's keywords (typed or clicked).
    pub fn answer_keyword(&mut self, label: &str) {
        self.reply(Reply::Keyword(label.to_owned()));
    }

    /// Answer the waiting prompt with typed text. Returns whether it was
    /// taken as an answer; if not, the text may be a command name.
    pub(crate) fn answer_typed(&mut self, text: &str) -> bool {
        let Some(ask) = self.waiting().cloned() else {
            return false;
        };
        if self.take_one_shot(&ask, text) {
            return true;
        }
        if let Some(point) = self.track_distance(&ask, text) {
            let line = format!("{} {}", self.cmd_prompt().plain(), text.trim());
            self.cmdline.say(line);
            self.reply(Reply::Point(point));
            return true;
        }
        let view = &self.docs[self.active].view;
        let origin = self
            .once
            .origin()
            .or(ask.base)
            .or(view.last_point())
            .unwrap_or(Point::new(0.0, 0.0));
        let Some(answer) = typed_reply(&ask, text, origin, self.cursor, &self.units()) else {
            return false;
        };
        let line = format!(
            "{} {}",
            self.runner_prompt().map(|p| p.plain()).unwrap_or_default(),
            text.trim()
        );
        self.cmdline.say(line);
        match answer {
            Ok(reply) => self.reply(reply),
            Err(why) => self.cmdline.say(why),
        }
        true
    }

    /// Cursor entry uses the same reply construction and echo as typed input.
    pub(crate) fn answer_dynamic(&mut self, text: &str) -> Result<(), String> {
        if self.cmdline.sysvar.is_some() {
            let before = self.cmdline.sysvar.clone();
            self.sysvar_submit(text);
            if self.cmdline.sysvar.is_some() && self.cmdline.sysvar == before {
                return Err(self.cmdline.log.last().cloned().unwrap_or_default());
            }
            return Ok(());
        }
        let ask = self
            .waiting()
            .cloned()
            .ok_or("No input prompt is active.")?;
        if self.take_one_shot(&ask, text) {
            return Ok(());
        }
        if let Some(point) = self.track_distance(&ask, text) {
            self.reply(Reply::Point(point));
            return Ok(());
        }
        // Enter at the cursor, as anywhere else, accepts what is selected: a
        // bare `Reply::Enter` would end a selection prompt as a cancel.
        if text.trim().is_empty() && ask.wants == Wants::Selection {
            self.answer_enter();
            return Ok(());
        }
        let origin = self
            .once
            .origin()
            .or(ask.base)
            .or(self.docs[self.active].view.last_point())
            .unwrap_or(Point::new(0.0, 0.0));
        let mode = CoordinateMode::DynamicPointer {
            default_relative: ask.base.is_some() && !self.settings.dynamic_input.pointer_absolute,
        };
        let reply = if text.trim().is_empty() {
            Ok(Reply::Enter)
        } else {
            typed_reply_with_mode(
                &ask,
                text,
                origin,
                self.docs[self.active].dynamic_input.cursor.or(self.cursor),
                mode,
                &self.units(),
            )
            .unwrap_or(Err("Invalid input for the current prompt."))
        };
        let prompt = self.cmd_prompt().plain();
        self.cmdline.say(format!("{prompt} {text}"));
        match reply {
            Ok(reply) => {
                self.reply(reply);
                Ok(())
            }
            Err(why) => {
                self.cmdline.say(why);
                Err(why.to_owned())
            }
        }
    }
}

/// What the canvas should do for a prompt, if it takes points.
fn canvas_prompt(ask: &Ask) -> Option<PointPrompt> {
    let takes_points = ask.wants == Wants::Object || takes_clicked_points(ask);
    takes_points.then(|| PointPrompt {
        base: ask.base,
        rubber_band: ask.rubber_band,
        pick: ask.wants == Wants::Object,
        pick_locked: ask.pick_locked,
        ghost: ask.ghost.clone(),
        snap_geometry: ask.snap_geometry.clone(),
        marker: ask.marker.clone(),
        ghost_style: match ask.ghost_style {
            GhostStyle::Normal => od_canvas::GhostStyle::Normal,
            GhostStyle::Window => od_canvas::GhostStyle::Window,
            GhostStyle::Crossing => od_canvas::GhostStyle::Crossing,
        },
    })
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use od_commands::Keyword;
    use od_core::{Entity, Units};

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn ask(wants: Wants, base: Option<Point>, keywords: &[(&str, &str)]) -> Ask {
        Ask {
            dynamic_dimensions: od_commands::DynamicDimensions::None,
            wants,
            message: "Prompt".to_owned(),
            keywords: keywords.iter().map(|(l, k)| Keyword::new(*l, *k)).collect(),
            base,
            ghost: None,
            ghost_style: GhostStyle::Normal,
            snap_geometry: Default::default(),
            rubber_band: true,
            marker: None,
            default_hint: None,
            mtext: None,
            mtext_of: None,
            overkill: None,
            attribute_definition: None,
            attribute_editor: None,
            block_dialog: None,
            field_dialog: None,
            allow_empty: false,
            pick_locked: false,
            esc_backs: false,
        }
    }

    #[test]
    fn points_are_typed_absolute_relative_or_polar() {
        let a = ask(Wants::Point, None, &[]);
        let origin = p(10.0, 10.0);
        let reply = |t| typed_reply(&a, t, origin, None, &Units::default());
        assert_eq!(reply("1,2"), Some(Ok(Reply::Point(p(1.0, 2.0)))));
        assert_eq!(reply("@3,4"), Some(Ok(Reply::Point(p(13.0, 14.0)))));
        assert_eq!(reply("what"), None);
        // A bare number needs a base point to measure from.
        assert_eq!(reply("5"), None);
    }

    #[test]
    fn a_bare_distance_follows_the_cursor_direction() {
        let a = ask(Wants::Point, Some(p(0.0, 0.0)), &[]);
        let no_direction = typed_reply(&a, "5", p(0.0, 0.0), None, &Units::default());
        assert!(matches!(no_direction, Some(Err(_))));
        let up = typed_reply(&a, "5", p(0.0, 0.0), Some(p(0.0, 100.0)), &Units::default());
        assert_eq!(up, Some(Ok(Reply::Point(p(0.0, 5.0)))));
    }

    #[test]
    fn distances_and_angles_come_from_numbers_or_points() {
        let base = Some(p(0.0, 0.0));
        let d = ask(Wants::Distance, base, &[]);
        assert_eq!(
            typed_reply(&d, "7", p(0.0, 0.0), None, &Units::default()),
            Some(Ok(Reply::Number(7.0)))
        );
        assert_eq!(
            typed_reply(&d, "3,4", p(0.0, 0.0), None, &Units::default()),
            Some(Ok(Reply::Number(5.0)))
        );
        let a = ask(Wants::Angle, base, &[]);
        // Typed angles are degrees; replies are radians.
        let Some(Ok(Reply::Number(r))) =
            typed_reply(&a, "90", p(0.0, 0.0), None, &Units::default())
        else {
            panic!("expected an angle");
        };
        assert!((r - FRAC_PI_2).abs() < 1e-12);
        assert_eq!(
            reply_for_point(Wants::Angle, base, p(0.0, 5.0)),
            Some(Reply::Number(FRAC_PI_2))
        );
    }

    #[test]
    fn typed_lengths_and_angles_use_the_drawings_units() {
        let units = Units {
            length: od_core::LengthFormat::Architectural,
            angle: od_core::AngleFormat::Gradians,
            ..Units::default()
        };
        let base = Some(p(0.0, 0.0));
        let d = ask(Wants::Distance, base, &[]);
        assert_eq!(
            typed_reply(&d, "5'-3 1/2\"", p(0.0, 0.0), None, &units),
            Some(Ok(Reply::Number(63.5)))
        );
        let a = ask(Wants::Angle, base, &[]);
        let Some(Ok(Reply::Number(r))) = typed_reply(&a, "100", p(0.0, 0.0), None, &units) else {
            panic!("expected an angle");
        };
        assert!((r - FRAC_PI_2).abs() < 1e-12, "a bare angle is in grads");
        let n = ask(Wants::Number, None, &[]);
        assert_eq!(typed_reply(&n, "5'", p(0.0, 0.0), None, &units), None);
    }

    #[test]
    fn keywords_match_by_key_or_label_start() {
        let a = ask(Wants::Point, None, &[("Undo", "U"), ("Exit", "E")]);
        let origin = p(0.0, 0.0);
        let pick = |t| typed_reply(&a, t, origin, None, &Units::default());
        assert_eq!(pick("u"), Some(Ok(Reply::Keyword("Undo".into()))));
        assert_eq!(pick("exi"), Some(Ok(Reply::Keyword("Exit".into()))));
        assert_eq!(pick("zzz"), None);
    }

    #[test]
    fn a_selection_prompt_does_not_take_typed_points() {
        let a = ask(Wants::Selection, None, &[]);
        assert_eq!(
            typed_reply(&a, "1,2", p(0.0, 0.0), None, &Units::default()),
            None
        );
    }

    fn circle(x: f64) -> Entity {
        Entity::Circle {
            center: p(x, 0.0),
            radius: 1.0,
        }
    }

    fn state_with_two_circles() -> AppState {
        let mut state = AppState::new();
        state.add_entity(circle(0.0));
        state.add_entity(circle(10.0));
        state
    }

    /// A command that asks for a whole selection and says how many it got.
    fn pick_objects(ed: &mut Editor) -> Result<(), Cancelled> {
        let ids = ed.get_selection("Select objects")?;
        ed.say(format!("{} chosen", ids.len()));
        Ok(())
    }

    #[test]
    fn enter_at_the_cursor_accepts_the_selection_instead_of_cancelling_the_prompt() {
        let mut state = state_with_two_circles();
        state.start_command("Pick", pick_objects);
        state.settle_command();
        state.select_all();
        // Dynamic input at the pointer submits an empty line for Enter.
        assert_eq!(state.answer_dynamic(""), Ok(()));
        state.settle_command();
        assert!(!state.command_running());
        let log = &state.cmdline.log;
        assert!(log.iter().any(|l| l == "2 chosen"), "{log:?}");
        assert!(!log.iter().any(|l| l == "*Cancel*"), "{log:?}");
    }

    #[test]
    fn enter_at_the_cursor_with_nothing_selected_keeps_asking() {
        let mut state = state_with_two_circles();
        state.start_command("Pick", pick_objects);
        state.settle_command();
        assert_eq!(state.answer_dynamic(""), Ok(()));
        state.settle_command();
        assert!(state.command_running(), "still waiting for objects");
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l == "Select objects first."));
        state.cancel_command();
    }

    #[test]
    fn a_command_asks_through_the_canvas_and_the_command_line() {
        let mut state = state_with_two_circles();
        state.select_all();
        state.start_command("Move", crate::modify::move_objects);
        state.settle_command();

        let prompt = state.cmd_prompt();
        assert_eq!(prompt.command, "MOVE");
        assert_eq!(prompt.text, "Specify base point or");
        assert!(state.docs[0].view.is_prompting());

        state.answer_point(p(1.0, 1.0));
        state.settle_command();
        assert_eq!(
            state.cmd_prompt().text,
            "Specify second point of move or <use first point as displacement>"
        );
        state.cmd_submit("@3,4");
        state.settle_command();

        assert!(!state.command_running());
        assert!(!state.docs[0].view.is_prompting());
        let objects = state.session().document().objects();
        assert_eq!(
            objects[0].entity,
            Entity::Circle {
                center: p(3.0, 4.0),
                radius: 1.0
            }
        );
        assert_eq!(objects[1].entity.bounds().min.x, 12.0);
        assert!(state.message.contains("Moved 2 objects"));
        state.undo();
        assert_eq!(state.session().document().objects()[0].entity, circle(0.0));
    }

    #[test]
    fn escape_cancels_a_waiting_command_and_leaves_the_drawing_alone() {
        let mut state = state_with_two_circles();
        state.select_all();
        state.start_command("Move", crate::modify::move_objects);
        state.settle_command();
        state.answer_point(p(0.0, 0.0));
        state.settle_command();
        assert!(state.can_cancel());

        state.cancel();
        assert!(!state.command_running());
        assert!(!state.docs[0].view.is_prompting());
        assert_eq!(state.session().document().objects()[0].entity, circle(0.0));
        assert_eq!(state.cmdline.log.last().unwrap(), "*Cancel*");
    }

    #[test]
    fn with_nothing_selected_the_command_asks_for_objects_and_enter_accepts() {
        let mut state = state_with_two_circles();
        state.start_command("Move", crate::modify::move_objects);
        state.settle_command();
        assert_eq!(state.cmd_prompt().text, "Select objects");
        assert!(!state.docs[0].view.is_prompting(), "selecting, not placing");

        state.answer_enter();
        assert_eq!(state.cmdline.log.last().unwrap(), "Select objects first.");
        let first = state.session().document().objects()[0].id;
        state.select_ids([first], false);
        state.cmd_submit("");
        state.settle_command();
        assert_eq!(state.cmd_prompt().text, "Specify base point or");
    }

    #[test]
    fn enter_at_the_second_point_uses_the_base_as_displacement() {
        let mut state = state_with_two_circles();
        state.select_all();
        state.start_command("Move", crate::modify::move_objects);
        state.settle_command();
        state.cmd_submit("5,6");
        state.settle_command();
        state.cmd_submit("");
        state.settle_command();
        assert_eq!(
            state.session().document().objects()[0].entity,
            Entity::Circle {
                center: p(5.0, 6.0),
                radius: 1.0
            }
        );
    }

    #[test]
    fn a_new_command_replaces_the_running_one() {
        let mut state = state_with_two_circles();
        state.select_all();
        state.start_command("Move", crate::modify::move_objects);
        state.settle_command();
        state.start_command("Line", crate::draw::draw_line);
        assert!(state.is_running("Line"));
        assert!(!state.is_running("Move"));
    }
}
