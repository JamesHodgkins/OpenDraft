//! The performance budgets of the specification (11.5), measured. They are
//! ignored in a normal run because debug builds say nothing about them. Run:
//! `cargo test --release -p od-app budget -- --ignored --nocapture`

use std::time::Instant;

use od_core::{Entity, Point};
use serde_json::{json, Value};

use super::{dispatch_test, document_id, Caller};
use crate::state::AppState;

const ME: Caller = Caller {
    client: "bench",
    grant_id: 1,
    history_any: false,
};

fn call(state: &mut AppState, op: &str, mut args: Value) -> Value {
    args["document_id"] = json!(document_id("inst", state.docs[0].uid));
    dispatch_test(state, "inst", &ME, op, args).unwrap()
}

/// 95th percentile of `runs` timings, in milliseconds.
fn p95(runs: usize, mut one: impl FnMut()) -> f64 {
    let mut times: Vec<f64> = (0..runs)
        .map(|_| {
            let start = Instant::now();
            one();
            start.elapsed().as_secs_f64() * 1e3
        })
        .collect();
    times.sort_by(f64::total_cmp);
    times[(runs * 95 / 100).min(runs - 1)]
}

fn drawing(n: usize) -> AppState {
    let mut state = AppState::new();
    // Added in a transaction each thousand: this is the setup, not the test.
    for chunk in 0..n / 1000 {
        state.docs[0]
            .session
            .transaction("Fill", |tx| {
                for i in 0..1000 {
                    let x = (chunk * 1000 + i) as f64;
                    tx.add(Entity::Line {
                        start: Point::new(x, 0.0),
                        end: Point::new(x + 0.5, 1.0),
                    });
                }
                Ok::<_, ()>(())
            })
            .unwrap();
    }
    state
}

#[test]
#[ignore = "a measurement for release builds"]
fn budget_reads_and_writes_on_a_ten_thousand_object_drawing() {
    let mut state = drawing(10_000);
    let out = std::io::stderr();
    let say = |text: String| {
        use std::io::Write;
        let _ = writeln!(out.lock(), "{text}");
    };
    let first = call(&mut state, "entities.query", json!({"limit": 100}));
    let id = first["entities"][50]["id"].as_str().unwrap().to_owned();

    let page = p95(200, || {
        call(&mut state, "entities.query", json!({"limit": 100}));
    });
    let far = p95(50, || {
        let mut args = json!({"limit": 100, "bounds": {"min_x": 9000.0, "min_y": -1.0, "max_x": 9100.0, "max_y": 2.0}});
        args["kinds"] = json!(["line"]);
        call(&mut state, "entities.query", args);
    });
    let one = p95(500, || {
        call(&mut state, "entities.get", json!({"ids": [id]}));
    });
    let summary = p95(200, || {
        call(&mut state, "document.get", json!({}));
    });
    say(format!(
        "10,000 objects: 100-entity page p95 {page:.2} ms, windowed page {far:.2} ms, one entity {one:.3} ms, summary {summary:.2} ms"
    ));
    assert!(page < 16.0, "page p95 {page} ms (budget 16)");
    assert!(one < 5.0, "single entity p95 {one} ms (budget 5)");

    // A 1,000-operation batch: validated as a whole before it is recorded.
    let revision = state.session().revision().to_string();
    let ops: Vec<Value> = (0..1000)
        .map(|i| {
            json!({"op": "add", "client_ref": format!("r{i}"), "entity": {"geometry": {
                "type": "line", "start": {"x": f64::from(i), "y": 5.0}, "end": {"x": f64::from(i) + 1.0, "y": 6.0}}}})
        })
        .collect();
    let start = Instant::now();
    call(
        &mut state,
        "entities.apply_batch",
        json!({"expected_revision": revision, "operations": ops}),
    );
    let batch = start.elapsed().as_secs_f64() * 1e3;
    let validate = p95(20, || {
        assert!(state.session().document().validate().is_ok());
    });
    say(format!(
        "1,000-operation batch (add) on 10,000 objects: {batch:.1} ms in all; a full validation alone p95 {validate:.2} ms"
    ));
    // It may pause the window once, but not for long.
    assert!(batch < 500.0, "a 1000 op batch took {batch} ms");
}
