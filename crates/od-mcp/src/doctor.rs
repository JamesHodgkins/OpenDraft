//! `opendraft-mcp --check`: say, in words, whether this program can reach a
//! running OpenDraft and what it would be allowed to do. For setting up and
//! for working out why a host says the server does not work.

use std::fmt::Write as _;
use std::path::Path;

use od_api::{ErrorCode, Scope};

use crate::client::{discover, probe, Found};

/// What the connection may do, in the words of the Settings switches.
fn permissions(scopes: &[Scope]) -> String {
    let mut allowed = vec!["read drawings"];
    if scopes.contains(&Scope::DrawingWrite) {
        allowed.push("change drawings");
    }
    if scopes.contains(&Scope::CommandInvoke) {
        allowed.push("run commands");
    }
    if scopes.contains(&Scope::FileWrite) {
        allowed.push("open and save files in approved folders");
    }
    allowed.join(", ")
}

fn describe(found: &Found, out: &mut String) -> bool {
    let d = &found.descriptor;
    let head = format!(
        "  {}  OpenDraft {}  (pid {})",
        d.instance_id, d.app_version, d.pid
    );
    if !found.reachable {
        let _ = writeln!(
            out,
            "{head}\n    not running: a leftover from a closed or crashed OpenDraft (it is cleaned up at the next start)"
        );
        return false;
    }
    match probe(d) {
        Ok(hello) => {
            let _ = writeln!(
                out,
                "{head}\n    connected, automation API {}\n    this connection may: {}\n    (the switches are in OpenDraft, Settings > Automation)",
                hello.api_version,
                permissions(&hello.scopes)
            );
            true
        }
        Err(error) => {
            let hint = match error.code {
                ErrorCode::InvalidArguments => {
                    "the program and the app speak different automation API versions: use the same release of both"
                }
                ErrorCode::ScopeDenied => {
                    "the app refused the token: the instance file is out of date, restart OpenDraft"
                }
                _ => "could not talk to it",
            };
            let _ = writeln!(out, "{head}\n    {hint} ({})", error.message);
            false
        }
    }
}

/// The report, and whether at least one app could be used.
pub fn report(dir: Option<&Path>, wanted: Option<&str>) -> (String, bool) {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "opendraft-mcp {} (automation API {}; MCP protocol versions {})",
        env!("CARGO_PKG_VERSION"),
        od_api::API_VERSION,
        rmcp::model::ProtocolVersion::KNOWN_VERSIONS
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    let Some(dir) = dir else {
        let _ = writeln!(
            out,
            "\nCannot tell where OpenDraft keeps its instance files: the folder variable for this system is not set (LOCALAPPDATA on Windows, XDG_RUNTIME_DIR on Linux, or OD_AUTOMATION_DIR)."
        );
        return (out, false);
    };
    let _ = writeln!(out, "Instance folder: {}", dir.display());
    let found = discover(dir);
    if found.is_empty() {
        let _ = writeln!(
            out,
            "\nNo OpenDraft is offering a connection. Start OpenDraft and turn on Settings > Automation > \"Let AI assistants work with the open drawings\", then run this again."
        );
        return (out, false);
    }
    let _ = writeln!(out, "\nFound {}:", crate::plural(found.len()));
    let mut usable = false;
    for one in &found {
        let chosen = wanted.is_none_or(|w| w == one.descriptor.instance_id);
        usable |= describe(one, &mut out) && chosen;
    }
    if let Some(w) = wanted {
        if !found.iter().any(|f| f.descriptor.instance_id == w) {
            let _ = writeln!(
                out,
                "\nThe instance asked for with --instance ({w}) is not among them."
            );
            usable = false;
        }
    }
    let live = found.iter().filter(|f| f.reachable).count();
    if wanted.is_none() && live > 1 {
        let _ = writeln!(
            out,
            "\nSeveral are running: start this program with --instance <id> to choose one."
        );
    }
    (out, usable)
}
