//! Runs a command on its own thread and trades requests with the UI thread.
//!
//! The command is ordinary straight-line code that asks and waits (see
//! [`Editor`]). The UI thread owns the drawing: each frame it calls
//! [`Runner::poll`], which does the drawing work the command asked for and
//! stops at the next prompt for the UI to answer.

use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread::JoinHandle;
use std::time::Duration;

use od_core::EntityId;

use crate::editor::Request;
use crate::prompt::{Cancelled, Prompt, Reply};
use crate::{Editor, Session};

/// How a command ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Completed,
    Cancelled,
    /// The command's thread panicked.
    Crashed,
}

enum State {
    /// The command is working, or about to send its next request.
    Running,
    Waiting(Prompt),
    Finished(Outcome),
}

/// A command in progress.
pub struct Runner {
    name: String,
    requests: Receiver<Request>,
    /// Dropped to cancel: the command's next wait then fails with `Cancelled`.
    replies: Option<Sender<Reply>>,
    state: State,
    said: Vec<String>,
    selections: Vec<(Vec<EntityId>, bool)>,
    thread: Option<JoinHandle<()>>,
}

impl Runner {
    /// Start `command` on its own thread. `pickset` is the selection at the
    /// moment of launch.
    pub fn start(
        name: impl Into<String>,
        pickset: Vec<EntityId>,
        command: impl FnOnce(&mut Editor) -> Result<(), Cancelled> + Send + 'static,
    ) -> Self {
        let name = name.into();
        let (request_tx, requests) = channel();
        let (replies, reply_rx) = channel();
        let thread = std::thread::Builder::new()
            .name(format!("command {name}"))
            .spawn(move || {
                let mut editor = Editor::new(request_tx, reply_rx, pickset);
                let result = command(&mut editor);
                editor.finish(result);
            })
            .ok();
        Self {
            name,
            requests,
            replies: Some(replies),
            state: if thread.is_some() {
                State::Running
            } else {
                State::Finished(Outcome::Crashed)
            },
            said: Vec::new(),
            selections: Vec::new(),
            thread,
        }
    }

    /// The command's name, as launched.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Whether the command has not ended yet.
    pub fn is_active(&self) -> bool {
        !matches!(self.state, State::Finished(_))
    }

    /// The prompt waiting for an answer.
    pub fn prompt(&self) -> Option<&Prompt> {
        match &self.state {
            State::Waiting(prompt) => Some(prompt),
            _ => None,
        }
    }

    /// How the command ended, once it has.
    pub fn outcome(&self) -> Option<Outcome> {
        match self.state {
            State::Finished(outcome) => Some(outcome),
            _ => None,
        }
    }

    /// Feedback the command has said since the last call.
    pub fn take_said(&mut self) -> Vec<String> {
        std::mem::take(&mut self.said)
    }

    /// Selection changes the command has asked for since the last call, in
    /// order: the ids and whether they are removed (else added). The UI owns
    /// the selection, so it applies these.
    pub fn take_selection_changes(&mut self) -> Vec<(Vec<EntityId>, bool)> {
        std::mem::take(&mut self.selections)
    }

    /// Answer the waiting prompt. Ignored when nothing is waiting.
    pub fn answer(&mut self, reply: Reply) {
        if !matches!(self.state, State::Waiting(_)) {
            return;
        }
        if reply == Reply::Cancel {
            self.cancel();
            return;
        }
        if let Some(replies) = &self.replies {
            if replies.send(reply).is_ok() {
                self.state = State::Running;
            }
        }
    }

    /// Abandon the command (Esc). Its next wait fails with `Cancelled`, so a
    /// transaction in progress reverts; `poll` reports the end.
    pub fn cancel(&mut self) {
        self.replies = None;
        if matches!(self.state, State::Waiting(_)) {
            self.state = State::Running;
        }
    }

