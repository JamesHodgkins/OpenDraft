//! A short, bounded log of what happened in the app, for callers that poll
//! (`events.poll`). It watches the drawings once a frame instead of hooking
//! every edit, so it sees the user's changes and the app's own alike without
//! the editing code knowing about it. It says that something changed and
//! where to look; the revision and `document.get` give the rest.

use std::collections::{HashMap, VecDeque};

use od_api::event_ops::{EventDto, EventKind, EventsResult};
use od_api::limits::QUERY_PAGE_MAX;
use od_api::WireId;

use super::document_id;
use crate::state::Doc;

/// Events kept. A caller that falls further behind is told to resync.
const KEPT: usize = 256;
/// Longest text carried in an event.
const TEXT_MAX: usize = 80;

struct Entry {
    seq: u64,
    kind: EventKind,
    document: Option<u64>,
    revision: Option<u64>,
    detail: Option<String>,
}

#[derive(Default)]
pub struct EventLog {
    next: u64,
    entries: VecDeque<Entry>,
    /// What each drawing looked like when last looked at: revision and
    /// whether it was modified.
    seen: HashMap<u64, (u64, bool)>,
    started: bool,
    /// The command serial last seen running, if any.
    running: Option<(u64, String)>,
}

/// Text from the app or a caller, made safe to pass on: no control
/// characters, bounded.
fn tidy(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(TEXT_MAX)
        .collect()
}

impl EventLog {
    fn push(
        &mut self,
        kind: EventKind,
        document: Option<u64>,
        revision: Option<u64>,
        detail: Option<String>,
    ) {
        self.next += 1;
        if self.entries.len() >= KEPT {
            self.entries.pop_front();
        }
        self.entries.push_back(Entry {
            seq: self.next,
            kind,
            document,
            revision,
            detail: detail.map(|d| tidy(&d)),
        });
    }

    /// Look at the drawings and the running command and note what is new.
    /// The first look only records where things stand.
    pub fn observe(&mut self, docs: &[Doc], serial: u64, running: Option<&str>) {
        let baseline = !self.started;
        self.started = true;
        let mut present = Vec::with_capacity(docs.len());
        for doc in docs.iter().filter(|d| !d.placeholder) {
            present.push(doc.uid);
            // A drawing in the Block Editor is not the drawing: say nothing
            // about its temporary session.
            if doc.block_edit.is_some() {
                continue;
            }
            let state = (doc.session.revision(), doc.is_modified());
            match self.seen.insert(doc.uid, state) {
                None if !baseline => {
                    self.push(
                        EventKind::DocumentOpened,
                        Some(doc.uid),
                        Some(state.0),
                        Some(doc.title.clone()),
                    );
                }
                Some(before) if before.0 != state.0 => {
                    let step = doc.session.history().top_undo().map(|s| s.name);
                    self.push(
                        EventKind::DocumentChanged,
                        Some(doc.uid),
                        Some(state.0),
                        step,
                    );
                }
                Some(before) if before.1 != state.1 => {
                    let kind = if state.1 {
                        EventKind::DocumentModified
                    } else {
                        EventKind::DocumentSaved
                    };
                    self.push(kind, Some(doc.uid), Some(state.0), None);
                }
                _ => {}
            }
        }
        let gone: Vec<u64> = self
            .seen
            .keys()
            .copied()
            .filter(|uid| !present.contains(uid))
            .collect();
        for uid in gone {
            self.seen.remove(&uid);
            self.push(EventKind::DocumentClosed, Some(uid), None, None);
        }
        match (&self.running, running) {
            (None, Some(name)) if !baseline => {
                self.running = Some((serial, name.to_owned()));
                self.push(EventKind::CommandStarted, None, None, Some(name.to_owned()));
            }
            (None, Some(name)) => self.running = Some((serial, name.to_owned())),
            (Some((was, name)), now) if now.is_none() || *was != serial => {
                let name = name.clone();
                self.push(EventKind::CommandEnded, None, None, Some(name));
                self.running = now.map(|n| (serial, n.to_owned()));
                if let Some(n) = now {
                    self.push(EventKind::CommandStarted, None, None, Some(n.to_owned()));
                }
            }
            _ => {}
        }
    }

    /// Events after `since` (all kept ones if `None`), oldest first, at
    /// most `limit`. `reset` says the caller missed some and should reread
    /// the drawings.
    pub fn since(&self, instance: &str, since: Option<u64>, limit: usize) -> EventsResult {
        let oldest = self.entries.front().map_or(self.next + 1, |e| e.seq);
        let after = since.unwrap_or(0);
        let reset = since.is_some() && after + 1 < oldest;
        let limit = limit.clamp(1, QUERY_PAGE_MAX);
        let mut events: Vec<EventDto> = self
            .entries
            .iter()
            .filter(|e| e.seq > after)
            .take(limit)
            .map(|e| EventDto {
                seq: WireId(e.seq),
                kind: e.kind,
                document_id: e.document.map(|uid| document_id(instance, uid)),
                revision: e.revision.map(WireId),
                detail: e.detail.clone(),
            })
            .collect();
        let next = events
            .last()
            .map_or(after.max(oldest.saturating_sub(1)), |e| e.seq.0);
        let more = self.entries.back().is_some_and(|e| e.seq > next);
        events.shrink_to_fit();
        EventsResult {
            events,
            next_seq: WireId(next),
            more,
            reset,
        }
    }
}
