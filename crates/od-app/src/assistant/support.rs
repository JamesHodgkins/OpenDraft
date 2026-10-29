//! A scripted user and a way to run the harness's thread beside the UI's.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use od_agent::{Answer, CancelToken, Plan, PlanOutcome, Question};
use serde_json::{json, Value};

use super::level::ApprovalLevel;
use super::prompter::{ChangeRequest, Decision, Prompter};
use crate::automation::{document_id, AutomationCore};
use crate::state::AppState;

/// A user who answers from a script, and remembers what they were shown.
#[derive(Default)]
pub struct Scripted {
    changes: Mutex<VecDeque<Decision>>,
    plans: Mutex<VecDeque<PlanOutcome>>,
    answers: Mutex<VecDeque<Answer>>,
    pub shown_changes: Mutex<Vec<ChangeRequest>>,
    pub shown_plans: Mutex<Vec<Plan>>,
}

impl Scripted {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn decides(&self, decision: Decision) -> &Self {
        if let Ok(mut q) = self.changes.lock() {
            q.push_back(decision);
        }
        self
    }

    pub fn answers_plan(&self, outcome: PlanOutcome) -> &Self {
        if let Ok(mut q) = self.plans.lock() {
            q.push_back(outcome);
        }
        self
    }

    pub fn change_prompts(&self) -> usize {
        self.shown_changes.lock().map_or(0, |s| s.len())
    }

    pub fn plan_prompts(&self) -> usize {
        self.shown_plans.lock().map_or(0, |s| s.len())
    }
}

impl Prompter for Scripted {
    fn approve_change(&self, request: &ChangeRequest, _cancel: &CancelToken) -> Decision {
        if let Ok(mut shown) = self.shown_changes.lock() {
            shown.push(request.clone());
        }
        self.changes
            .lock()
            .ok()
            .and_then(|mut q| q.pop_front())
            .unwrap_or(Decision::Approved)
    }

    fn approve_plan(
        &self,
        plan: &Plan,
        _level: ApprovalLevel,
        _cancel: &CancelToken,
    ) -> PlanOutcome {
        if let Ok(mut shown) = self.shown_plans.lock() {
            shown.push(plan.clone());
        }
        self.plans
            .lock()
            .ok()
            .and_then(|mut q| q.pop_front())
            .unwrap_or(PlanOutcome::Approved)
    }

    fn ask(&self, _question: &Question, _cancel: &CancelToken) -> Answer {
        self.answers
            .lock()
            .ok()
            .and_then(|mut q| q.pop_front())
            .unwrap_or(Answer::Text("no preference".to_owned()))
    }
}

/// Run `work` on its own thread, as the assistant's is, while this thread
/// does what the UI does: answer the core's queue each frame, and let the
/// test act as the user between frames.
pub fn drive<T: Send>(
    core: &AutomationCore,
    state: &mut AppState,
    mut user: impl FnMut(&mut AppState),
    work: impl FnOnce() -> T + Send,
) -> T {
    let registry = crate::commands::build_registry();
    thread::scope(|scope| {
        let handle = scope.spawn(work);
        while !handle.is_finished() {
            core.poll(state, &registry);
            user(state);
            thread::sleep(Duration::from_millis(1));
        }
        match handle.join() {
            Ok(value) => value,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    })
}

pub fn quiet_user(_: &mut AppState) {}

pub fn core() -> Arc<AutomationCore> {
    Arc::new(AutomationCore::new(
        crate::automation::fresh_instance_id(),
        Box::new(|| {}),
    ))
}

/// A batch that adds one line, written as the tool takes it.
pub fn add_a_line(core: &AutomationCore, state: &AppState, x: f64) -> Value {
    json!({
        "document_id": document_id(core.instance_id(), state.docs[0].uid),
        "expected_revision": state.session().revision().to_string(),
        "operations": [{"op": "add", "client_ref": "a", "entity": {"geometry": {
            "type": "line", "start": {"x": x, "y": 0}, "end": {"x": x + 1.0, "y": 0}}}}],
    })
}
