//! `selection.request`: a caller asks the user to select objects in the app
//! and gets their ids back. The user picks on the canvas as they always do
//! and finishes with Enter, or refuses with Esc; the caller can only start
//! the request, watch it and cancel it, like an interactive command, and it
//! is polled the same way (`operations.get`).
//!
//! It runs as a small command of its own, so it inherits the canvas's
//! selection rules (hidden, frozen and locked objects, the active space),
//! Esc, the command line, and the rule that it never cuts in on a command the
//! user is running. Nothing in the drawing changes.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use od_api::limits::{
    GET_IDS_MAX, SELECTION_PROMPT_MAX, SELECTION_WAIT_DEFAULT_S, SELECTION_WAIT_MAX_S,
};
use od_api::session_ops::SelectionRequestArgs;
use od_api::{ApiError, ErrorCode};
use od_commands::{Cancelled, Editor, Options};
use od_core::{Document, EntityId, EntityKind, LayerId};

use super::command_runs::{encode, sync};
use super::file_ops::not_too_many;
use super::guard::{open_for_change, Caller};
use super::read::parse;
use crate::state::AppState;
use crate::text::plural;

/// How long starting a request waits for its prompt to appear.
const FIRST_PROMPT_WAIT: Duration = Duration::from_millis(100);
/// The command's name: what the command line labels the prompt with. A
/// caller cannot choose it, so a prompt under it is known to be a request.
const NAME: &str = "Assistant";

/// What the user chose, left by the request's command for the host to pick up.
pub(crate) struct Picked {
    pub(crate) revision: u64,
    pub(crate) ids: Vec<u64>,
}

pub(crate) type Slot = Arc<Mutex<Option<Picked>>>;

/// What the user's selection must satisfy to be handed back.
#[derive(Clone)]
pub(super) struct Rule {
    prompt: String,
    kinds: Option<Vec<EntityKind>>,
    layer: Option<LayerId>,
    min: usize,
    max: usize,
}

pub(super) enum Verdict {
    /// Hand these back; `left_out` were selected but do not qualify.
    Accept {
        ids: Vec<EntityId>,
        left_out: Vec<EntityId>,
        /// What to tell the user about those, if anything.
        said: String,
    },
    /// Ask again, saying why.
    Again {
        left_out: Vec<EntityId>,
        said: String,
    },
}

impl Rule {
    /// Anything qualifies, within these counts (tests).
    #[cfg(test)]
    pub(super) fn any(min: usize, max: usize) -> Self {
        Self {
            prompt: String::new(),
            kinds: None,
            layer: None,
            min,
            max,
        }
    }

    /// The question as the user reads it, with what is wanted spelled out.
    fn asking(&self) -> String {
        let count = match (self.min, self.max) {
            (min, max) if min == max => format!(" (exactly {min})"),
            (0 | 1, GET_IDS_MAX) => String::new(),
            (min, GET_IDS_MAX) => format!(" (at least {min})"),
            (0 | 1, max) => format!(" (at most {max})"),
            (min, max) => format!(" ({min} to {max})"),
        };
        format!("{}{count}", self.prompt)
    }

    fn accepts(&self, document: &Document, id: EntityId) -> bool {
        document.object(id).is_some_and(|o| {
            self.layer.is_none_or(|l| o.attrs.layer == l)
                && self
                    .kinds
                    .as_ref()
                    .is_none_or(|k| k.contains(&o.entity.kind()))
        })
    }

