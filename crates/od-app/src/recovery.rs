//! Autosave files and finding the ones a crash left behind.
//!
//! Each running instance owns a session folder under the recovery root and
//! holds an exclusive lock on a file in it for as long as it lives. A folder
//! whose lock can be taken therefore belongs to an instance that is gone
//! (crashed or killed): its drawings are offered back at the next start. A
//! normal exit deletes the folder, so only a crash leaves anything.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use od_core::Document;
use serde::{Deserialize, Serialize};

const LOCK_FILE: &str = "lock";
const SESSION_PREFIX: &str = "session-";
/// Tells apart sessions started in the same process in the same millisecond.
static SESSIONS_STARTED: AtomicU64 = AtomicU64::new(0);

/// What is remembered about an autosaved drawing besides its content.
#[derive(Serialize, Deserialize)]
struct Info {
    title: String,
    /// Where the drawing lived on disk, if it had been saved.
    original: Option<PathBuf>,
}

/// This run's autosave folder. Dropping it (a normal exit) deletes it.
pub struct Recovery {
    dir: PathBuf,
    lock: Option<File>,
    /// Leave the folder behind when dropped, as a crash would (tests).
    keep: bool,
}

/// A drawing found in a crashed session.
#[derive(Clone, Debug)]
pub struct Recovered {
    pub title: String,
    pub original: Option<PathBuf>,
    /// The autosaved copy.
    pub file: PathBuf,
    /// When it was last autosaved.
    pub saved: Option<SystemTime>,
}

/// A crashed session's leftovers, until they are recovered or discarded.
pub struct Orphan {
    dir: PathBuf,
    pub items: Vec<Recovered>,
}

impl Orphan {
    /// Delete the autosaved copies for good.
    pub fn discard(self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn file_name(uid: u64, extension: &str) -> String {
    format!("doc-{uid}.{extension}")
}

impl Recovery {
    /// Start this run's session folder under `root`.
    pub fn open(root: &Path) -> io::Result<Self> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        let count = SESSIONS_STARTED.fetch_add(1, Ordering::Relaxed);
        let dir = root.join(format!(
            "{SESSION_PREFIX}{}-{stamp}-{count}",
            std::process::id()
        ));
        fs::create_dir_all(&dir)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(dir.join(LOCK_FILE))?;
        lock.try_lock()
            .map_err(|e| io::Error::other(e.to_string()))?;
        Ok(Self {
            dir,
            lock: Some(lock),
            keep: false,
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Write the current state of drawing `uid`, replacing its last autosave.
    pub fn write(
        &self,
        uid: u64,
        title: &str,
        original: Option<&Path>,
        document: &Document,
    ) -> io::Result<()> {
        let info = Info {
            title: title.to_owned(),
            original: original.map(Path::to_owned),
        };
        let text = ron::to_string(&info).map_err(io::Error::other)?;
        od_files::save(&self.dir.join(file_name(uid, "odx")), document)
            .map_err(io::Error::other)?;
        fs::write(self.dir.join(file_name(uid, "info")), text)
    }

    /// Forget drawing `uid`'s autosave (it was saved, closed or is clean).
    pub fn remove(&self, uid: u64) {
        let _ = fs::remove_file(self.dir.join(file_name(uid, "odx")));
        let _ = fs::remove_file(self.dir.join(file_name(uid, "info")));
    }

    /// The sessions under `root`, other than `own`, whose instance is gone and
    /// which left drawings behind. Empty leftovers are cleaned up silently.
    pub fn find_orphans(root: &Path, own: &Path) -> Vec<Orphan> {
        let Ok(entries) = fs::read_dir(root) else {
            return Vec::new();
        };
        let mut orphans = Vec::new();
        for entry in entries.flatten() {
            let dir = entry.path();
            let is_session = dir
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with(SESSION_PREFIX));
            if !is_session || !dir.is_dir() || dir == own {
                continue;
            }
            if !owner_is_gone(&dir) {
                continue;
            }
            let items = read_items(&dir);
            if items.is_empty() {
                let _ = fs::remove_dir_all(&dir);
            } else {
                orphans.push(Orphan { dir, items });
            }
        }
        orphans.sort_by(|a, b| a.dir.cmp(&b.dir));
        orphans
    }

    /// Behave as if the app had crashed: release the lock, keep the files.
    #[cfg(test)]
    pub(crate) fn abandon(mut self) {
        self.lock = None;
        self.keep = true;
    }
}

impl Drop for Recovery {
    fn drop(&mut self) {
        // Release the lock first: a folder holding an open file cannot be
        // deleted on every system.
        self.lock = None;
        if !self.keep {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }
}

/// Whether nothing holds the session's lock any more.
fn owner_is_gone(dir: &Path) -> bool {
    match OpenOptions::new().write(true).open(dir.join(LOCK_FILE)) {
        Ok(file) => file.try_lock().is_ok(),
        // No lock file: the session never finished starting.
        Err(_) => true,
    }
}

fn read_items(dir: &Path) -> Vec<Recovered> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "odx"))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|file| {
            let info = fs::read_to_string(file.with_extension("info"))
                .ok()
                .and_then(|text| ron::from_str::<Info>(&text).ok());
            let (title, original) = match info {
                Some(info) => (info.title, info.original),
                None => (
                    file.file_stem()
                        .map_or_else(String::new, |s| s.to_string_lossy().into_owned()),
                    None,
                ),
            };
            let saved = fs::metadata(&file).and_then(|m| m.modified()).ok();
            Recovered {
                title,
                original,
                file,
                saved,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use od_core::{Entity, Point};

    use super::*;

    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("od-rec-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Scratch(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn drawing() -> Document {
        Document::from_entities([Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(1.0, 1.0),
        }])
    }

    #[test]
    fn a_crashed_session_is_found_with_its_drawings() {
        let root = Scratch::new("crash");
        let session = Recovery::open(&root.0).unwrap();
        session.write(1, "Drawing1", None, &drawing()).unwrap();
        session
            .write(2, "plan", Some(Path::new("C:/work/plan.odx")), &drawing())
            .unwrap();
        session.abandon();

        let next = Recovery::open(&root.0).unwrap();
        let orphans = Recovery::find_orphans(&root.0, next.dir());
        assert_eq!(orphans.len(), 1);
        let items = &orphans[0].items;
        assert_eq!(items.len(), 2);
        assert_eq!(
            (items[0].title.as_str(), items[0].original.as_deref()),
            ("Drawing1", None)
        );
        assert_eq!(items[1].title, "plan");
        assert_eq!(
            items[1].original.as_deref(),
            Some(Path::new("C:/work/plan.odx"))
        );
        assert_eq!(od_files::load(&items[0].file).unwrap().document, drawing());
        assert!(items[0].saved.is_some());
    }

    #[test]
    fn a_running_session_is_not_an_orphan() {
        let root = Scratch::new("live");
        let first = Recovery::open(&root.0).unwrap();
        first.write(1, "Drawing1", None, &drawing()).unwrap();
        let second = Recovery::open(&root.0).unwrap();
        assert!(
            Recovery::find_orphans(&root.0, second.dir()).is_empty(),
            "the other instance still holds its lock"
        );
    }

    #[test]
    fn a_normal_exit_leaves_nothing_behind() {
        let root = Scratch::new("clean");
        let session = Recovery::open(&root.0).unwrap();
        session.write(1, "Drawing1", None, &drawing()).unwrap();
        let dir = session.dir().to_owned();
        drop(session);
        assert!(!dir.exists());
        let next = Recovery::open(&root.0).unwrap();
        assert!(Recovery::find_orphans(&root.0, next.dir()).is_empty());
    }

    #[test]
    fn removing_an_autosave_forgets_that_drawing_only() {
        let root = Scratch::new("remove");
        let session = Recovery::open(&root.0).unwrap();
        session.write(1, "one", None, &drawing()).unwrap();
        session.write(2, "two", None, &drawing()).unwrap();
        session.remove(1);
        session.abandon();
        let next = Recovery::open(&root.0).unwrap();
        let orphans = Recovery::find_orphans(&root.0, next.dir());
        let titles: Vec<_> = orphans[0].items.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, ["two"]);
    }

    #[test]
    fn writing_again_replaces_the_autosave() {
        let root = Scratch::new("rewrite");
        let session = Recovery::open(&root.0).unwrap();
        session.write(1, "d", None, &drawing()).unwrap();
        let mut bigger = drawing();
        bigger.add(Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: 2.0,
        });
        session.write(1, "d", None, &bigger).unwrap();
        session.abandon();
        let next = Recovery::open(&root.0).unwrap();
        let orphans = Recovery::find_orphans(&root.0, next.dir());
        assert_eq!(
            od_files::load(&orphans[0].items[0].file)
                .unwrap()
                .document
                .len(),
            2
        );
    }

