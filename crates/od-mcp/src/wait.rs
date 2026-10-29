//! Waiting on an operation for a little while, so a caller that asked the
//! user for something (a selection, a command to finish) hears about the
//! answer as it comes instead of polling blind. The wait is done here, never
//! in the app, which must answer on its UI thread within a frame: this asks
//! again every so often, each ask as cheap as any poll, and gives back the
//! latest status when the time is up. It is bounded well under the time MCP
//! hosts allow a tool call.

use std::time::{Duration, Instant};

use od_api::limits::OPERATION_WAIT_MAX_S;
use od_api::{ApiError, ErrorCode};
use serde_json::{json, Map, Value};

/// The argument that asks for a wait.
pub const FIELD: &str = "wait_seconds";
/// How often the app is asked while waiting.
const ASK_EVERY: Duration = Duration::from_millis(250);

/// Add the argument to the schema of the tool that accepts it.
pub fn offer(properties: &mut Map<String, Value>) {
    properties.insert(
        FIELD.to_owned(),
        json!({
            "type": "integer",
            "minimum": 0,
            "maximum": OPERATION_WAIT_MAX_S,
            "description": "Optional: wait up to this many seconds (30 at most) for it to finish; answers sooner if it does."
        }),
    );
}

/// Take the wait out of the arguments, so the app never sees it. A wait
/// longer than the most is the most.
pub fn take(arguments: &mut Map<String, Value>) -> Result<Duration, ApiError> {
    let Some(value) = arguments.remove(FIELD) else {
        return Ok(Duration::ZERO);
    };
    let seconds = value.as_u64().ok_or_else(|| {
        ApiError::new(
            ErrorCode::InvalidArguments,
            "wait_seconds must be a whole number of seconds.",
        )
    })?;
    Ok(Duration::from_secs(
        seconds.min(u64::from(OPERATION_WAIT_MAX_S)),
    ))
}

/// Whether the operation in a status has more to do: the user is still
/// answering, or its work is running.
fn unfinished(status: &Value) -> bool {
    matches!(
        status["state"].as_str(),
        Some("running" | "awaiting_consent")
    )
}

/// Ask once, and keep asking until the operation is finished or `wait` has
/// passed. An error from any ask ends the wait with that error.
pub fn until_finished(
    wait: Duration,
    ask: impl FnMut() -> Result<Value, ApiError>,
) -> Result<Value, ApiError> {
    settle(wait, ASK_EVERY, ask)
}

fn settle(
    wait: Duration,
    every: Duration,
    mut ask: impl FnMut() -> Result<Value, ApiError>,
) -> Result<Value, ApiError> {
    let until = Instant::now() + wait;
    let mut status = ask()?;
    while unfinished(&status) && Instant::now() + every <= until {
        std::thread::sleep(every);
        status = ask()?;
    }
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TICK: Duration = Duration::from_millis(2);

    fn args(value: Value) -> Map<String, Value> {
        value.as_object().cloned().unwrap_or_default()
    }

    #[test]
    fn the_wait_is_taken_out_of_the_arguments_and_bounded() {
        let mut given = args(json!({"operation_id": "op_1", "wait_seconds": 10}));
        assert_eq!(take(&mut given).unwrap(), Duration::from_secs(10));
        assert!(!given.contains_key(FIELD), "the app never sees it");
        assert_eq!(given.len(), 1);

        let mut none = args(json!({"operation_id": "op_1"}));
        assert_eq!(take(&mut none).unwrap(), Duration::ZERO);
        let mut long = args(json!({"wait_seconds": 100_000}));
        assert_eq!(take(&mut long).unwrap(), Duration::from_secs(30));
        for bad in [json!(-1), json!("5"), json!(1.5), json!(null)] {
            let mut wrong = args(json!({ "wait_seconds": bad }));
            let error = take(&mut wrong).unwrap_err();
            assert_eq!(error.code, ErrorCode::InvalidArguments);
        }
    }

    #[test]
    fn it_returns_as_soon_as_the_operation_finishes() {
        let mut asked = 0;
        let status = settle(Duration::from_secs(5), TICK, || {
            asked += 1;
            let state = if asked < 4 { "running" } else { "succeeded" };
            Ok(json!({"state": state}))
        })
        .unwrap();
        assert_eq!(status["state"], "succeeded");
        assert_eq!(asked, 4);
    }

    #[test]
    fn with_no_wait_it_asks_exactly_once() {
        let mut asked = 0;
        let status = settle(Duration::ZERO, TICK, || {
            asked += 1;
            Ok(json!({"state": "running"}))
        })
        .unwrap();
        assert_eq!(status["state"], "running");
        assert_eq!(asked, 1);
    }

    #[test]
    fn when_time_runs_out_the_latest_status_comes_back_still_running() {
        let started = Instant::now();
        let status = settle(Duration::from_millis(40), TICK, || {
            Ok(json!({"state": "running", "waiting_for": "Select the walls"}))
        })
        .unwrap();
        assert_eq!(status["state"], "running");
        assert_eq!(status["waiting_for"], "Select the walls");
        assert!(started.elapsed() < Duration::from_millis(500));
    }

    #[test]
    fn a_finished_operation_is_not_waited_for_and_an_error_ends_the_wait() {
        let mut asked = 0;
        settle(Duration::from_secs(5), TICK, || {
            asked += 1;
            Ok(json!({"state": "cancelled"}))
        })
        .unwrap();
        assert_eq!(asked, 1);

        let mut asked = 0;
        let error = settle(Duration::from_secs(5), TICK, || {
            asked += 1;
            if asked == 2 {
                Err(ApiError::new(ErrorCode::InstanceGone, "gone"))
            } else {
                Ok(json!({"state": "running"}))
            }
        })
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::InstanceGone);
        assert_eq!(asked, 2);
    }
}
