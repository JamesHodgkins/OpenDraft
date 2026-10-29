//! Running the calls in a model's turn: each is checked against the tools
//! that were on offer when the model chose it, given its retry key, passed
//! to the host, and its result shaped. The harness's own two tools, the plan
//! and the question, are answered here by asking the host to ask the user.

use od_api::{catalogue, idempotency};
use serde_json::Value;

use crate::host::{Answer, Plan, PlanOutcome, Question, ToolOutcome};
use crate::shape;
use crate::state::{Failure, TaskState};
use crate::task::{Event, Task};
use crate::types::{Content, Message, Part, ToolCall, ToolResult};
use crate::workflow::{changes_drawing, ASK_USER, PROPOSE_PLAN};

/// How one call came out.
enum Step {
    /// The model is told this.
    Result(Vec<Content>, bool),
    /// The task ends instead.
    End(TaskState),
}

impl Task<'_> {
    /// Answer every call in `message`, in order, as one results message. A
    /// call is answered even when the task ends part way, so the kept
    /// conversation is well formed.
    pub(crate) fn run_calls(&mut self, message: &Message) {
        // What was on offer when the model chose, not what is on offer now:
        // a plan approved by the first call does not open the writes to the
        // second call of the same turn.
        let offered: Vec<String> = self
            .workflow
            .offered(self.writes_allowed)
            .into_iter()
            .map(|t| t.name)
            .collect();
        let mut results = Vec::new();
        let mut ended = None;
        for call in message.tool_calls() {
            let step = match &ended {
                Some(_) => Step::Result(shape::refused("not run: the task ended first"), true),
                None => self.run_call(call, &offered),
            };
            let (content, is_error) = match step {
                Step::Result(content, is_error) => (content, is_error),
                Step::End(state) => {
                    ended = Some(state);
                    (shape::refused("not run: the task ended first"), true)
                }
            };
            results.push(Part::ToolResult(ToolResult {
                call_id: call.id.clone(),
                content,
                is_error,
            }));
        }
        self.push_results(results);
        if let Some(state) = ended {
            self.end(state);
        }
    }

    fn run_call(&mut self, call: &ToolCall, offered: &[String]) -> Step {
        if self.cancel.is_cancelled() {
            return Step::End(TaskState::Cancelled);
        }
        if let Err(kind) = self.meter.charge_tool_call() {
            return Step::End(TaskState::Failed(Failure::Budget(kind)));
        }
        if !offered.iter().any(|name| name == &call.name) {
            return Step::Result(
                shape::refused(&format!(
                    "{} is not a tool you can use now. Use only the tools you were offered.",
                    call.name
                )),
                true,
            );
        }
        if !call.arguments.is_object() {
            return Step::Result(shape::refused("the arguments must be a JSON object"), true);
        }
        match call.name.as_str() {
            PROPOSE_PLAN => self.plan(call),
            ASK_USER => self.question(call),
            _ => self.tool(call),
        }
    }

    fn tool(&mut self, call: &ToolCall) -> Step {
        let Some(def) = catalogue::find(&call.name) else {
            return Step::Result(shape::refused("no such tool"), true);
        };
        let mut sent = call.clone();
        if idempotency::supports(def.operation) {
            if let Value::Object(arguments) = &mut sent.arguments {
                arguments.insert(
                    idempotency::KEY_FIELD.to_owned(),
                    Value::String(self.retry_key(&call.id)),
                );
            }
        }
        self.go(TaskState::RunningTool);
        (self.on_event)(Event::ToolStarted {
            name: call.name.clone(),
        });
        let outcome = self.host.call(&sent, self.cancel);
        let ok = matches!(outcome, ToolOutcome::Done(_));
        (self.on_event)(Event::ToolFinished {
            name: call.name.clone(),
            ok,
        });
        match outcome {
            ToolOutcome::Done(result) => {
                if changes_drawing(def.operation) {
                    self.wrote = true;
                }
                match shape::done(&call.name, result, &mut self.meter) {
                    Ok(content) => Step::Result(content, false),
                    Err(kind) => Step::End(TaskState::Failed(Failure::Budget(kind))),
                }
            }
            ToolOutcome::Failed(error) => Step::Result(shape::failed(&error), true),
            ToolOutcome::Declined => Step::Result(
                shape::refused("the user was asked and declined this; do not try it again"),
                true,
            ),
            ToolOutcome::Cancelled => Step::End(TaskState::Cancelled),
        }
    }

    fn plan(&mut self, call: &ToolCall) -> Step {
        let Some(plan) = parse_plan(&call.arguments) else {
            return Step::Result(
                shape::refused("a plan needs a summary and at least one step, as text"),
                true,
            );
        };
        self.go(TaskState::AwaitingApproval);
        match self.host.propose_plan(&plan, self.cancel) {
            PlanOutcome::Approved => {
                self.writes_allowed = true;
                let said = format!(
                    "The user approved the plan. You may make up to {} changes, as the plan \
                     states.",
                    plan.steps.len()
                );
                Step::Result(vec![Content::Text(said)], false)
            }
            PlanOutcome::Rejected(why) => {
                self.writes_allowed = false;
                self.go(TaskState::Planning);
                let said = match why {
                    Some(why) => format!("The user did not approve the plan: {why}"),
                    None => "The user did not approve the plan.".to_owned(),
                };
                Step::Result(shape::refused(&said), true)
            }
            PlanOutcome::Cancelled => Step::End(TaskState::Cancelled),
        }
    }

    fn question(&mut self, call: &ToolCall) -> Step {
        let Some(question) = parse_question(&call.arguments) else {
            return Step::Result(shape::refused("a question needs text"), true);
        };
        self.go(TaskState::WaitingForUser);
        match self.host.ask_user(&question, self.cancel) {
            Answer::Text(answer) => Step::Result(
                vec![Content::Text(shape::cut(
                    answer,
                    self.config.budgets.result_bytes_each,
                ))],
                false,
            ),
            Answer::Cancelled => Step::End(TaskState::Cancelled),
        }
    }

    /// `{task}-{call}` in the characters a key may hold.
    fn retry_key(&self, call_id: &str) -> String {
        let plain = |text: &str| -> String {
            text.chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | ':') {
                        c
                    } else {
                        '_'
                    }
                })
                .collect()
        };
        let mut key = format!("{}-{}", plain(&self.config.task_id), plain(call_id));
        key.truncate(idempotency::KEY_MAX);
        key
    }
}

fn parse_plan(arguments: &Value) -> Option<Plan> {
    let summary = arguments.get("summary")?.as_str()?.trim().to_owned();
    let steps: Vec<String> = arguments
        .get("steps")?
        .as_array()?
        .iter()
        .map(|s| s.as_str().map(|s| s.trim().to_owned()))
        .collect::<Option<_>>()?;
    if summary.is_empty() || steps.is_empty() || steps.iter().any(String::is_empty) {
        return None;
    }
    Some(Plan { summary, steps })
}

fn parse_question(arguments: &Value) -> Option<Question> {
    let text = arguments.get("question")?.as_str()?.trim().to_owned();
    if text.is_empty() {
        return None;
    }
    let choices = match arguments.get("choices") {
        None | Some(Value::Null) => Vec::new(),
        Some(list) => list
            .as_array()?
            .iter()
            .map(|c| c.as_str().map(str::to_owned))
            .collect::<Option<_>>()?,
    };
    Some(Question { text, choices })
}
