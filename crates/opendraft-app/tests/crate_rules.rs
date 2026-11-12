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
//! The `opendraft-agent` crate (the in-app assistant's harness) may depend on
//! `opendraft-api` and nothing else of ours, so the assistant can reach a drawing
//! only through the operations that outside programs use, with the same
//! scopes, checks and limits.

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
            "opendraft-core",
            Rule {
                level: 0,
                only: Some(&[]),
                never: &["egui", "eframe", "rmcp", "interprocess"],
            },
        ),
        (
            "opendraft-dwg",
            Rule {
                level: 0,
                only: Some(&[]),
                never: UI,
            },
        ),
        (
            "opendraft-commands",
            Rule {
                level: 1,
                only: Some(&["opendraft-core"]),
                never: UI,
            },
        ),
        (
            "opendraft-files",
            Rule {
                level: 1,
                only: Some(&["opendraft-core", "opendraft-dwg"]),
                never: &["egui", "eframe", "opendraft-app"],
            },
        ),
        (
            "opendraft-api",
            Rule {
                level: 1,
                only: Some(&["opendraft-core"]),
                never: &["egui", "eframe", "rmcp", "interprocess", "opendraft-app"],
            },
        ),
        (
            "opendraft-canvas",
            Rule {
                level: 2,
                only: None,
                never: &["opendraft-app"],
            },
        ),
        (
            "opendraft-ui",
            Rule {
                level: 2,
                only: None,
                never: &["opendraft-core", "opendraft-app"],
            },
        ),
        (
            "opendraft-mcp",
            Rule {
                level: 2,
                only: Some(&["opendraft-api"]),
                never: &["opendraft-app", "opendraft-core", "egui", "eframe"],
            },
        ),
        (
            "opendraft-agent",
            Rule {
                level: 2,
                only: Some(&["opendraft-api"]),
                never: &[
                    "rmcp",
                    "interprocess",
                    "egui",
                    "eframe",
                    "opendraft-app",
                    "opendraft-core",
                ],
            },
        ),
        (
            "opendraft-app",
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
            package("opendraft-core", &["rstar"]),
            package("opendraft-api", &["opendraft-core", "serde"]),
            package("opendraft-mcp", &["opendraft-api", "rmcp"]),
            package(
                "opendraft-app",
                &["opendraft-api", "opendraft-core", "eframe"],
            ),
        ]
    };
    assert!(violations(&graph(clean())).is_empty());

    let with = |change: fn(&mut Vec<Value>)| {
        let mut packages = clean();
        change(&mut packages);
        violations(&graph(packages))
    };

    // The wire contract pulling in the MCP SDK or a UI library.
    let found = with(|p| p[1] = package("opendraft-api", &["opendraft-core", "rmcp"]));
    assert_eq!(found, ["opendraft-api must never depend on rmcp"]);
    let found = with(|p| p[1] = package("opendraft-api", &["opendraft-core", "egui"]));
    assert_eq!(found, ["opendraft-api must never depend on egui"]);

    // The MCP program reaching the app, or the document directly.
    let found = with(|p| p[2] = package("opendraft-mcp", &["opendraft-api", "opendraft-app"]));
    assert!(
        found.contains(&"opendraft-mcp must never depend on opendraft-app".to_owned()),
        "{found:?}"
    );
    let found = with(|p| p[2] = package("opendraft-mcp", &["opendraft-api", "opendraft-core"]));
    assert!(
        found.contains(&"opendraft-mcp must never depend on opendraft-core".to_owned()),
        "{found:?}"
    );

    // A lower crate depending upward.
    let found = with(|p| p[0] = package("opendraft-core", &["opendraft-api"]));
    assert!(
        found
            .iter()
            .any(|f| f.contains("dependencies point downward")),
        "{found:?}"
    );

    // A new crate with no rule.
    let found = with(|p| p.push(package("opendraft-surprise", &["opendraft-core"])));
    assert_eq!(found.len(), 1);
    assert!(
        found[0].starts_with("opendraft-surprise has no rule"),
        "{found:?}"
    );
}

#[test]
fn the_in_app_assistant_crate_may_reach_the_drawing_only_through_the_api() {
    // These are what must hold for `opendraft-agent`. A crate that reached the
    // document, the commands, the files or the app could change a drawing
    // without the grants, revisions and limits every outside call meets.
    let ok = |deps: &[&str]| {
        violations(&graph(vec![
            package("opendraft-core", &[]),
            package("opendraft-api", &["opendraft-core"]),
            package("opendraft-agent", deps),
        ]))
    };
    assert!(ok(&["opendraft-api", "serde_json"]).is_empty());
    for forbidden in [
        "opendraft-core",
        "opendraft-app",
        "rmcp",
        "interprocess",
        "egui",
    ] {
        let found = ok(&["opendraft-api", forbidden]);
        assert!(
            !found.is_empty(),
            "opendraft-agent must not depend on {forbidden}"
        );
    }
    // `opendraft-commands`, `opendraft-files` and `opendraft-canvas` are not in this graph, so
    // name them one by one: only `opendraft-api` is allowed of ours.
    for ours in [
        "opendraft-commands",
        "opendraft-files",
        "opendraft-canvas",
        "opendraft-ui",
        "opendraft-dwg",
    ] {
        let found = violations(&graph(vec![
            package("opendraft-api", &[]),
            package(ours, &[]),
            package("opendraft-agent", &["opendraft-api", ours]),
        ]));
        assert!(
            found.iter().any(|f| f.contains("may depend only on")),
            "opendraft-agent must not depend on {ours}: {found:?}"
        );
    }
}
