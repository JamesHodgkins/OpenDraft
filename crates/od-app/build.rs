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

/// The icon and version information Explorer, the taskbar and SmartScreen
/// show for the exe. Without a resource compiler the exe is still built,
/// just plain.
#[cfg(windows)]
fn windows_resources() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    println!("cargo:rerun-if-changed=../../assets/app.ico");
    let mut resource = winresource::WindowsResource::new();
    resource
        .set_icon("../../assets/app.ico")
        .set("ProductName", "OpenDraft")
        .set("FileDescription", "OpenDraft, a 2D CAD program")
        .set("LegalCopyright", "GPL-3.0-or-later")
        .set("OriginalFilename", "opendraft.exe");
    if let Err(e) = resource.compile() {
        println!("cargo:warning=the Windows icon and version were not embedded: {e}");
    }
}

#[cfg(not(windows))]
fn windows_resources() {}

fn main() {
    windows_resources();
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