    /// Check what the user selected against the drawing as it is now: an
    /// object that is gone, hidden or locked since, or that the request did
    /// not ask for, is left out and never handed back.
    pub(super) fn judge(&self, document: &Document, picked: &[EntityId]) -> Verdict {
        let pickable: std::collections::BTreeSet<EntityId> =
            document.selectable_objects().map(|o| o.id).collect();
        let (ids, left_out): (Vec<EntityId>, Vec<EntityId>) = picked
            .iter()
            .copied()
            .partition(|id| pickable.contains(id) && self.accepts(document, *id));
        let mut said = String::new();
        if !left_out.is_empty() {
            let (n, wanted) = (left_out.len(), self.wanted());
            said = format!(
                "{} did not qualify{wanted} and {} taken out of the selection.",
                plural(n, "object"),
                if n == 1 { "was" } else { "were" }
            );
        }
        let problem = if ids.len() > self.max {
            Some(format!(
                "{} selected; at most {} can be used. Remove some, then press Enter.",
                plural(ids.len(), "object"),
                self.max
            ))
        } else if ids.len() < self.min {
            Some(format!(
                "{} usable; at least {} needed.",
                plural(ids.len(), "object"),
                self.min
            ))
        } else {
            None
        };
        match problem {
            None => Verdict::Accept {
                ids,
                left_out,
                said,
            },
            Some(problem) => Verdict::Again {
                left_out,
                said: if said.is_empty() {
                    problem
                } else {
                    format!("{said} {problem}")
                },
            },
        }
    }

    /// " (wanted: line or circle)", or nothing when anything qualifies.
    fn wanted(&self) -> String {
        if self.kinds.is_none() && self.layer.is_none() {
            return String::new();
        }
        let what = match &self.kinds {
            Some(kinds) => {
                let names: Vec<_> = kinds.iter().map(|k| k.wire_name()).collect();
                names.join(" or ")
            }
            None => "objects".to_owned(),
        };
        let on = if self.layer.is_some() {
            " on the requested layer"
        } else {
            ""
        };
        format!(" (wanted: {what}{on})")
    }
}

/// The command: ask, check against the drawing, and ask again until the
/// answer qualifies or the user gives up.
fn ask(ed: &mut Editor, rule: &Rule, answer: &Slot) -> Result<(), Cancelled> {
    loop {
        let mut options = Options::<()>::new(rule.asking());
        if rule.min == 0 {
            options = options.allow_empty();
        }
        let picked = ed.ask_selection(options)?;
        let judging = rule.clone();
        let (revision, verdict) = ed.read_revision(move |document, revision| {
            (revision, judging.judge(document, &picked))
        })?;
        match verdict {
            Verdict::Accept {
                ids,
                left_out,
                said,
            } => {
                if !left_out.is_empty() {
                    ed.deselect(left_out);
                    ed.say(said);
                }
                if let Ok(mut slot) = answer.lock() {
                    *slot = Some(Picked {
                        revision,
                        ids: ids.iter().map(|id| id.0).collect(),
                    });
                }
                return Ok(());
            }
            Verdict::Again { left_out, said } => {
                if !left_out.is_empty() {
                    ed.deselect(left_out);
                }
                ed.say(said);
            }
        }
    }
}

fn invalid(message: &str) -> ApiError {
    ApiError::new(ErrorCode::InvalidArguments, message)
}

/// The prompt made safe to show: control characters become spaces, runs of
/// spaces one, and it must say something within the limit.
fn clean_prompt(text: &str) -> Result<String, ApiError> {
    let flat: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let prompt = flat.split_whitespace().collect::<Vec<_>>().join(" ");
    if prompt.is_empty() {
        return Err(invalid("The prompt must say what the user should select."));
    }
    if prompt.chars().count() > SELECTION_PROMPT_MAX {
        return Err(invalid("The prompt is too long.").detail("limit", SELECTION_PROMPT_MAX));
    }
    Ok(prompt)
}

fn rule_from(args: &SelectionRequestArgs, document: &Document) -> Result<Rule, ApiError> {
    let min = args.min_count.unwrap_or(1);
    let max = args.max_count.unwrap_or(GET_IDS_MAX).min(GET_IDS_MAX);
    if max == 0 || min > max {
        return Err(
            invalid("max_count must be at least 1 and not below min_count.")
                .detail("limit", GET_IDS_MAX),
        );
    }
    let kinds = match &args.kinds {
        None => None,
        Some(names) => {
            let mut kinds = Vec::new();
            for name in names {
                let kind = EntityKind::from_wire(name)
                    .ok_or_else(|| invalid("No such entity kind.").detail("kind", name.as_str()))?;
                kinds.push(kind);
            }
            Some(kinds)
        }
    };
    let layer = match args.layer_id {
        None => None,
        Some(id) => {
            let id = LayerId(id.0);
            document.layer(id).ok_or_else(|| {
                ApiError::new(ErrorCode::InvalidReference, "No such layer.")
                    .detail("layer_id", id.0)
            })?;
            Some(id)
        }
    };
    Ok(Rule {
        prompt: clean_prompt(&args.prompt)?,
        kinds,
        layer,
        min,
        max,
    })
}

