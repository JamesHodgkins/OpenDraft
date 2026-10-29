//! The assistant's thread, and the way it asks the person at the screen. The
//! thread runs the harness; what it has to say goes to the UI thread as
//! updates, and a question it needs answered travels with a channel the UI
//! answers on. Nothing here touches a drawing: that is the host's, through
//! the core, on the UI thread's frame.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use od_agent::{
    run_task, Answer, CancelToken, Event, Plan, PlanOutcome, Provider, Question, Report,
    TaskConfig, Workflow,
};

use crate::assistant::{AppHost, ApprovalLevel, ChangeRequest, Decision, Prompter};

/// Called whenever there is something new for the UI to look at.
pub type Waker = Arc<dyn Fn() + Send + Sync>;

/// What the assistant needs from the person.
#[derive(Clone, Debug)]
pub enum Ask {
    Change(ChangeRequest),
    Plan(Plan, ApprovalLevel),
    Question(Question),
}

#[derive(Debug)]
pub enum Reply {
    Change(Decision),
    Plan(PlanOutcome),
    Answer(Answer),
}

/// A question waiting on the person, and where its answer goes.
pub struct Pending {
    pub ask: Ask,
    pub(super) reply: Sender<Reply>,
}

pub(super) enum Update {
    Event(Event),
    Ask(Pending),
    Done(Box<Report>),
}

/// Asks through the update channel and waits for the answer, giving up if the
/// task is stopped or the screen goes away.
struct ChannelPrompter {
    updates: Mutex<Sender<Update>>,
    wake: Waker,
}

impl ChannelPrompter {
    fn ask(&self, ask: Ask, cancel: &CancelToken) -> Option<Reply> {
        let (reply, answer) = mpsc::channel();
        self.updates
            .lock()
            .ok()?
            .send(Update::Ask(Pending { ask, reply }))
            .ok()?;
        (self.wake)();
        loop {
            match answer.recv_timeout(Duration::from_millis(50)) {
                Ok(reply) => return Some(reply),
                Err(RecvTimeoutError::Disconnected) => return None,
                Err(RecvTimeoutError::Timeout) => {
                    if cancel.is_cancelled() {
                        return None;
                    }
                }
            }
        }
    }
}

impl Prompter for ChannelPrompter {
    fn approve_change(&self, request: &ChangeRequest, cancel: &CancelToken) -> Decision {
        match self.ask(Ask::Change(request.clone()), cancel) {
            Some(Reply::Change(decision)) => decision,
            _ => Decision::Cancelled,
        }
    }

    fn approve_plan(&self, plan: &Plan, level: ApprovalLevel, cancel: &CancelToken) -> PlanOutcome {
        match self.ask(Ask::Plan(plan.clone(), level), cancel) {
            Some(Reply::Plan(outcome)) => outcome,
            _ => PlanOutcome::Cancelled,
        }
    }

    fn ask(&self, question: &Question, cancel: &CancelToken) -> Answer {
        match ChannelPrompter::ask(self, Ask::Question(question.clone()), cancel) {
            Some(Reply::Answer(answer)) => answer,
            _ => Answer::Cancelled,
        }
    }
}

/// Everything one task needs, moved onto its thread.
pub(super) struct Job {
    pub provider: Arc<dyn Provider>,
    pub core: Arc<crate::automation::AutomationCore>,
    pub workflow: Workflow,
    pub level: ApprovalLevel,
    pub prompt: String,
    pub config: TaskConfig,
    pub cancel: CancelToken,
    pub wake: Waker,
}

/// Start the task on its own thread. `Err` is why it could not start.
pub(super) fn start(job: Job) -> Result<Receiver<Update>, String> {
    let (updates, receiver) = mpsc::channel();
    let prompter = Arc::new(ChannelPrompter {
        updates: Mutex::new(updates.clone()),
        wake: Arc::clone(&job.wake),
    });
    let host = AppHost::new(job.core, prompter, job.level)
        .ok_or_else(|| "The assistant could not connect to the drawings.".to_owned())?;
    thread::Builder::new()
        .name("assistant".to_owned())
        .spawn(move || {
            let wake = Arc::clone(&job.wake);
            let events = updates.clone();
            let report = run_task(
                job.provider,
                &host,
                &job.workflow,
                &job.prompt,
                job.config,
                &job.cancel,
                &mut |event| {
                    let _ = events.send(Update::Event(event));
                    wake();
                },
            );
            let _ = updates.send(Update::Done(Box::new(report)));
            wake();
        })
        .map_err(|e| format!("The assistant could not start: {e}"))?;
    Ok(receiver)
}
