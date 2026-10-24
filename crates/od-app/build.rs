//! Tells the program which commit it was built from and whether that commit is
//! the tagged release of its version (see `src/build_info.rs`). Without git
//! (a source archive) both stay unknown.

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

fn main() {
    // A commit, a branch switch or a new tag changes one of these.
    for path in ["HEAD", "refs", "packed-refs"] {
        println!("cargo:rerun-if-changed=../../.git/{path}");
    }
    let commit = git(&["rev-parse", "--short=9", "HEAD"]);
    let on_tag = commit.as_ref().map(|_| {
        let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
        let tags = git(&["tag", "--points-at", "HEAD"]).unwrap_or_default();
        let wanted = format!("v{version}");
        tags.lines().any(|t| t == wanted)
    });
    println!("cargo:rustc-env=OD_COMMIT={}", commit.unwrap_or_default());
    let on_tag = match on_tag {
        Some(true) => "1",
        Some(false) => "0",
        None => "?",
    };
    println!("cargo:rustc-env=OD_ON_TAG={on_tag}");
}
