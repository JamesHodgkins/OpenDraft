//! `opendraft-mcp --check`, run as a user would, against a fake app.
#![allow(clippy::unwrap_used)] // helpers outside #[test] functions

mod support;

use std::path::PathBuf;
use std::process::Command;

use support::{FakeApp, TOKEN};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("od-mcp-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn check(dir: &PathBuf, extra: &[&str]) -> (bool, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_opendraft-mcp"))
        .arg("--check")
        .args(extra)
        .env("OD_AUTOMATION_DIR", dir)
        .output()
        .unwrap();
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

#[test]
fn check_says_what_it_can_reach_and_what_it_would_be_allowed() {
    let dir = scratch("check");
    let _app = FakeApp::start(&dir, "iiii", TOKEN);
    let (ok, text) = check(&dir, &[]);
    assert!(ok, "{text}");
    assert!(text.contains("automation API 1"), "{text}");
    assert!(
        text.contains("2026-07-28"),
        "the MCP versions are listed: {text}"
    );
    assert!(text.contains("connected, automation API 1"), "{text}");
    assert!(
        text.contains("may: read drawings"),
        "read-only until the user allows more: {text}"
    );
    assert!(!text.contains(TOKEN), "never the token: {text}");
    assert!(
        !check(&dir, &["--instance", "zzzz"]).0,
        "an instance that is not there"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_helps_when_nothing_is_running_or_only_a_leftover_is() {
    let dir = scratch("check-none");
    let (ok, text) = check(&dir, &[]);
    assert!(!ok);
    assert!(
        text.contains("Settings > Automation"),
        "tells you what to do: {text}"
    );
    // A descriptor nothing answers on, as a crash leaves.
    od_api::instance::write(
        &dir,
        &od_api::instance::Descriptor {
            instance_id: "stale".to_owned(),
            pid: 1,
            app_version: "0".to_owned(),
            api_versions: vec![1],
            endpoint: "od-nothing-here".to_owned(),
            token: "t".to_owned(),
        },
    )
    .unwrap();
    let (ok, text) = check(&dir, &[]);
    assert!(!ok);
    assert!(
        text.contains("not running") && text.contains("leftover"),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn check_with_two_apps_asks_for_a_choice() {
    let dir = scratch("check-two");
    let _a = FakeApp::start(&dir, "jjjj", TOKEN);
    let _b = FakeApp::start(&dir, "kkkk", TOKEN);
    let (ok, text) = check(&dir, &[]);
    assert!(ok);
    assert!(text.contains("--instance"), "{text}");
    assert!(check(&dir, &["--instance", "kkkk"]).0);
    let _ = std::fs::remove_dir_all(&dir);
}
