//! Previews of drawings for the Home page: PNG pictures kept in a cache folder
//! beside the settings, drawn by `od_canvas::preview` on a worker thread after
//! a drawing has opened or saved, never by loading a drawing just to picture it.
//!
//! A picture is named by what the file was when it was drawn (its real path,
//! date, length and the renderer's version, [`FileKey`]), so another program
//! changing the file simply stops it matching. The folder is bounded by file
//! count and total size; the oldest go first.

use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use eframe::egui;
use od_canvas::preview::{self, Preview};
use od_core::Document;

use crate::recent::RecentId;

/// Bumped when the renderer's output changes on purpose, so old pictures are
/// not shown.
pub const RENDERER_VERSION: u32 = 3;
/// What the cache folder may hold, measured on real drawings (see `ui_plan.md`).
const MAX_FILES: usize = 64;
const MAX_BYTES: u64 = 16 * 1024 * 1024;
/// A picture larger than this is not kept (the card shows its placeholder).
const MAX_PNG_BYTES: usize = 150 * 1024;

/// What a file was when its picture was made.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FileKey {
    /// The real path ([`identity`]).
    pub path: PathBuf,
    pub modified_ms: u64,
    pub len: u64,
}

impl FileKey {
    pub fn new(path: &Path, modified: SystemTime, len: u64) -> Self {
        Self {
            path: identity(path),
            modified_ms: modified
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_millis().try_into().unwrap_or(u64::MAX)),
            len,
        }
    }

    /// The file's key as it is now, `None` if it cannot be read.
    fn of_file(path: &Path) -> Option<Self> {
        let meta = std::fs::metadata(path).ok().filter(|m| m.is_file())?;
        Some(Self::new(path, meta.modified().ok()?, meta.len()))
    }

    fn file_name(&self) -> String {
        // FNV-1a: names need to be stable between runs, which `DefaultHasher`
        // does not promise.
        let text = format!(
            "{RENDERER_VERSION}|{}|{}|{}",
            self.path.display(),
            self.modified_ms,
            self.len
        );
        let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
        });
        format!("{hash:016x}.png")
    }
}

/// The real path of `path`, or as written when it cannot be worked out (the
/// same rule as `files::same_file`).
pub fn identity(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned())
}

enum Job {
    Render {
        path: PathBuf,
        document: Box<Document>,
    },
    Load {
        id: RecentId,
        key: FileKey,
    },
}

/// What the worker reports.
pub enum Output {
    /// A picture of the file at this real path is made, or could not be.
    Rendered(PathBuf),
    /// The picture for `key`, if the cache has one.
    Loaded {
        id: RecentId,
        key: FileKey,
        image: Option<egui::ColorImage>,
    },
}

struct Shared {
    dir: PathBuf,
    /// Real paths with a picture being made, and how many.
    pending: Mutex<HashMap<PathBuf, usize>>,
    ctx: OnceLock<egui::Context>,
}

/// How the rest of the app asks for pictures; cheap to clone.
#[derive(Clone)]
pub struct PreviewHandle {
    jobs: Sender<Job>,
    shared: Arc<Shared>,
}

/// Start the worker thread. `dir` is created when the first picture is kept.
pub fn start(dir: PathBuf) -> (PreviewHandle, Receiver<Output>) {
    let (jobs, queue) = channel::<Job>();
    let (out, results) = channel();
    let shared = Arc::new(Shared {
        dir,
        pending: Mutex::new(HashMap::new()),
        ctx: OnceLock::new(),
    });
    let worker = shared.clone();
    std::thread::spawn(move || {
        for job in queue {
            let (output, rendered) = match job {
                Job::Render { path, document } => {
                    let ident = render_job(&worker.dir, &path, &document);
                    (Output::Rendered(ident.clone()), Some(ident))
                }
                Job::Load { id, key } => (
                    Output::Loaded {
                        image: load(&worker.dir, &key),
                        id,
                        key,
                    },
                    None,
                ),
            };
            let sent = out.send(output).is_ok();
            // Reported first, then no longer pending: a look that finds a
            // path not pending knows its report is already queued.
            if let Some(ident) = rendered {
                finished(&worker, &ident);
            }
            if !sent {
                break;
            }
            if let Some(ctx) = worker.ctx.get() {
                ctx.request_repaint();
            }
        }
    });
    (PreviewHandle { jobs, shared }, results)
}

impl PreviewHandle {
    /// Have the worker wake the window when it has something.
    pub fn set_context(&self, ctx: &egui::Context) {
        let _ = self.shared.ctx.set(ctx.clone());
    }

