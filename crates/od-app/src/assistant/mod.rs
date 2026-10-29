//! The app's side of the in-app assistant: what the harness (`od-agent`) is
//! given to reach the drawings, and how the user is asked.
//!
//! The harness only asks and is told. It reaches a drawing through
//! [`AppHost`], which sends every request through the in-app connection to
//! the automation core, so the checks, limits and undo provenance are those
//! of any outside program. The permissions it holds are given here, below
//! the harness: a task's scopes, one approval for one exact request, and
//! nothing the user did not say yes to.
//!
//! The dialogs and the panel come later. [`Prompter`] is what they will
//! implement; the tests use a scripted one.

// Not used until the panel is built; the tests drive it.
#![allow(dead_code)]

mod host;
mod level;
mod prompter;

#[cfg(test)]
mod host_tests;
#[cfg(test)]
mod run_tests;
#[cfg(test)]
mod support;

#[allow(unused_imports)]
pub use host::AppHost;
#[allow(unused_imports)]
pub use level::{ApprovalLevel, AUTO_APPROVE_CEILING};
#[allow(unused_imports)]
pub use prompter::{ChangeRequest, Decision, Prompter};