    /// Do the drawing work the command has asked for and stop at its next
    /// prompt or its end. Never blocks.
    pub fn poll(&mut self, session: &mut Session) {
        self.pump(session, Some(Duration::ZERO));
    }

    /// Like `poll`, but gives the command up to `patience` to reach its next
    /// request. Used right after answering, so a quick command's next prompt
    /// is there in the same frame.
    pub fn poll_for(&mut self, session: &mut Session, patience: Duration) {
        self.pump(session, Some(patience));
    }

    /// Like `poll`, but waits for the command to ask or end. For tests and
    /// headless runs.
    pub fn wait(&mut self, session: &mut Session) {
        self.pump(session, None);
    }

    /// `patience` is how long to wait for each request; `None` waits as long
    /// as it takes.
    fn pump(&mut self, session: &mut Session, patience: Option<Duration>) {
        while matches!(self.state, State::Running) {
            let request = match patience {
                None => self.requests.recv().map_err(|_| TryRecvError::Disconnected),
                Some(Duration::ZERO) => self.requests.try_recv(),
                Some(patience) => self.requests.recv_timeout(patience).map_err(|e| match e {
                    RecvTimeoutError::Timeout => TryRecvError::Empty,
                    RecvTimeoutError::Disconnected => TryRecvError::Disconnected,
                }),
            };
            match request {
                Ok(Request::Run(job)) => {
                    // The job is the command's code running on our thread. A
                    // panic in it must end that command, not the application.
                    let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        job(session);
                    }));
                    if ran.is_err() {
                        self.replies = None;
                        self.state = State::Finished(Outcome::Crashed);
                        self.join();
                    }
                }
                Ok(Request::Say(text)) => self.said.push(text),
                Ok(Request::Select { ids, remove }) => self.selections.push((ids, remove)),
                Ok(Request::Ask(prompt)) => self.state = State::Waiting(prompt),
                Ok(Request::Done(result)) => {
                    self.state = State::Finished(match result {
                        Ok(()) => Outcome::Completed,
                        Err(Cancelled) => Outcome::Cancelled,
                    });
                    self.join();
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.state = State::Finished(Outcome::Crashed);
                    self.join();
                }
            }
        }
    }

    fn join(&mut self) {
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        // Let a command still waiting for an answer end, so its thread does.
        self.replies = None;
    }
}

/// What a scripted run asked and how it ended.
#[derive(Debug, PartialEq)]
pub struct Transcript {
    pub outcome: Outcome,
    /// The message of every prompt, in order.
    pub asked: Vec<String>,
    /// Everything the command said.
    pub said: Vec<String>,
}

/// Run `command` to the end against `session`, answering its prompts from
/// `answers` in order. When the answers run out the next prompt is cancelled.
/// For tests and headless use.
pub fn run_scripted(
    session: &mut Session,
    pickset: Vec<EntityId>,
    answers: impl IntoIterator<Item = Reply>,
    command: impl FnOnce(&mut Editor) -> Result<(), Cancelled> + Send + 'static,
) -> Transcript {
    let mut answers = answers.into_iter();
    let mut runner = Runner::start("script", pickset, command);
    let mut asked = Vec::new();
    let mut said = Vec::new();
    loop {
        runner.wait(session);
        said.extend(runner.take_said());
        if let Some(outcome) = runner.outcome() {
            return Transcript {
                outcome,
                asked,
                said,
            };
        }
        if let Some(prompt) = runner.prompt() {
            asked.push(prompt.message.clone());
        }
        runner.answer(answers.next().unwrap_or(Reply::Cancel));
    }
}

#[cfg(test)]
mod tests {
    use od_core::{Entity, Point};

    use super::*;
    use crate::{Answer, Options, PointOptions};

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn circle_at(x: f64) -> Entity {
        Entity::Circle {
            center: p(x, 0.0),
            radius: 1.0,
        }
    }

