//! The pictures the Home page is showing, as textures: which recent drawing
//! has which, loaded by the preview worker (never on the UI thread), kept
//! within a fixed number, and held over a save until the new picture is ready
//! so a card does not flash.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::Receiver;

use eframe::egui;

use crate::home_scan::{FileState, RecentEntry};
use crate::previews::{FileKey, Output, PreviewHandle};
use crate::recent::RecentId;

/// Decoded pictures kept: ten cards and a few spare. At 512 x 320 x 4 bytes
/// (640 KiB) each, this is 10 MiB of texture memory at most.
pub const MAX_TEXTURES: usize = 16;

struct Shown {
    key: FileKey,
    texture: egui::TextureHandle,
    last_used: u64,
}

#[derive(Default)]
pub struct PreviewTextures {
    link: Option<(PreviewHandle, Receiver<Output>)>,
    shown: HashMap<RecentId, Shown>,
    /// What each entry has asked the worker for, so it asks once.
    asked: HashMap<RecentId, FileKey>,
    /// Entries whose own save has been drawn anew and whose picture is
    /// being read: their old picture stays until it comes.
    waiting: HashSet<RecentId>,
    frame: u64,
}

/// What a picture would be called for this entry, if its file is known.
pub fn key_of(entry: &RecentEntry) -> Option<FileKey> {
    let FileState::Present {
        modified: Some(modified),
        size,
    } = &entry.state
    else {
        return None;
    };
    let path = entry.resolved.as_ref().unwrap_or(&entry.path);
    Some(FileKey::new(path, *modified, *size))
}

impl PreviewTextures {
    /// Start the preview worker in the cache folder beside the settings, and
    /// leave its handle in `slot` for the file code to use. With no settings
    /// folder there are no previews.
    pub fn start(slot: &mut Option<PreviewHandle>) -> Self {
        match crate::profile::app_data() {
            Some(dir) => {
                let (handle, results) = crate::previews::start(dir.join("previews"));
                *slot = Some(handle.clone());
                Self::new(handle, results)
            }
            None => Self::default(),
        }
    }

    pub fn new(handle: PreviewHandle, results: Receiver<Output>) -> Self {
        Self {
            link: Some((handle, results)),
            ..Self::default()
        }
    }

