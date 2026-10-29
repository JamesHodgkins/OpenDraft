//! The hand runner is kept working by running it, with a scripted model and
//! a scripted keyboard.

use std::io::Cursor;
use std::sync::{Arc, Mutex};

use od_agent::TaskState;

use super::console::Sink;
use super::{run, DevRun};
use crate::assistant::ApprovalLevel;

/// A sink the test can read back.
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if let Ok(mut buffer) = self.0.lock() {
            buffer.extend_from_slice(bytes);
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Capture {
    fn text(&self) -> String {
        self.0
            .lock()
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default()
    }
}

fn settings(edit: bool, level: ApprovalLevel) -> DevRun {
    DevRun {
        file: None,
        prompt: "go".to_owned(),
        edit,
        level,
        key: None,
        save_to: None,
    }
}

fn go(settings: DevRun, keys: &str) -> (Result<super::Outcome, String>, String) {
    let capture = Capture::default();
    let sink: Sink = Arc::new(Mutex::new(Box::new(capture.clone())));
    let outcome = run(settings, Box::new(Cursor::new(keys.to_owned())), sink);
    (outcome, capture.text())
}

#[test]
fn a_scripted_review_runs_to_the_end_and_changes_nothing() {
    let (outcome, shown) = go(settings(false, ApprovalLevel::ReviewEach), "");
    let outcome = outcome.unwrap();
    assert_eq!(outcome.report.state, TaskState::Completed);
    assert_eq!(outcome.lines, 0);
    assert!(shown.contains("-> opendraft_drawing_audit"), "{shown}");
    assert!(shown.contains("model: scripted"));
}

#[test]
fn a_scripted_edit_asks_and_draws_when_told_yes() {
    let (outcome, shown) = go(settings(true, ApprovalLevel::ReviewEach), "y\ny\n");
    let outcome = outcome.unwrap();
    assert_eq!(outcome.report.state, TaskState::Completed);
    assert_eq!(outcome.lines, 1, "{shown}");
    assert!(shown.contains("Approve the plan?") && shown.contains("Allow it once?"));
}

#[test]
fn a_no_at_the_keyboard_draws_nothing() {
    let (outcome, shown) = go(settings(true, ApprovalLevel::ReviewEach), "y\nn\n");
    let outcome = outcome.unwrap();
    assert_eq!(outcome.lines, 0, "{shown}");
}

#[test]
fn closing_the_keyboard_stops_the_task_and_draws_nothing() {
    let (outcome, _) = go(settings(true, ApprovalLevel::ApprovePlan), "");
    let outcome = outcome.unwrap();
    assert_eq!(outcome.report.state, TaskState::Cancelled);
    assert_eq!(outcome.lines, 0);
}

#[test]
fn a_missing_file_is_reported_not_a_panic() {
    let mut s = settings(false, ApprovalLevel::ReviewEach);
    s.file = Some("no-such-file.odx".into());
    let (outcome, _) = go(s, "");
    assert!(outcome.err().is_some_and(|e| e.contains("could not open")));
}
