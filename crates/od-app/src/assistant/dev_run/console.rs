//! The user, as a terminal: what the assistant wants to do is printed and
//! answered with y or n.

use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex};

use od_agent::{Answer, CancelToken, Plan, PlanOutcome, Question};

use crate::assistant::{ApprovalLevel, ChangeRequest, Decision, Prompter};

/// Where lines for the person at the keyboard go.
pub type Sink = Arc<Mutex<Box<dyn Write + Send>>>;

pub fn say(sink: &Sink, line: &str) {
    if let Ok(mut out) = sink.lock() {
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
    }
}

pub struct Console {
    pub input: Mutex<Box<dyn BufRead + Send>>,
    pub out: Sink,
}

impl Console {
    fn read_line(&self) -> Option<String> {
        let mut line = String::new();
        let n = self.input.lock().ok()?.read_line(&mut line).ok()?;
        (n > 0).then(|| line.trim().to_owned())
    }

    fn yes(&self, question: &str) -> Option<bool> {
        say(&self.out, &format!("{question} [y/n] "));
        let answer = self.read_line()?;
        Some(answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes"))
    }
}

impl Prompter for Console {
    fn approve_change(&self, request: &ChangeRequest, _cancel: &CancelToken) -> Decision {
        say(
            &self.out,
            &format!(
                "\n  The assistant wants to make this change ({}):\n  {}",
                request.tool,
                serde_json::to_string_pretty(&request.arguments).unwrap_or_default()
            ),
        );
        match self.yes("  Allow it once?") {
            Some(true) => Decision::Approved,
            Some(false) => Decision::Declined,
            None => Decision::Cancelled,
        }
    }

    fn approve_plan(
        &self,
        plan: &Plan,
        level: ApprovalLevel,
        _cancel: &CancelToken,
    ) -> PlanOutcome {
        say(&self.out, &format!("\n  Plan: {}", plan.summary));
        for (n, step) in plan.steps.iter().enumerate() {
            say(&self.out, &format!("    {}. {step}", n + 1));
        }
        let allows = match level {
            ApprovalLevel::ApprovePlan => {
                format!("  Approving allows {} change(s).", plan.steps.len())
            }
            _ => "  Approving allows no change by itself.".to_owned(),
        };
        say(&self.out, &allows);
        match self.yes("  Approve the plan?") {
            Some(true) => PlanOutcome::Approved,
            Some(false) => PlanOutcome::Rejected(None),
            None => PlanOutcome::Cancelled,
        }
    }

    fn ask(&self, question: &Question, _cancel: &CancelToken) -> Answer {
        say(
            &self.out,
            &format!("\n  The assistant asks: {}", question.text),
        );
        for choice in &question.choices {
            say(&self.out, &format!("    - {choice}"));
        }
        match self.read_line() {
            Some(text) => Answer::Text(text),
            None => Answer::Cancelled,
        }
    }
}
