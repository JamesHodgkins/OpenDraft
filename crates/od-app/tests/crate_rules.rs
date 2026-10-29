//! ARCHITECTURE.md, "Crates (dependencies point downward)": the table is a
//! test. Each crate has a level, may depend only on crates of a lower level,
//! and has what it must never depend on. It reads each crate's declared
//! dependencies with `cargo metadata --no-deps` (offline), so a change that
//! lets, say, the in-app assistant reach the drawing without the guarded path,
//! or lets the wire contract pull in a UI library, fails here rather than
//! passing review by accident.
//!
//! Changing a rule is a decision: edit the table in ARCHITECTURE.md and the
//! table below together.
//!
//! The `od-agent` crate does not exist yet. Its rule is here, and applies
//! the day it is added: it may depend on `od-api` and nothing else of ours,
//! so the assistant can reach a drawing only through the operations that
//! outside programs use, with the same scopes, checks and limits.

use std::collections::HashMap;
use std::process::Command;

use serde_json::Value;

struct Rule {
    /// A crate may depend only on crates of a lower level.
    level: u8,
    /// If set, the only crates of ours it may depend on.
    only: Option<&'static [&'static str]>,
    /// Crates, of ours or not, it must never depend on.
    never: &'static [&'static str],
}

const UI: &[&str] = &["egui", "eframe"];

/// Every crate of the workspace, with its rule. A crate missing here fails,
/// so a new one gets a rule when it is added.
fn rules() -> HashMap<&'static str, Rule> {
    let table: Vec<(&str, Rule)> = vec![
        (
            "od-core",
            Rule {
                level: 0,
                only: Some(&[]),
                never: &["egui", "eframe", "rmcp", "interprocess"],
            },
        ),
        (
            "od-dwg",
            Rule {
                level: 0,
                only: Some(&[]),
                never: UI,
            },
        ),
        (
            "od-commands",
            Rule {
                level: 1,
                only: Some(&["od-core"]),
                never: UI,
            },
        ),
        (
            "od-files",
            Rule {
                level: 1,
                only: Some(&["od-core", "od-dwg"]),
                never: &["egui", "eframe", "od-app"],
            },
        ),
        (
            "od-api",
            Rule {
                level: 1,
                only: Some(&["od-core"]),
                never: &["egui", "eframe", "rmcp", "interprocess", "od-app"],
            },
        ),
        (
            "od-canvas",
            Rule {
                level: 2,
                only: None,
                never: &["od-app"],
            },
        ),
        (
            "od-ui",
            Rule {
                level: 2,
                only: None,
                never: &["od-core", "od-app"],
            },
        ),
        (
            "od-mcp",
            Rule {
                level: 2,
                only: Some(&["od-api"]),
                never: &["od-app", "od-core", "egui", "eframe"],
            },
        ),
        (
            "od-agent",
            Rule {
                level: 2,
                only: Some(&["od-api"]),
                never: &[
                    "rmcp",
                    "interprocess",
                    "egui",
                    "eframe",
                    "od-app",
                    "od-core",
                ],
            },
        ),
        (
            "od-app",
            Rule {
                level: 3,
                only: None,
                never: &[],
            },
        ),
    ];
    table.into_iter().collect()
}