    /// A session with two circles, and their ids.
    fn two_circles() -> (Session, Vec<EntityId>) {
        let mut session = Session::default();
        let ids = (0..2)
            .map(|i| session.document.add(circle_at(f64::from(i) * 10.0)))
            .collect();
        (session, ids)
    }

    /// The Move command, written the way commands will be.
    fn move_command(ed: &mut Editor) -> Result<(), Cancelled> {
        let ids = ed.pickset_or_select("Select objects")?;
        let base = ed.get_point("Specify base point")?;
        let to = ed.get_point(PointOptions::new("Specify second point").base(base))?;
        let (dx, dy) = (to.x - base.x, to.y - base.y);
        ed.transaction("Move", move |tx| {
            for id in ids {
                tx.modify(id, |e| e.translated(dx, dy));
            }
            Ok(())
        })?;
        ed.say("Moved");
        Ok(())
    }

    #[test]
    fn a_command_asks_waits_and_edits() {
        let (mut session, ids) = two_circles();
        let answers = [Reply::Point(p(1.0, 1.0)), Reply::Point(p(4.0, 5.0))];
        let transcript = run_scripted(&mut session, ids.clone(), answers, move_command);

        assert_eq!(transcript.outcome, Outcome::Completed);
        assert_eq!(
            transcript.asked,
            ["Specify base point", "Specify second point"],
            "the pickset made the selection prompt unnecessary"
        );
        assert_eq!(transcript.said, ["Moved"]);
        let at = |x, y| Entity::Circle {
            center: p(x, y),
            radius: 1.0,
        };
        assert_eq!(session.document.objects()[0].entity, at(3.0, 4.0));
        assert_eq!(session.document.objects()[1].entity, at(13.0, 4.0));
        // Both objects moved in one undo step.
        assert_eq!(session.undo().unwrap().name, "Move");
        assert_eq!(session.document.objects()[0].entity, circle_at(0.0));
        assert!(!session.history.can_undo());
    }

    #[test]
    fn with_nothing_preselected_the_command_asks_for_a_selection() {
        let (mut session, ids) = two_circles();
        let answers = [
            Reply::Selection(vec![ids[1]]),
            Reply::Point(p(0.0, 0.0)),
            Reply::Point(p(0.0, 7.0)),
        ];
        let transcript = run_scripted(&mut session, vec![], answers, move_command);
        assert_eq!(transcript.asked[0], "Select objects");
        assert_eq!(transcript.outcome, Outcome::Completed);
        assert_eq!(session.document.objects()[0].entity, circle_at(0.0));
        assert_eq!(
            session.document.objects()[1].entity,
            Entity::Circle {
                center: p(10.0, 7.0),
                radius: 1.0
            }
        );
    }

    #[test]
    fn cancelling_at_a_prompt_leaves_no_trace() {
        let (mut session, ids) = two_circles();
        let before = session.document.objects().to_vec();
        let transcript = run_scripted(
            &mut session,
            ids,
            [Reply::Point(p(0.0, 0.0)), Reply::Cancel],
            move_command,
        );
        assert_eq!(transcript.outcome, Outcome::Cancelled);
        assert_eq!(session.document.objects(), before);
        assert!(!session.history.can_undo());
    }

    #[test]
    fn running_out_of_answers_cancels() {
        let (mut session, ids) = two_circles();
        let transcript = run_scripted(&mut session, ids, [], move_command);
        assert_eq!(transcript.outcome, Outcome::Cancelled);
        assert_eq!(transcript.asked, ["Specify base point"]);
    }

    #[test]
    fn enter_uses_the_default_and_keywords_come_back_by_label() {
        let mut session = Session::default();
        let transcript = run_scripted(
            &mut session,
            vec![],
            [Reply::Enter, Reply::Keyword("Undo".into()), Reply::Enter],
            |ed| {
                let n = ed.get_number(Options::new("Count").default(3.0))?;
                assert_eq!(n, 3.0);
                let answer = ed.ask_point(PointOptions::new("Next point").keyword("Undo", "U"))?;
                assert_eq!(answer, Answer::Keyword("Undo".into()));
                let answer = ed.ask_point("Another point")?;
                assert_eq!(answer, Answer::Enter);
                Ok(())
            },
        );
        assert_eq!(transcript.outcome, Outcome::Completed, "{transcript:?}");
    }