    /// The real paths whose pictures are being made. A picture is reported
    /// (`Output::Rendered`) before its path stops being pending, so what was
    /// pending in this list when it was taken, and is not now, has been
    /// reported already.
    pub fn pending_paths(&self) -> std::collections::HashSet<PathBuf> {
        self.shared
            .pending
            .lock()
            .map(|p| p.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Whether a picture of the file at real path `path` is being made.
    #[cfg(test)]
    pub fn is_pending(&self, path: &Path) -> bool {
        self.shared
            .pending
            .lock()
            .is_ok_and(|p| p.get(path).is_some_and(|n| *n > 0))
    }

    /// Make a picture of `document`, which has just been written to `path`.
    pub fn request(&self, path: &Path, document: Document) {
        let ident = identity(path);
        if let Ok(mut pending) = self.shared.pending.lock() {
            *pending.entry(ident).or_insert(0) += 1;
        }
        let job = Job::Render {
            path: path.to_owned(),
            document: Box::new(document),
        };
        if self.jobs.send(job).is_err() {
            self.finished(&identity(path));
        }
    }

    /// As `request` for a drawing just opened, unless the cache already has a
    /// picture of the file as it is (`document` is then not asked for).
    pub fn request_unless_stored(&self, path: &Path, document: impl FnOnce() -> Document) {
        let stored = FileKey::of_file(path)
            .is_some_and(|key| self.shared.dir.join(key.file_name()).is_file());
        if !stored {
            self.request(path, document());
        }
    }

    /// Ask for the picture for `key`; the answer is an [`Output::Loaded`].
    pub fn load(&self, id: RecentId, key: FileKey) {
        let _ = self.jobs.send(Job::Load { id, key });
    }

    fn finished(&self, ident: &Path) {
        finished(&self.shared, ident);
    }
}

fn finished(shared: &Shared, ident: &Path) {
    if let Ok(mut pending) = shared.pending.lock() {
        if let Some(n) = pending.get_mut(ident) {
            *n -= 1;
            if *n == 0 {
                pending.remove(ident);
            }
        }
    }
}

/// Make and keep the picture; the real path it is for.
fn render_job(dir: &Path, path: &Path, document: &Document) -> PathBuf {
    let ident = identity(path);
    // A failed or panicking render must not end the worker, nor fail the
    // open or save it follows.
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let picture = preview::render(document).ok()?;
        let png = encode(&picture)?;
        // The file's date and length as they are now, after the write.
        let key = FileKey::of_file(path)?;
        store(dir, &key, &png)
    }));
    ident
}

/// The picture as PNG, `None` if it came out too large to keep.
pub fn encode(picture: &Preview) -> Option<Vec<u8>> {
    let (pixels, _) = picture.rgba.as_chunks::<4>();
    let rgb: Vec<u8> = pixels.iter().flat_map(|p| [p[0], p[1], p[2]]).collect();
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, picture.width, picture.height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::High);
    let mut writer = encoder.write_header().ok()?;
    writer.write_image_data(&rgb).ok()?;
    writer.finish().ok()?;
    (out.len() <= MAX_PNG_BYTES).then_some(out)
}

/// A PNG as pixels; `None` for anything that is not a picture this renderer
/// could have made.
pub fn decode(bytes: &[u8]) -> Option<egui::ColorImage> {
    let mut reader = png::Decoder::new(Cursor::new(bytes)).read_info().ok()?;
    let info = reader.info();
    if (info.width, info.height) != (preview::WIDTH, preview::HEIGHT)
        || info.color_type != png::ColorType::Rgb
        || info.bit_depth != png::BitDepth::Eight
    {
        return None;
    }
    let mut pixels = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut pixels).ok()?;
    pixels.truncate(frame.buffer_size());
    (pixels.len() == (preview::WIDTH * preview::HEIGHT * 3) as usize).then(|| {
        egui::ColorImage::from_rgb([preview::WIDTH as usize, preview::HEIGHT as usize], &pixels)
    })
}

fn load(dir: &Path, key: &FileKey) -> Option<egui::ColorImage> {
    decode(&std::fs::read(dir.join(key.file_name())).ok()?)
}

/// Write the picture for `key` all at once (a temporary file renamed over),
/// then trim the folder.
fn store(dir: &Path, key: &FileKey, png: &[u8]) -> Option<()> {
    std::fs::create_dir_all(dir).ok()?;
    let name = key.file_name();
    let temporary = dir.join(format!("{name}.{}.tmp", std::process::id()));
    std::fs::write(&temporary, png).ok()?;
    if std::fs::rename(&temporary, dir.join(name)).is_err() {
        let _ = std::fs::remove_file(&temporary);
        return None;
    }
    trim(dir);
    Some(())
}

/// Remove the oldest pictures until the folder is within its budgets.
fn trim(dir: &Path) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(SystemTime, u64, PathBuf)> = read
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "png"))
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            Some((meta.modified().ok()?, meta.len(), e.path()))
        })
        .collect();
    files.sort_by_key(|f| std::cmp::Reverse(f.0));
    let (mut count, mut bytes) = (0, 0);
    for (_, len, path) in files {
        count += 1;
        bytes += len;
        if count > MAX_FILES || bytes > MAX_BYTES {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(test)]
#[path = "previews_tests.rs"]
mod tests;