/// What breaks the rules, from `cargo metadata --no-deps` output.
fn violations(metadata: &Value) -> Vec<String> {
    let rules = rules();
    let packages = metadata["packages"].as_array().cloned().unwrap_or_default();
    let ours: Vec<&str> = packages.iter().filter_map(|p| p["name"].as_str()).collect();
    let mut found = Vec::new();
    for package in &packages {
        let Some(name) = package["name"].as_str() else {
            continue;
        };
        let Some(rule) = rules.get(name) else {
            found.push(format!(
                "{name} has no rule: add it to the table in ARCHITECTURE.md and to crate_rules.rs"
            ));
            continue;
        };
        let dependencies = package["dependencies"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for dependency in dependencies.iter().filter_map(|d| d["name"].as_str()) {
            if rule.never.contains(&dependency) {
                found.push(format!("{name} must never depend on {dependency}"));
            }
            if !ours.contains(&dependency) {
                continue;
            }
            if rule.only.is_some_and(|only| !only.contains(&dependency)) {
                found.push(format!(
                    "{name} may depend only on {:?} of ours, not {dependency}",
                    rule.only.unwrap_or_default()
                ));
            }
            match rules.get(dependency) {
                Some(below) if below.level >= rule.level => found.push(format!(
                    "{name} (level {}) depends on {dependency} (level {}): dependencies point downward",
                    rule.level, below.level
                )),
                _ => {}
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

fn workspace_metadata() -> Value {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml");
    let output = Command::new(cargo)
        .args([
            "metadata",
            "--no-deps",
            "--offline",
            "--format-version",
            "1",
        ])
        .arg("--manifest-path")
        .arg(manifest)
        .output()
        .expect("cargo metadata runs");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("cargo metadata is JSON")
}

#[test]
fn the_crates_depend_only_downward_and_only_on_what_they_may() {
    let found = violations(&workspace_metadata());
    assert!(
        found.is_empty(),
        "the crate rules in ARCHITECTURE.md are broken:\n  {}",
        found.join("\n  ")
    );
}

/// A package as `cargo metadata` shows it, with these dependencies.
fn package(name: &str, depends_on: &[&str]) -> Value {
    serde_json::json!({
        "name": name,
        "dependencies": depends_on.iter().map(|d| serde_json::json!({"name": d})).collect::<Vec<_>>(),
    })
}

fn graph(packages: Vec<Value>) -> Value {
    serde_json::json!({ "packages": packages })
}

#[test]
fn the_checker_notices_each_kind_of_breakage() {
    // The real shape of the workspace, as a baseline that is clean.
    let clean = || {
        vec![
            package("od-core", &["rstar"]),
            package("od-api", &["od-core", "serde"]),
            package("od-mcp", &["od-api", "rmcp"]),
            package("od-app", &["od-api", "od-core", "eframe"]),
        ]
    };
    assert!(violations(&graph(clean())).is_empty());

    let with = |change: fn(&mut Vec<Value>)| {
        let mut packages = clean();
        change(&mut packages);
        violations(&graph(packages))
    };

    // The wire contract pulling in the MCP SDK or a UI library.
    let found = with(|p| p[1] = package("od-api", &["od-core", "rmcp"]));
    assert_eq!(found, ["od-api must never depend on rmcp"]);
    let found = with(|p| p[1] = package("od-api", &["od-core", "egui"]));
    assert_eq!(found, ["od-api must never depend on egui"]);

    // The MCP program reaching the app, or the document directly.
    let found = with(|p| p[2] = package("od-mcp", &["od-api", "od-app"]));
    assert!(
        found.contains(&"od-mcp must never depend on od-app".to_owned()),
        "{found:?}"
    );
    let found = with(|p| p[2] = package("od-mcp", &["od-api", "od-core"]));
    assert!(
        found.contains(&"od-mcp must never depend on od-core".to_owned()),
        "{found:?}"
    );

    // A lower crate depending upward.
    let found = with(|p| p[0] = package("od-core", &["od-api"]));
    assert!(
        found
            .iter()
            .any(|f| f.contains("dependencies point downward")),
        "{found:?}"
    );

    // A new crate with no rule.
    let found = with(|p| p.push(package("od-surprise", &["od-core"])));
    assert_eq!(found.len(), 1);
    assert!(found[0].starts_with("od-surprise has no rule"), "{found:?}");
}

#[test]
fn the_in_app_assistant_crate_may_reach_the_drawing_only_through_the_api() {
    // The day `od-agent` is added, these are what must hold. A crate that
    // reached the document, the commands, the files or the app could change
    // a drawing without the grants, revisions and limits every outside call
    // meets.
    let ok = |deps: &[&str]| {
        violations(&graph(vec![
            package("od-core", &[]),
            package("od-api", &["od-core"]),
            package("od-agent", deps),
        ]))
    };
    assert!(ok(&["od-api", "serde_json"]).is_empty());
    for forbidden in ["od-core", "od-app", "rmcp", "interprocess", "egui"] {
        let found = ok(&["od-api", forbidden]);
        assert!(!found.is_empty(), "od-agent must not depend on {forbidden}");
    }
    // `od-commands`, `od-files` and `od-canvas` are not in this graph, so
    // name them one by one: only `od-api` is allowed of ours.
    for ours in ["od-commands", "od-files", "od-canvas", "od-ui", "od-dwg"] {
        let found = violations(&graph(vec![
            package("od-api", &[]),
            package(ours, &[]),
            package("od-agent", &["od-api", ours]),
        ]));
        assert!(
            found.iter().any(|f| f.contains("may depend only on")),
            "od-agent must not depend on {ours}: {found:?}"
        );
    }
}
