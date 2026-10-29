//! The file operations an automation connection has asked for: each from
//! the request, through the user's consent where one is needed, to its end.
//! Lives in `AppState` because the worker's results arrive there.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use od_api::file_ops::{OperationState, OperationStatus};
use od_api::{ApiError, ErrorCode, WireId};
use od_core::LayoutId;

use super::document_id;
use super::selection_request::Slot;
use crate::files::same_file;

/// Operations kept for polling; the oldest finished ones go first.
const KEPT: usize = 64;
/// How long a question waits for the user.
pub const CONSENT_WAIT: Duration = Duration::from_secs(120);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Open,
    Save,
    SaveAs,
    Export,
    /// An interactive command the user is finishing in the app.
    Command,
    /// The user choosing objects in the app for the caller.
    Selection,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Open => "open",
            Kind::Save => "save",
            Kind::SaveAs => "save_as",
            Kind::Export => "export",
            Kind::Command => "command",
            Kind::Selection => "selection",
        }
    }
}

/// What to do when the operation starts (after consent, if it needs it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    Open,
    Save { uid: u64 },
    SaveAs { uid: u64 },
    ExportDxf { uid: u64 },
    ExportSheet { uid: u64, layout: LayoutId },
    Command { uid: u64, serial: u64 },
}

pub struct Op {
    pub id: u64,
    pub grant_id: u64,
    /// What the connection called itself: shown beside the question, never
    /// trusted for anything.
    pub client: String,
    pub kind: Kind,
    pub plan: Plan,
    pub path: PathBuf,
    pub state: OperationState,
    /// The worker's job, once started.
    pub job: Option<u64>,
    pub document: Option<u64>,
    pub expected_revision: Option<u64>,
    pub revision: Option<u64>,
    pub notes: Vec<String>,
    pub error: Option<ApiError>,
    pub command_id: Option<&'static str>,
    pub waiting_for: Option<String>,
    /// For a selection request: when the user's time is up.
    pub deadline: Option<Instant>,
    /// Where the request's command leaves what the user chose.
    pub answer: Option<Slot>,
    /// What the user chose, once they have.
    pub selected: Option<Vec<u64>>,
    asked: Instant,
}

impl Op {
    pub fn status(&self, instance: &str) -> OperationStatus {
        OperationStatus {
            operation_id: format!("op_{}", self.id),
            kind: self.kind.name().to_owned(),
            state: self.state,
            file_name: od_api::paths::file_name(&self.path),
            command_id: self.command_id.map(str::to_owned),
            waiting_for: self.waiting_for.clone(),
            document_id: match self.kind {
                Kind::Open if self.state == OperationState::Succeeded => self.document,
                Kind::Selection => self.document,
                _ => None,
            }
            .map(|uid| document_id(instance, uid)),
            revision: self.revision.map(WireId),
            entity_ids: self
                .selected
                .as_ref()
                .map(|ids| ids.iter().copied().map(WireId).collect()),
            notes: self.notes.clone(),
            error: self.error.clone(),
            cancellable: matches!(
                self.state,
                OperationState::AwaitingConsent | OperationState::Running
            ),
        }
    }
}

#[derive(Default)]
pub struct FileOps {
    next: u64,
    ops: Vec<Op>,
}

/// A client name made safe to show beside a question.
fn tidy(client: &str) -> String {
    let clean: String = client
        .chars()
        .filter(|c| !c.is_control())
        .take(60)
        .collect();
    if clean.trim().is_empty() {
        "an assistant".to_owned()
    } else {
        clean.trim().to_owned()
    }
}

impl FileOps {
    pub fn add(
        &mut self,
        grant_id: u64,
        client: &str,
        kind: Kind,
        plan: Plan,
        path: PathBuf,
        expected_revision: Option<u64>,
    ) -> u64 {
        self.next += 1;
        let id = self.next;
        if self.ops.len() >= KEPT {
            if let Some(old) = self.ops.iter().position(|o| o.state.is_finished()) {
                self.ops.remove(old);
            }
        }
        self.ops.push(Op {
            id,
            grant_id,
            client: tidy(client),
            kind,
            plan,
            path,
            state: OperationState::Running,
            job: None,
            document: None,
            expected_revision,
            revision: None,
            notes: Vec::new(),
            error: None,
            command_id: None,
            waiting_for: None,
            deadline: None,
            answer: None,
            selected: None,
            asked: Instant::now(),
        });
        id
    }

    /// Note an interactive command that has just been started.
    pub fn add_command(
        &mut self,
        grant_id: u64,
        client: &str,
        command_id: &'static str,
        uid: u64,
        serial: u64,
    ) -> u64 {
        let id = self.add(
            grant_id,
            client,
            Kind::Command,
            Plan::Command { uid, serial },
            PathBuf::new(),
            None,
        );
        if let Some(op) = self.get_mut(id) {
            op.command_id = Some(command_id);
            op.document = Some(uid);
        }
        id
    }

    /// Note a selection request whose command has just been started. It
    /// ends at `deadline` if the user has not finished by then.
    pub fn add_selection(
        &mut self,
        grant_id: u64,
        client: &str,
        (uid, serial): (u64, u64),
        deadline: Instant,
        answer: Slot,
    ) -> u64 {
        let id = self.add(
            grant_id,
            client,
            Kind::Selection,
            Plan::Command { uid, serial },
            PathBuf::new(),
            None,
        );
        if let Some(op) = self.get_mut(id) {
            op.document = Some(uid);
            op.deadline = Some(deadline);
            op.answer = Some(answer);
        }
        id
    }

