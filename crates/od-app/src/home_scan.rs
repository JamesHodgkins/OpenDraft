//! What the Home page knows about the files behind the recent list: whether
//! each exists, its date and size, which real file it is (so two spellings of
//! one path can be merged) and whether it is open in a tab. Looking costs disk
//! access, which can be slow on a removable or network drive, so a worker
//! thread does it and the page shows `Pending` until the answer arrives.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};

use crate::recent::{RecentFiles, RecentId};

/// How long a look at the files on disk is trusted.
const REFRESH: Duration = Duration::from_secs(3);

/// What is known about one file.
#[derive(Clone, Debug, PartialEq)]
pub enum FileState {
    /// Not looked at yet.
    Pending,
    Present {
        modified: Option<SystemTime>,
        size: u64,
    },
    /// Missing or unreadable.
    Missing,
}

/// One line of the recent-drawings list as the page shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct RecentEntry {
    pub id: RecentId,
    pub path: PathBuf,
    /// The real path, once the worker has worked it out.
    pub resolved: Option<PathBuf>,
    pub pinned: bool,
    pub state: FileState,
    /// The uid of the open drawing that is this file.
    pub open_uid: Option<u64>,
}

/// What to look at: the recent list and the saved paths of the open drawings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScanRequest {
    pub recent: Vec<(RecentId, PathBuf)>,
    pub open: Vec<(u64, PathBuf)>,
}