    #[test]
    fn a_failed_transaction_in_a_command_reverts() {
        let (mut session, _) = two_circles();
        let transcript = run_scripted(&mut session, vec![], [], |ed| {
            ed.transaction("Doomed", |tx| {
                tx.add(circle_at(50.0));
                Err::<(), _>(Cancelled)
            })
        });
        assert_eq!(transcript.outcome, Outcome::Cancelled);
        assert_eq!(session.document.len(), 2);
        assert!(!session.history.can_undo());
    }

    #[test]
    fn reading_the_drawing_returns_data_to_the_command() {
        let (mut session, _) = two_circles();
        let transcript = run_scripted(&mut session, vec![], [], |ed| {
            let count = ed.read(|doc| doc.len())?;
            ed.say(format!("{count} objects"));
            Ok(())
        });
        assert_eq!(transcript.said, ["2 objects"]);
    }

    #[test]
    fn a_command_can_ask_for_selection_changes() {
        let (mut session, ids) = two_circles();
        let mut runner = Runner::start("test", vec![], {
            let ids = ids.clone();
            move |ed| {
                ed.select(ids.clone());
                ed.deselect(vec![ids[0]]);
                Ok(())
            }
        });
        runner.wait(&mut session);
        assert_eq!(runner.outcome(), Some(Outcome::Completed));
        assert_eq!(
            runner.take_selection_changes(),
            vec![(ids.clone(), false), (vec![ids[0]], true)]
        );
        assert!(runner.take_selection_changes().is_empty());
    }

    #[test]
    fn a_waiting_command_can_be_cancelled_from_outside() {
        let (mut session, _) = two_circles();
        let mut runner = Runner::start("test", vec![], |ed| {
            ed.get_point("Specify point")?;
            Ok(())
        });
        runner.wait(&mut session);
        assert_eq!(runner.prompt().unwrap().message, "Specify point");
        runner.cancel();
        runner.wait(&mut session);
        assert_eq!(runner.outcome(), Some(Outcome::Cancelled));
        assert!(!runner.is_active());
    }

    #[test]
    fn a_panicking_command_is_reported_not_propagated() {
        let (mut session, _) = two_circles();
        let mut runner = Runner::start("test", vec![], |_| panic!("plugin bug"));
        runner.wait(&mut session);
        assert_eq!(runner.outcome(), Some(Outcome::Crashed));
    }

    #[test]
    fn a_panic_in_work_done_for_the_command_is_contained() {
        let (mut session, _) = two_circles();
        let mut runner = Runner::start("test", vec![], |ed| {
            ed.read(|_| -> usize { panic!("bug in a drawing job") })?;
            Ok(())
        });
        runner.wait(&mut session);
        assert_eq!(runner.outcome(), Some(Outcome::Crashed));
        assert_eq!(session.document.len(), 2, "the drawing is still usable");
    }

    #[test]
    fn a_ghost_is_part_of_the_prompt() {
        let mut session = Session::default();
        let mut runner = Runner::start("test", vec![], |ed| {
            ed.get_point(
                PointOptions::new("Second point")
                    .base(p(1.0, 1.0))
                    .ghost(|at| vec![circle_at(at.x)]),
            )?;
            Ok(())
        });
        runner.wait(&mut session);
        let prompt = runner.prompt().unwrap();
        assert_eq!(prompt.base, Some(p(1.0, 1.0)));
        let ghost = prompt.ghost.as_ref().unwrap();
        assert_eq!(ghost(p(5.0, 0.0)), vec![circle_at(5.0)]);
        runner.cancel();
    }
}
