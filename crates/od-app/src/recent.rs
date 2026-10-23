//! The list of recently opened or saved drawings. Kept apart from `Settings`
//! because it changes as the user works, not in the dialog.
//!
//! Each entry is a record with an id that stays the same as the list moves,
//! so the Home page can key its widgets by it. Pinned entries come first;
//! within each group the most recently opened or saved is first.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

/// The newline-separated paths of the first versions.
const OLD_KEY: &str = "recent_files";
const KEY: &str = "recent_drawings";
const VERSION: u64 = 1;
/// How many drawings are remembered.
pub const MAX_RECENT: usize = 10;
/// How many of them may be pinned.
pub const MAX_PINNED: usize = 5;

/// Identifies one entry for as long as it stays on the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RecentId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentDrawing {
    pub id: RecentId,
    pub path: PathBuf,
    pub pinned: bool,
    pub last_opened: SystemTime,
}

/// How Home lists the recent drawings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RecentView {
    #[default]
    Grid,
    List,
}

/// Pinning was refused: `MAX_PINNED` are pinned already.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PinLimit;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentFiles {
    /// Pinned first, then by `last_opened`, newest first.
    items: Vec<RecentDrawing>,
    next_id: u64,
    pub view: RecentView,
}

impl Default for RecentFiles {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            next_id: 1,
            view: RecentView::default(),
        }
    }
}

impl RecentFiles {
    pub fn items(&self) -> &[RecentDrawing] {
        &self.items
    }

    pub fn paths(&self) -> Vec<PathBuf> {
        self.items.iter().map(|i| i.path.clone()).collect()
    }

    pub fn get(&self, id: RecentId) -> Option<&RecentDrawing> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn pinned_count(&self) -> usize {
        self.items.iter().filter(|i| i.pinned).count()
    }

    /// A time later than every one on the list, so the entry it is given
    /// sorts first however quickly things happen.
    fn stamp(&self) -> SystemTime {
        let newest = self.items.iter().map(|i| i.last_opened).max();
        // Whole milliseconds, as they are stored.
        let now = from_millis(millis(SystemTime::now()));
        match newest {
            Some(n) if n >= now => n + Duration::from_millis(1),
            _ => now,
        }
    }