impl ScanRequest {
    pub fn new(recent: &RecentFiles, open: Vec<(u64, PathBuf)>) -> Self {
        Self {
            recent: recent
                .items()
                .iter()
                .map(|i| (i.id, i.path.clone()))
                .collect(),
            open,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scanned {
    pub id: RecentId,
    pub path: PathBuf,
    /// The canonical path, if it could be worked out.
    pub resolved: Option<PathBuf>,
    pub state: FileState,
    pub open_uid: Option<u64>,
}

/// Look at the files now (on the calling thread).
pub fn scan(request: &ScanRequest) -> Vec<Scanned> {
    // Two paths name one file when both canonicalise to the same path, else
    // only when they are written the same (as `files::same_file`).
    let open: Vec<(u64, &PathBuf, Option<PathBuf>)> = request
        .open
        .iter()
        .map(|(uid, path)| (*uid, path, std::fs::canonicalize(path).ok()))
        .collect();
    request
        .recent
        .iter()
        .map(|(id, path)| {
            let resolved = std::fs::canonicalize(path).ok();
            let meta = std::fs::metadata(path).ok().filter(|m| m.is_file());
            let state = match meta {
                Some(m) => FileState::Present {
                    modified: m.modified().ok(),
                    size: m.len(),
                },
                None => FileState::Missing,
            };
            let open_uid = open.iter().find_map(|(uid, raw, canonical)| {
                let same = match (&resolved, canonical) {
                    (Some(a), Some(b)) => a == b,
                    _ => *raw == path,
                };
                same.then_some(*uid)
            });
            Scanned {
                id: *id,
                path: path.clone(),
                resolved,
                state,
                open_uid,
            }
        })
        .collect()
}

struct Done {
    at: Instant,
    request: ScanRequest,
    found: Vec<Scanned>,
}

struct Running {
    request: ScanRequest,
    result: mpsc::Receiver<Vec<Scanned>>,
}

/// The latest scan, and the next one while it is under way.
#[derive(Default)]
pub struct HomeScan {
    done: Option<Done>,
    running: Option<Running>,
}

impl HomeScan {
    /// Take in a finished scan and start another if the request changed or
    /// the last one is old. Returns the files found to be the same file as
    /// another entry, for `RecentFiles::merge_resolved`, when a scan has just
    /// finished.
    pub fn update(
        &mut self,
        wanted: ScanRequest,
        ctx: &eframe::egui::Context,
    ) -> Option<Vec<(RecentId, PathBuf, PathBuf)>> {
        let mut resolved = None;
        if let Some(running) = &self.running {
            match running.result.try_recv() {
                Ok(found) => {
                    resolved = Some(
                        found
                            .iter()
                            .filter_map(|s| Some((s.id, s.path.clone(), s.resolved.clone()?)))
                            .collect(),
                    );
                    if let Some(running) = self.running.take() {
                        self.done = Some(Done {
                            at: Instant::now(),
                            request: running.request,
                            found,
                        });
                    }
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => self.running = None,
            }
        }
        let fresh = self
            .done
            .as_ref()
            .is_some_and(|d| d.request == wanted && d.at.elapsed() < REFRESH);
        let busy = self.running.as_ref().is_some_and(|r| r.request == wanted);
        if !fresh && !busy {
            let (tx, rx) = mpsc::channel();
            let request = wanted.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                // The page may have gone away; then nobody wants the answer.
                let _ = tx.send(scan(&wanted));
                ctx.request_repaint();
            });
            self.running = Some(Running {
                request,
                result: rx,
            });
        }
        ctx.request_repaint_after(REFRESH);
        resolved
    }

    /// The recent list as the page shows it.
    pub fn entries(&self, recent: &RecentFiles) -> Vec<RecentEntry> {
        recent
            .items()
            .iter()
            .map(|item| {
                let found = self.done.as_ref().and_then(|d| {
                    d.found
                        .iter()
                        .find(|s| s.id == item.id && s.path == item.path)
                });
                RecentEntry {
                    id: item.id,
                    path: item.path.clone(),
                    resolved: found.and_then(|s| s.resolved.clone()),
                    pinned: item.pinned,
                    state: found.map_or(FileState::Pending, |s| s.state.clone()),
                    open_uid: found.and_then(|s| s.open_uid),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("od-scan-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_present_file_has_a_size_and_a_missing_one_is_marked() {
        let dir = scratch("present");
        std::fs::write(dir.join("a.odx"), "12345").unwrap();
        let mut recent = RecentFiles::default();
        recent.add(&dir.join("a.odx"));
        recent.add(&dir.join("gone.odx"));
        let found = scan(&ScanRequest::new(&recent, Vec::new()));
        let state = |name: &str| {
            found
                .iter()
                .find(|s| s.path == dir.join(name))
                .unwrap()
                .state
                .clone()
        };
        assert!(matches!(state("a.odx"), FileState::Present { size: 5, .. }));
        assert_eq!(state("gone.odx"), FileState::Missing);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_spellings_of_one_file_resolve_to_one_path_and_the_open_tab_is_found() {
        let dir = scratch("spellings");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.odx"), "x").unwrap();
        let plain = dir.join("a.odx");
        let roundabout = dir.join("sub").join("..").join("a.odx");
        let mut recent = RecentFiles::default();
        recent.add(&plain);
        recent.add(&roundabout);
        let request = ScanRequest::new(&recent, vec![(7, plain.clone())]);
        let found = scan(&request);
        assert_eq!(found[0].resolved, found[1].resolved);
        assert!(found[0].resolved.is_some());
        assert!(found.iter().all(|s| s.open_uid == Some(7)));
        let merged: Vec<_> = found
            .iter()
            .filter_map(|s| Some((s.id, s.path.clone(), s.resolved.clone()?)))
            .collect();
        assert!(recent.merge_resolved(&merged));
        assert_eq!(recent.items().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unresolvable_paths_never_merge_and_untitled_drawings_never_match() {
        let mut recent = RecentFiles::default();
        recent.add(&PathBuf::from("no/such/a.odx"));
        recent.add(&PathBuf::from("no/such/../such/a.odx"));
        let found = scan(&ScanRequest::new(&recent, Vec::new()));
        assert!(found
            .iter()
            .all(|s| s.resolved.is_none() && s.open_uid.is_none()));
        let merged: Vec<_> = found
            .iter()
            .filter_map(|s| Some((s.id, s.path.clone(), s.resolved.clone()?)))
            .collect();
        assert!(!recent.merge_resolved(&merged));
        assert_eq!(recent.items().len(), 2);
    }

    #[test]
    fn entries_wait_as_pending_until_a_scan_for_their_path_arrives() {
        let mut recent = RecentFiles::default();
        recent.add(&PathBuf::from("a.odx"));
        let scan = HomeScan::default();
        assert_eq!(scan.entries(&recent)[0].state, FileState::Pending);
    }
}