/// `selection.request`.
pub fn start(
    state: &mut AppState,
    instance: &str,
    caller: &Caller,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let args: SelectionRequestArgs = parse(arguments)?;
    sync(state);
    not_too_many(state, caller)?;
    // It would end the command the user is in the middle of.
    if let Some(runner) = state.runner.as_ref() {
        return Err(ApiError::command_busy(runner.name()));
    }
    // A dialog is the user's focus right now.
    if state.modal_open() {
        return Err(ApiError::new(
            ErrorCode::OperationBusy,
            "A dialog is open in OpenDraft. Ask the user to close it, then try again.",
        )
        .retryable());
    }
    let index = open_for_change(
        state,
        instance,
        &args.document_id,
        args.expected_revision.map(|r| r.0),
    )?;
    if index != state.active {
        return Err(ApiError::new(
            ErrorCode::InvalidReference,
            "The user picks in the active tab. Ask them to switch to that drawing.",
        )
        .detail("document_id", &args.document_id));
    }
    let rule = rule_from(&args, state.docs[index].session.document())?;
    let wait = args
        .timeout_seconds
        .unwrap_or(SELECTION_WAIT_DEFAULT_S)
        .clamp(1, SELECTION_WAIT_MAX_S);

    if !args.keep_current_selection {
        state.docs[index].selection.clear();
    }
    let answer: Slot = Arc::default();
    let given = Arc::clone(&answer);
    state.start_command(NAME, move |ed| ask(ed, &rule, &given));
    state.pump_command_for(FIRST_PROMPT_WAIT);
    let uid = state.docs[index].uid;
    let deadline = Instant::now() + Duration::from_secs(u64::from(wait));
    let id = state.file_ops.add_selection(
        caller.grant_id,
        caller.client,
        (uid, state.command_serial),
        deadline,
        answer,
    );
    sync(state);
    match state.file_ops.get(id) {
        Some(op) => encode(&op.status(instance)),
        None => Err(ApiError::new(
            ErrorCode::InternalError,
            "The operation vanished.",
        )),
    }
}

/// A request's command ended normally: take what it left and finish the
/// operation with it.
pub(super) fn settle(state: &mut AppState, id: u64) {
    let picked = state
        .file_ops
        .get(id)
        .and_then(|op| op.answer.as_ref())
        .and_then(|slot| slot.lock().ok()?.take());
    match picked {
        Some(picked) => {
            if let Some(op) = state.file_ops.get_mut(id) {
                op.revision = Some(picked.revision);
                op.selected = Some(picked.ids);
            }
            state.file_ops.succeed(id, Vec::new());
        }
        None => state.file_ops.fail(
            id,
            ErrorCode::OperationFailed,
            "The selection was lost before it could be passed on.",
        ),
    }
}

/// Stop the running requests that `unwanted` picks out by owner, for the
/// reason given: the prompt leaves the screen, and the user is told.
pub(crate) fn withdraw(state: &mut AppState, unwanted: impl Fn(u64) -> bool, reason: &str) {
    let mine: Vec<(u64, u64)> = state
        .file_ops
        .running_selections()
        .into_iter()
        .filter(|(_, grant, _)| unwanted(*grant))
        .map(|(id, _, serial)| (id, serial))
        .collect();
    for (id, serial) in mine {
        state.file_ops.cancel(id);
        if state.command_serial == serial && state.command_running() {
            state.cancel_command();
            state.tell(reason.to_owned());
        }
    }
}

/// End the requests whose time is up.
pub(super) fn expire(state: &mut AppState) {
    let late = state.file_ops.overdue_selections();
    for (id, serial) in late {
        state.file_ops.fail(
            id,
            ErrorCode::DeadlineExceeded,
            "The user did not finish selecting in time.",
        );
        if state.command_serial == serial && state.command_running() {
            state.cancel_command();
            state.tell("The assistant's request timed out.".to_owned());
        }
    }
}
