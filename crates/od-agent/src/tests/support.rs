//! A scripted host and the small helpers every test uses.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use od_api::{ApiError, ErrorCode};
use serde_json::{json, Value};

use crate::{
    run_task, Answer, Budgets, CancelToken, Event, FakeProvider, Host, Part, Plan, PlanOutcome,
    Question, Report, Retry, Step, TaskConfig, ToolCall, ToolOutcome, Workflow,
};

#[derive(Default)]
pub struct FakeHost {
    pub calls: Mutex<Vec<ToolCall>>,
    pub plans: Mutex<Vec<Plan>>,
    pub questions: Mutex<Vec<Question>>,
    pub finished: AtomicU32,
    outcomes: Mutex<HashMap<String, VecDeque<ToolOutcome>>>,
    plan_outcomes: Mutex<VecDeque<PlanOutcome>>,
    answers: Mutex<VecDeque<Answer>>,
    /// Cancel the task as soon as a tool is called.
    cancel_on_call: Mutex<Option<CancelToken>>,
}

impl FakeHost {
    pub fn new() -> Self {
        Self::default()
    }

    /// What a tool answers, once, in order; after that `{"ok": true}`.
    pub fn answers_tool(&self, name: &str, outcome: ToolOutcome) -> &Self {
        if let Ok(mut map) = self.outcomes.lock() {
            map.entry(name.to_owned()).or_default().push_back(outcome);
        }
        self
    }

    pub fn answers_plan(&self, outcome: PlanOutcome) -> &Self {
        if let Ok(mut q) = self.plan_outcomes.lock() {
            q.push_back(outcome);
        }
        self
    }

    pub fn answers_question(&self, answer: Answer) -> &Self {
        if let Ok(mut q) = self.answers.lock() {
            q.push_back(answer);
        }
        self
    }

    pub fn cancels_on_first_call(&self, token: &CancelToken) -> &Self {
        if let Ok(mut c) = self.cancel_on_call.lock() {
            *c = Some(token.clone());
        }
        self
    }

    pub fn called(&self) -> Vec<ToolCall> {
        self.calls.lock().map(|c| c.clone()).unwrap_or_default()
    }

    pub fn finish_count(&self) -> u32 {
        self.finished.load(Ordering::SeqCst)
    }
}

impl Host for FakeHost {
    fn call(&self, call: &ToolCall, _cancel: &CancelToken) -> ToolOutcome {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(call.clone());
        }
        if let Ok(mut c) = self.cancel_on_call.lock() {
            if let Some(token) = c.take() {
                token.cancel();
            }
        }
        let scripted = self
            .outcomes
            .lock()
            .ok()
            .and_then(|mut m| m.get_mut(&call.name).and_then(|q| q.pop_front()));
        scripted.unwrap_or_else(|| ToolOutcome::Done(json!({"ok": true})))
    }

    fn propose_plan(&self, plan: &Plan, _cancel: &CancelToken) -> PlanOutcome {
        if let Ok(mut plans) = self.plans.lock() {
            plans.push(plan.clone());
        }
        self.plan_outcomes
            .lock()
            .ok()
            .and_then(|mut q| q.pop_front())
            .unwrap_or(PlanOutcome::Approved)
    }

    fn ask_user(&self, question: &Question, _cancel: &CancelToken) -> Answer {
        if let Ok(mut qs) = self.questions.lock() {
            qs.push(question.clone());
        }
        self.answers
            .lock()
            .ok()
            .and_then(|mut q| q.pop_front())
            .unwrap_or(Answer::Text("no preference".to_owned()))
    }

    fn finish(&self) {
        self.finished.fetch_add(1, Ordering::SeqCst);
    }
}

/// Budgets that make a test quick: no waits between tries.
pub fn quick() -> TaskConfig {
    let mut config = TaskConfig::new("t1");
    config.retry = Retry {
        tries: 3,
        base: Duration::from_millis(1),
    };
    config
}

pub fn with_budgets(change: impl FnOnce(&mut Budgets)) -> TaskConfig {
    let mut config = quick();
    change(&mut config.budgets);
    config
}

pub struct Run {
    pub report: Report,
    pub events: Vec<Event>,
    pub provider: Arc<FakeProvider>,
}

pub fn run(
    steps: Vec<Step>,
    host: &FakeHost,
    workflow: &Workflow,
    config: TaskConfig,
    cancel: &CancelToken,
) -> Run {
    let provider = Arc::new(FakeProvider::new(steps));
    let mut events = Vec::new();
    let report = run_task(
        provider.clone(),
        host,
        workflow,
        "check this drawing",
        config,
        cancel,
        &mut |e| events.push(e),
    );
    Run {
        report,
        events,
        provider,
    }
}

/// The ordinary run: default test budgets, no cancel.
pub fn go(steps: Vec<Step>, host: &FakeHost, workflow: &Workflow) -> Run {
    run(steps, host, workflow, quick(), &CancelToken::new())
}

/// The text of every tool result the model was sent in its last request.
pub fn last_results(run: &Run) -> Vec<(String, bool)> {
    let requests = run.provider.requests();
    let Some(last) = requests.last() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for message in &last.messages {
        for part in &message.parts {
            if let Part::ToolResult(r) = part {
                let text: Vec<String> = r
                    .content
                    .iter()
                    .filter_map(|c| match c {
                        crate::Content::Text(t) => Some(t.clone()),
                        _ => None,
                    })
                    .collect();
                out.push((text.join(" "), r.is_error));
            }
        }
    }
    out
}

pub fn tool_names(run: &Run, request: usize) -> Vec<String> {
    run.provider.requests()[request]
        .tools
        .iter()
        .map(|t| t.name.clone())
        .collect()
}

pub fn api_error(code: ErrorCode, message: &str) -> ApiError {
    ApiError::new(code, message)
}

pub fn summary_call(id: &str) -> Step {
    Step::call(id, "opendraft_drawing_summary", json!({}))
}

pub fn batch_call(id: &str) -> Step {
    Step::call(
        id,
        "opendraft_entities_apply_batch",
        json!({"document": "d1", "operations": []}),
    )
}

pub fn plan_call(id: &str, steps: Value) -> Step {
    Step::call(
        id,
        "propose_plan",
        json!({"summary": "Move the line", "steps": steps}),
    )
}