    /// Every running command or selection request, as (id, serial,
    /// document): the operations that follow a command in the app.
    pub fn running_commands(&self) -> Vec<(u64, u64, u64)> {
        self.ops
            .iter()
            .filter(|o| matches!(o.kind, Kind::Command | Kind::Selection))
            .filter(|o| o.state == OperationState::Running)
            .filter_map(|o| match o.plan {
                Plan::Command { uid, serial } => Some((o.id, serial, uid)),
                _ => None,
            })
            .collect()
    }

    /// The selection requests still going, as (id, owner grant, serial of
    /// the command showing the prompt).
    pub fn running_selections(&self) -> Vec<(u64, u64, u64)> {
        self.ops
            .iter()
            .filter(|o| o.kind == Kind::Selection && o.state == OperationState::Running)
            .filter_map(|o| match o.plan {
                Plan::Command { serial, .. } => Some((o.id, o.grant_id, serial)),
                _ => None,
            })
            .collect()
    }

    /// The selection requests whose time is up, as (id, serial).
    pub fn overdue_selections(&self) -> Vec<(u64, u64)> {
        let now = Instant::now();
        self.running_selections()
            .into_iter()
            .filter(|(id, ..)| {
                self.get(*id)
                    .and_then(|o| o.deadline)
                    .is_some_and(|deadline| deadline <= now)
            })
            .map(|(id, _, serial)| (id, serial))
            .collect()
    }

    pub fn get(&self, id: u64) -> Option<&Op> {
        self.ops.iter().find(|o| o.id == id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut Op> {
        self.ops.iter_mut().find(|o| o.id == id)
    }

    /// Operations still in flight, for the cap on how many a caller starts.
    pub fn unfinished(&self, grant_id: u64) -> usize {
        self.ops
            .iter()
            .filter(|o| o.grant_id == grant_id && !o.state.is_finished())
            .count()
    }

    /// The question the user should answer next.
    pub fn awaiting(&self) -> Option<&Op> {
        self.ops
            .iter()
            .find(|o| o.state == OperationState::AwaitingConsent)
    }

    pub fn fail(&mut self, id: u64, code: ErrorCode, message: &str) {
        if let Some(op) = self.get_mut(id).filter(|o| !o.state.is_finished()) {
            op.state = OperationState::Failed;
            op.error = Some(ApiError::new(code, message));
        }
    }

    pub fn succeed(&mut self, id: u64, notes: Vec<String>) {
        if let Some(op) = self.get_mut(id).filter(|o| !o.state.is_finished()) {
            op.state = OperationState::Succeeded;
            op.notes = notes;
        }
    }

    pub fn cancel(&mut self, id: u64) {
        if let Some(op) = self.get_mut(id).filter(|o| !o.state.is_finished()) {
            op.state = OperationState::Cancelled;
        }
    }

    /// The user gave up on all file work from the status bar. Commands the
    /// user is answering are not file work and go on.
    pub fn cancel_running(&mut self) {
        for op in &mut self.ops {
            if op.state == OperationState::Running
                && !matches!(op.kind, Kind::Command | Kind::Selection)
            {
                op.state = OperationState::Cancelled;
            }
        }
    }

    /// The connections went away: questions nobody can use any more.
    pub fn cancel_waiting(&mut self) {
        for op in &mut self.ops {
            if op.state == OperationState::AwaitingConsent {
                op.state = OperationState::Cancelled;
            }
        }
    }

    /// Questions the user did not answer in time fail.
    pub fn expire(&mut self) {
        self.expire_after(CONSENT_WAIT);
    }

    pub(super) fn expire_after(&mut self, wait: Duration) {
        let late: Vec<u64> = self
            .ops
            .iter()
            .filter(|o| o.state == OperationState::AwaitingConsent && o.asked.elapsed() >= wait)
            .map(|o| o.id)
            .collect();
        for id in late {
            self.fail(
                id,
                ErrorCode::DeadlineExceeded,
                "The user did not answer in time.",
            );
        }
    }

    fn running(&mut self, kind: Kind, matches: impl Fn(&Op) -> bool) -> Option<&mut Op> {
        self.ops
            .iter_mut()
            .find(|o| o.kind == kind && o.state == OperationState::Running && matches(o))
    }

    /// An open finished: `Ok` carries the new drawing's uid and what the
    /// import left out.
    pub fn opened(&mut self, path: &Path, result: Result<(Option<u64>, Vec<String>), String>) {
        let Some(op) = self.running(Kind::Open, |o| same_file(&o.path, path)) else {
            return;
        };
        match result {
            Ok((uid, notes)) => {
                op.state = OperationState::Succeeded;
                op.document = uid;
                op.notes = notes;
            }
            Err(message) => {
                op.state = OperationState::Failed;
                op.error = Some(ApiError::new(ErrorCode::OperationFailed, message));
            }
        }
    }

    /// A save finished.
    pub fn saved(&mut self, uid: u64, result: Result<Vec<String>, String>) {
        let found = self.ops.iter_mut().find(|o| {
            matches!(o.kind, Kind::Save | Kind::SaveAs)
                && o.state == OperationState::Running
                && o.document == Some(uid)
        });
        Self::settle(found, result);
    }

    /// An export finished.
    pub fn exported(&mut self, path: &Path, result: Result<Vec<String>, String>) {
        let found = self.running(Kind::Export, |o| same_file(&o.path, path));
        Self::settle(found, result);
    }

    fn settle(op: Option<&mut Op>, result: Result<Vec<String>, String>) {
        let Some(op) = op else {
            return;
        };
        match result {
            Ok(notes) => {
                op.state = OperationState::Succeeded;
                op.notes = notes;
            }
            Err(message) => {
                op.state = OperationState::Failed;
                op.error = Some(ApiError::new(ErrorCode::OperationFailed, message));
            }
        }
    }
}