    #[test]
    fn an_empty_leftover_session_is_cleaned_up_not_offered() {
        let root = Scratch::new("empty");
        let session = Recovery::open(&root.0).unwrap();
        let dir = session.dir().to_owned();
        session.abandon();
        let next = Recovery::open(&root.0).unwrap();
        assert!(Recovery::find_orphans(&root.0, next.dir()).is_empty());
        assert!(!dir.exists());
    }

    #[test]
    fn discarding_deletes_the_leftovers() {
        let root = Scratch::new("discard");
        let session = Recovery::open(&root.0).unwrap();
        session.write(1, "d", None, &drawing()).unwrap();
        session.abandon();
        let next = Recovery::open(&root.0).unwrap();
        let mut orphans = Recovery::find_orphans(&root.0, next.dir());
        let orphan = orphans.remove(0);
        let file = orphan.items[0].file.clone();
        orphan.discard();
        assert!(!file.exists());
        assert!(Recovery::find_orphans(&root.0, next.dir()).is_empty());
    }

    #[test]
    fn a_missing_info_file_still_recovers_the_drawing() {
        let root = Scratch::new("noinfo");
        let session = Recovery::open(&root.0).unwrap();
        session.write(7, "named", None, &drawing()).unwrap();
        fs::remove_file(session.dir().join("doc-7.info")).unwrap();
        session.abandon();
        let next = Recovery::open(&root.0).unwrap();
        let orphans = Recovery::find_orphans(&root.0, next.dir());
        assert_eq!(orphans[0].items[0].title, "doc-7");
    }

    #[test]
    fn a_missing_root_is_not_an_error() {
        let orphans = Recovery::find_orphans(Path::new("/definitely/not/here"), Path::new("x"));
        assert!(orphans.is_empty());
    }
}