    fn sort(&mut self) {
        self.items.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then(b.last_opened.cmp(&a.last_opened))
                .then(a.id.cmp(&b.id))
        });
        // Room is made by forgetting the least recent unpinned entry; at most
        // `MAX_PINNED` are pinned, so there is always one.
        while self.items.len() > MAX_RECENT {
            let Some(last) = self.items.iter().rposition(|i| !i.pinned) else {
                break;
            };
            self.items.remove(last);
        }
    }

    /// Put `path` first among the unpinned (or keep it among the pinned)
    /// after a successful open or save.
    pub fn add(&mut self, path: &Path) -> RecentId {
        let stamp = self.stamp();
        if let Some(item) = self.items.iter_mut().find(|i| i.path == path) {
            item.last_opened = stamp;
            let id = item.id;
            self.sort();
            return id;
        }
        let id = RecentId(self.next_id);
        self.next_id += 1;
        self.items.push(RecentDrawing {
            id,
            path: path.to_owned(),
            pinned: false,
            last_opened: stamp,
        });
        self.sort();
        id
    }

    /// As `add`, for a drawing the user pointed at to stand in for the entry
    /// `old`: that entry goes, and its pin passes to the new one.
    pub fn add_replacing(&mut self, path: &Path, old: Option<RecentId>) -> RecentId {
        let was_pinned = old
            .and_then(|old| self.get(old))
            .is_some_and(|item| item.pinned);
        if let Some(old) = old {
            if self.get(old).is_some_and(|item| item.path != path) {
                self.remove(old);
            }
        }
        let id = self.add(path);
        if was_pinned {
            if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
                item.pinned = true;
            }
            self.sort();
        }
        id
    }

    pub fn pin(&mut self, id: RecentId) -> Result<(), PinLimit> {
        if self.get(id).is_some_and(|i| !i.pinned) {
            if self.pinned_count() >= MAX_PINNED {
                return Err(PinLimit);
            }
            if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
                item.pinned = true;
            }
            self.sort();
        }
        Ok(())
    }

    pub fn unpin(&mut self, id: RecentId) {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.pinned = false;
        }
        self.sort();
    }

    /// Forget one entry (the file is not touched).
    pub fn remove(&mut self, id: RecentId) {
        self.items.retain(|i| i.id != id);
    }

    /// Forget every entry that is not pinned; how many went.
    pub fn clear_unpinned(&mut self) -> usize {
        let before = self.items.len();
        self.items.retain(|i| i.pinned);
        before - self.items.len()
    }

    /// Merge entries that were found to be the same file. Each group of
    /// `(id, stored path, resolved path)` with one resolved path becomes the
    /// entry opened last, which keeps its path and id, is pinned if any was,
    /// and (lowest id breaking a tie) is the one that survives. Entries whose
    /// stored path has changed since the lookup are left alone. True if the
    /// list changed.
    pub fn merge_resolved(&mut self, found: &[(RecentId, PathBuf, PathBuf)]) -> bool {
        let mut changed = false;
        for (_, _, resolved) in found {
            let group: Vec<RecentId> = found
                .iter()
                .filter(|(id, stored, r)| {
                    r == resolved && self.get(*id).is_some_and(|i| i.path == *stored)
                })
                .map(|(id, ..)| *id)
                .collect();
            if group.len() < 2 {
                continue;
            }
            let survivor = group
                .iter()
                .filter_map(|id| self.get(*id))
                .max_by(|a, b| a.last_opened.cmp(&b.last_opened).then(b.id.cmp(&a.id)))
                .map(|i| i.id);
            let Some(survivor) = survivor else { continue };
            let pinned = group
                .iter()
                .filter_map(|id| self.get(*id))
                .any(|i| i.pinned);
            for id in group.into_iter().filter(|id| *id != survivor) {
                self.remove(id);
            }
            if let Some(item) = self.items.iter_mut().find(|i| i.id == survivor) {
                item.pinned = pinned;
            }
            changed = true;
        }
        if changed {
            self.sort();
        }
        changed
    }

    /// Read the list: the current format, else the older newline-separated
    /// paths (kept in their order). Anything unreadable is left out rather
    /// than failing the rest.
    pub fn load(storage: Option<&dyn eframe::Storage>) -> Self {
        let Some(storage) = storage else {
            return Self::default();
        };
        if let Some(value) = storage
            .get_string(KEY)
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        {
            return Self::from_value(&value);
        }
        let old = storage.get_string(OLD_KEY).unwrap_or_default();
        let paths: Vec<PathBuf> = old
            .lines()
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .take(MAX_RECENT)
            .collect();
        let count = paths.len();
        let mut recent = Self {
            items: paths
                .into_iter()
                .enumerate()
                .map(|(i, path)| RecentDrawing {
                    id: RecentId(i as u64 + 1),
                    path,
                    pinned: false,
                    last_opened: from_millis((count - i) as u64),
                })
                .collect(),
            next_id: count as u64 + 1,
            view: RecentView::default(),
        };
        recent.sort();
        recent
    }

    fn from_value(value: &Value) -> Self {
        let stored = value.get("items").and_then(Value::as_array);
        let count = stored.map_or(0, Vec::len);
        let mut items: Vec<RecentDrawing> = stored
            .into_iter()
            .flatten()
            .enumerate()
            .filter_map(|(i, v)| {
                let path = PathBuf::from(v.get("path")?.as_str()?);
                Some(RecentDrawing {
                    id: RecentId(v.get("id").and_then(Value::as_u64).unwrap_or(0)),
                    path,
                    pinned: v.get("pinned").and_then(Value::as_bool).unwrap_or(false),
                    // Without a time, the stored order stands.
                    last_opened: from_millis(
                        v.get("last_opened")
                            .and_then(Value::as_u64)
                            .unwrap_or((count - i) as u64),
                    ),
                })
            })
            .collect();
        let mut next_id = value.get("next_id").and_then(Value::as_u64).unwrap_or(0);
        repair_ids(&mut items, &mut next_id);
        let mut pinned = 0;
        for item in &mut items {
            if item.pinned {
                pinned += 1;
                item.pinned = pinned <= MAX_PINNED;
            }
        }
        let view = match value.get("view").and_then(Value::as_str) {
            Some("list") => RecentView::List,
            _ => RecentView::Grid,
        };
        let mut recent = Self {
            items,
            next_id,
            view,
        };
        recent.sort();
        recent
    }

    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        let items: Vec<Value> = self
            .items
            .iter()
            .map(|i| {
                json!({
                    "id": i.id.0,
                    "path": i.path.to_string_lossy(),
                    "pinned": i.pinned,
                    "last_opened": millis(i.last_opened),
                })
            })
            .collect();
        let view = match self.view {
            RecentView::Grid => "grid",
            RecentView::List => "list",
        };
        let value = json!({
            "version": VERSION,
            "next_id": self.next_id,
            "view": view,
            "items": items,
        });
        storage.set_string(KEY, value.to_string());
    }
}

/// Make every id non-zero and unique, in stored order: the first to use an
/// id keeps it, a repeat or a zero takes the next free one.
fn repair_ids(items: &mut [RecentDrawing], next_id: &mut u64) {
    let top = items.iter().map(|i| i.id.0).max().unwrap_or(0);
    if top == u64::MAX {
        for (i, item) in items.iter_mut().enumerate() {
            item.id = RecentId(i as u64 + 1);
        }
        *next_id = items.len() as u64 + 1;
        return;
    }
    *next_id = (*next_id).max(top + 1);
    let mut used = std::collections::HashSet::new();
    for item in items {
        if item.id.0 == 0 || !used.insert(item.id) {
            item.id = RecentId(*next_id);
            *next_id += 1;
            used.insert(item.id);
        }
    }
}

fn millis(at: SystemTime) -> u64 {
    at.duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis().try_into().unwrap_or(u64::MAX))
}

fn from_millis(ms: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_millis(ms)
}

#[cfg(test)]
#[path = "recent_tests.rs"]
mod tests;