    /// Take in what the worker has finished and ask for what is missing.
    /// Returns the picture to show for each entry that has one.
    pub fn update(
        &mut self,
        ctx: &egui::Context,
        entries: &[RecentEntry],
    ) -> HashMap<RecentId, egui::TextureHandle> {
        let Self {
            link,
            shown,
            asked,
            waiting,
            frame,
        } = self;
        let Some((handle, results)) = link else {
            return HashMap::new();
        };
        handle.set_context(ctx);
        // Taken before the reports are read: see `pending_paths`.
        let pending = handle.pending_paths();
        *frame += 1;
        let frame = *frame;
        while let Ok(output) = results.try_recv() {
            match output {
                Output::Rendered(path) => {
                    // A new picture may exist: ask again for those it is for,
                    // keeping what they show until it is read.
                    asked.retain(|id, key| {
                        let stale = key.path == path;
                        if stale && shown.contains_key(id) {
                            waiting.insert(*id);
                        }
                        !stale
                    });
                }
                // An answer to a question no longer asked.
                Output::Loaded { id, key, .. } if asked.get(&id) != Some(&key) => {}
                Output::Loaded {
                    id, image: None, ..
                } => {
                    waiting.remove(&id);
                }
                Output::Loaded {
                    id,
                    key,
                    image: Some(image),
                } => {
                    waiting.remove(&id);
                    let texture = ctx.load_texture(
                        format!("preview-{}", id.0),
                        image,
                        // Mipmapped: a card shows it at under half size, and thin lines
                        // would shimmer if it were only filtered.
                        egui::TextureOptions::LINEAR
                            .with_mipmap_mode(Some(egui::TextureFilter::Linear)),
                    );
                    shown.insert(
                        id,
                        Shown {
                            key,
                            texture,
                            last_used: frame,
                        },
                    );
                }
            }
        }
        // Only so many are ever pictured: the first ones, which are on show.
        let entries = &entries[..entries.len().min(MAX_TEXTURES)];
        let listed: HashSet<RecentId> = entries.iter().map(|e| e.id).collect();
        shown.retain(|id, _| listed.contains(id));
        asked.retain(|id, _| listed.contains(id));
        waiting.retain(|id| listed.contains(id));
        let mut current = HashMap::new();
        for entry in entries {
            let key = key_of(entry);
            if let Some(pic) = shown.get_mut(&entry.id) {
                if Some(&pic.key) == key.as_ref() {
                    pic.last_used = frame;
                    current.insert(entry.id, pic.texture.clone());
                    continue;
                }
                // The file has changed. If it is our own save, keep the old
                // picture until the new one is ready; if another program
                // changed it, the old one no longer shows.
                if pending.contains(&pic.key.path) || waiting.contains(&entry.id) {
                    pic.last_used = frame;
                    current.insert(entry.id, pic.texture.clone());
                } else {
                    shown.remove(&entry.id);
                }
            }
            if let Some(key) = key {
                if asked.get(&entry.id) != Some(&key) {
                    asked.insert(entry.id, key.clone());
                    handle.load(entry.id, key);
                }
            }
        }
        while shown.len() > MAX_TEXTURES {
            let oldest = shown
                .iter()
                .filter(|(id, _)| !current.contains_key(id))
                .min_by_key(|(_, s)| s.last_used)
                .map(|(id, _)| *id);
            let Some(oldest) = oldest else { break };
            shown.remove(&oldest);
            asked.remove(&oldest);
        }
        current
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use od_core::{Document, Entity, Point};

    use super::*;
    use crate::previews::{identity, start};

    fn entry_for(id: u64, path: &Path) -> RecentEntry {
        let meta = std::fs::metadata(path).unwrap();
        RecentEntry {
            id: RecentId(id),
            path: path.to_owned(),
            resolved: Some(identity(path)),
            pinned: false,
            state: FileState::Present {
                modified: meta.modified().ok(),
                size: meta.len(),
            },
            open_uid: None,
        }
    }

    fn drawing() -> Document {
        let mut doc = Document::default();
        doc.add(Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: 5.0,
        });
        doc
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("od-textures-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Run frames until `done` holds or a time limit passes.
    fn until(
        textures: &mut PreviewTextures,
        ctx: &egui::Context,
        entries: &[RecentEntry],
        done: impl Fn(&HashMap<RecentId, egui::TextureHandle>) -> bool,
    ) -> HashMap<RecentId, egui::TextureHandle> {
        let started = Instant::now();
        loop {
            let shown = textures.update(ctx, entries);
            if done(&shown) || started.elapsed() > Duration::from_secs(20) {
                return shown;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn a_saved_drawing_gets_its_picture_and_keeps_the_old_one_until_the_new_is_ready() {
        let dir = scratch("flow");
        let file = dir.join("plan.odx");
        std::fs::write(&file, "one").unwrap();
        let (handle, results) = start(dir.join("cache"));
        let mut textures = PreviewTextures::new(handle.clone(), results);
        let ctx = egui::Context::default();
        // Nothing drawn yet: no picture, and the first look asks the worker.
        handle.request(&file, drawing());
        let entries = [entry_for(1, &file)];
        let shown = until(&mut textures, &ctx, &entries, |s| {
            s.contains_key(&RecentId(1))
        });
        assert!(shown.contains_key(&RecentId(1)), "the picture arrived");
        // The file is saved again; its picture is being remade. Until it is
        // ready the old one stays, with no gap in between.
        std::fs::write(&file, "two, longer").unwrap();
        let changed = [entry_for(1, &file)];
        handle.request(&file, drawing());
        let held = textures.update(&ctx, &changed);
        assert!(held.contains_key(&RecentId(1)), "held over the save");
        let fresh = until(&mut textures, &ctx, &changed, |_| {
            !handle.is_pending(&identity(&file))
        });
        assert!(fresh.contains_key(&RecentId(1)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_change_by_another_program_drops_the_old_picture_at_once() {
        let dir = scratch("external");
        let file = dir.join("plan.odx");
        std::fs::write(&file, "one").unwrap();
        let (handle, results) = start(dir.join("cache"));
        let mut textures = PreviewTextures::new(handle.clone(), results);
        let ctx = egui::Context::default();
        handle.request(&file, drawing());
        let entries = [entry_for(1, &file)];
        let shown = until(&mut textures, &ctx, &entries, |s| {
            s.contains_key(&RecentId(1))
        });
        assert!(shown.contains_key(&RecentId(1)));
        std::fs::write(&file, "edited elsewhere").unwrap();
        let shown = textures.update(&ctx, &[entry_for(1, &file)]);
        assert!(shown.is_empty(), "no grace period for an outside edit");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_file_or_one_not_yet_looked_at_has_no_picture_and_asks_for_none() {
        let (handle, results) = start(scratch("none").join("cache"));
        let mut textures = PreviewTextures::new(handle, results);
        let ctx = egui::Context::default();
        let mut entry = entry_for_nothing();
        assert!(textures.update(&ctx, &[entry.clone()]).is_empty());
        entry.state = FileState::Missing;
        assert!(textures.update(&ctx, &[entry]).is_empty());
        assert!(textures.asked.is_empty());
    }

    fn entry_for_nothing() -> RecentEntry {
        RecentEntry {
            id: RecentId(9),
            path: PathBuf::from("nowhere.odx"),
            resolved: None,
            pinned: false,
            state: FileState::Pending,
            open_uid: None,
        }
    }

    #[test]
    fn removed_entries_and_the_limit_free_their_textures() {
        let dir = scratch("limit");
        let (handle, results) = start(dir.join("cache"));
        let mut textures = PreviewTextures::new(handle.clone(), results);
        let ctx = egui::Context::default();
        let mut entries = Vec::new();
        for i in 0..MAX_TEXTURES as u64 + 4 {
            let file = dir.join(format!("{i}.odx"));
            std::fs::write(&file, format!("drawing {i}")).unwrap();
            handle.request(&file, drawing());
            entries.push(entry_for(i + 1, &file));
        }
        let all = entries.len();
        let _ = until(&mut textures, &ctx, &entries, |s| s.len() >= MAX_TEXTURES);
        std::thread::sleep(Duration::from_millis(300));
        let _ = textures.update(&ctx, &entries);
        assert!(
            textures.shown.len() <= MAX_TEXTURES,
            "{} of {all}",
            textures.shown.len()
        );
        let _ = textures.update(&ctx, &entries[..1]);
        assert!(
            textures.shown.len() <= 1,
            "entries off the list lose their textures"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
