//! Paper output. A backend lists the printers of the machine, says which media
//! one can take, and sends it a plotted sheet. The sheet is the same
//! `PlotSheet` the PDF and the preview come from; a backend only decides how
//! to hand it over (CUPS takes a PDF, Windows a raster).
//!
//! Nothing here uses a shell: programs get their arguments as a list, and
//! names from the user or a driver are data (a JSON file, an argument), never
//! text inserted into a command or a script.

use std::ffi::OsString;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use od_core::plot::PlotSheet;
use od_core::plot_plan::MAX_COPIES;

#[cfg(any(test, not(windows)))]
pub mod cups;
#[cfg(test)]
pub mod fake;
#[cfg(any(test, windows))]
pub mod windows;

/// How long finding printers or their media may take.
pub const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(10);
/// How long handing a job to the print system may take.
pub const SPOOL_TIMEOUT: Duration = Duration::from_secs(60);
/// The most pixels a raster page may have.
#[cfg(any(test, windows))]
pub const MAX_RASTER_PIXELS: u64 = 100_000_000;
/// The finest raster printing resolution, in dots per inch.
pub const MAX_PRINT_DPI: u32 = 1200;
/// How much of a program's output is kept.
const MAX_OUTPUT: u64 = 64 * 1024;
/// Media sizes closer than this are the same paper, in millimetres.
const SIZE_TOLERANCE_MM: f64 = 0.5;

/// A printer as the print system names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrinterId(pub String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Printer {
    pub id: PrinterId,
    pub is_default: bool,
}

/// A paper the printer can take, as it names it.
#[derive(Clone, Debug, PartialEq)]
pub struct Medium {
    pub name: String,
    pub width_mm: f64,
    pub height_mm: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Capabilities {
    pub media: Vec<Medium>,
}

/// One sheet for one printer.
#[derive(Clone, Debug)]
pub struct PrintJob {
    pub printer: PrinterId,
    pub copies: u32,
    /// Raster resolution, for backends that print a picture.
    pub dpi: u32,
    pub sheet: PlotSheet,
}

/// What the print system said about an accepted job.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Receipt {
    pub queue_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrintError {
    Cancelled,
    /// There is no print system to ask; saving a PDF still works.
    BackendUnavailable(&'static str),
    Capability(String),
    SpoolRejected(String),
    /// The raster page would hold too many pixels. Only the Windows backend
    /// prints a picture, so elsewhere nothing makes it.
    #[cfg_attr(not(any(test, windows)), allow(dead_code))]
    RasterTooLarge,
    /// The job asks for something that cannot be printed.
    Invalid(&'static str),
    /// The printer has no paper of the sheet's size.
    MissingMedia,
    /// The print system did not answer in time; the job may still be queued.
    Timeout(&'static str),
    TemporaryFile(String),
    /// The sheet could not be made.
    Plot(String),
}

impl std::fmt::Display for PrintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled => f.write_str("printing was cancelled"),
            Self::BackendUnavailable(why) => f.write_str(why),
            Self::Capability(why) => write!(f, "the printer could not be asked: {why}"),
            Self::SpoolRejected(why) => write!(f, "the print system refused the job: {why}"),
            Self::RasterTooLarge => f.write_str(
                "the page is too large to print as a picture; lower the resolution or save a PDF",
            ),
            Self::Invalid(why) => f.write_str(why),
            Self::MissingMedia => f.write_str(
                "the printer has no paper of that size; choose another paper or save a PDF",
            ),
            Self::Timeout(what) => {
                write!(f, "{what} took too long; the job may still be in the queue")
            }
            Self::Plot(why) => write!(f, "the sheet could not be made: {why}"),
            Self::TemporaryFile(why) => write!(f, "a temporary file could not be made: {why}"),
        }
    }
}

pub trait PrintBackend: Send + Sync {
    fn printers(&self) -> Result<Vec<Printer>, PrintError>;
    fn capabilities(&self, printer: &PrinterId) -> Result<Capabilities, PrintError>;
    /// Hand `job` over. Cancelling is certain only until the print system
    /// has accepted the job.
    fn submit(&self, job: &PrintJob, cancel: &AtomicBool) -> Result<Receipt, PrintError>;
}

/// The paper of `capabilities` that `sheet` is cut for: the same size in
/// either orientation, within half a millimetre. A printer's default paper is
/// never substituted.
pub fn resolve_media<'a>(
    capabilities: &'a Capabilities,
    sheet: &PlotSheet,
) -> Result<&'a Medium, PrintError> {
    let close = |a: f64, b: f64| (a - b).abs() <= SIZE_TOLERANCE_MM;
    capabilities
        .media
        .iter()
        .find(|m| {
            (close(m.width_mm, sheet.width) && close(m.height_mm, sheet.height))
                || (close(m.width_mm, sheet.height) && close(m.height_mm, sheet.width))
        })
        .ok_or(PrintError::MissingMedia)
}

/// The checks every backend makes before it writes anything.
pub fn validate_job(job: &PrintJob) -> Result<(), PrintError> {
    if !(1..=MAX_COPIES).contains(&job.copies) {
        return Err(PrintError::Invalid("copies must be 1 to 999"));
    }
    if !(job.sheet.width.is_finite() && job.sheet.height.is_finite())
        || job.sheet.width <= 0.0
        || job.sheet.height <= 0.0
    {
        return Err(PrintError::Invalid("the sheet has no size"));
    }
    if !safe_name(&job.printer.0) {
        return Err(PrintError::Invalid("the printer name cannot be used"));
    }
    if !(1..=MAX_PRINT_DPI).contains(&job.dpi) {
        return Err(PrintError::Invalid("the resolution must be 1 to 1200 dpi"));
    }
    Ok(())
}

/// Whether a printer or paper name is something a program may be given: it
/// has text, no control characters and does not look like an option.
pub fn safe_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.chars().count() <= 255
        && !name.starts_with('-')
        && !name.chars().any(char::is_control)
}

/// Pixels for a page of `mm` at `dpi`, or `RasterTooLarge` when the page
/// would hold more than [`MAX_RASTER_PIXELS`] (computed without overflow).
#[cfg(any(test, windows))]
pub fn raster_size(width_mm: f64, height_mm: f64, dpi: u32) -> Result<(u32, u32), PrintError> {
    let pixels = |mm: f64| (mm / 25.4 * f64::from(dpi)).round().max(1.0);
    let (w, h) = (pixels(width_mm), pixels(height_mm));
    if !(w.is_finite() && h.is_finite()) || w * h > MAX_RASTER_PIXELS as f64 {
        return Err(PrintError::RasterTooLarge);
    }
    Ok((w as u32, h as u32))
}

/// A program to run and how long it may take.
pub struct Call {
    pub program: &'static str,
    pub args: Vec<OsString>,
    pub envs: Vec<(&'static str, OsString)>,
    pub timeout: Duration,
    /// Whether to kill it at the timeout or on cancel. A program that has
    /// already queued a job must be left to finish.
    pub kill: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Output {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Runs programs; the system one starts real processes, tests answer from a
/// script.
pub trait Runner: Send + Sync {
    fn run(&self, call: &Call, cancel: &AtomicBool) -> Result<Output, PrintError>;
}

pub struct SystemRunner;

impl Runner for SystemRunner {
    fn run(&self, call: &Call, cancel: &AtomicBool) -> Result<Output, PrintError> {
        if cancel.load(Ordering::Relaxed) {
            return Err(PrintError::Cancelled);
        }
        let mut command = Command::new(call.program);
        command
            .args(&call.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in &call.envs {
            command.env(key, value);
        }
        let mut child = command.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                PrintError::BackendUnavailable(
                    "the print system is not installed; save a PDF instead",
                )
            } else {
                PrintError::Capability(e.kind().to_string())
            }
        })?;
        let read = |pipe: Option<Box<dyn Read + Send>>| {
            std::thread::spawn(move || {
                let mut text = Vec::new();
                if let Some(pipe) = pipe {
                    let _ = pipe.take(MAX_OUTPUT).read_to_end(&mut text);
                }
                String::from_utf8_lossy(&text).into_owned()
            })
        };
        let out = read(
            child
                .stdout
                .take()
                .map(|p| Box::new(p) as Box<dyn Read + Send>),
        );
        let err = read(
            child
                .stderr
                .take()
                .map(|p| Box::new(p) as Box<dyn Read + Send>),
        );
        let started = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {}
                Err(e) => return Err(PrintError::Capability(e.kind().to_string())),
            }
            let timed_out = started.elapsed() >= call.timeout;
            let cancelled = cancel.load(Ordering::Relaxed);
            if (timed_out || cancelled) && call.kill {
                let _ = child.kill();
                let _ = child.wait();
                return Err(if cancelled {
                    PrintError::Cancelled
                } else {
                    PrintError::Timeout("the print system")
                });
            }
            if timed_out {
                // Left running: it may already have queued the job.
                return Err(PrintError::Timeout("the print system"));
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        Ok(Output {
            success: status.success(),
            stdout: out.join().unwrap_or_default(),
            stderr: err.join().unwrap_or_default(),
        })
    }
}

/// A private folder for the files of one job, removed when dropped.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new() -> Result<Self, PrintError> {
        let mut bytes = [0u8; 8];
        getrandom::fill(&mut bytes).map_err(|e| PrintError::TemporaryFile(e.to_string()))?;
        let name: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let path = std::env::temp_dir().join(format!("opendraft-print-{name}"));
        #[cfg_attr(not(unix), allow(unused_mut))]
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder
            .create(&path)
            .map_err(|e| PrintError::TemporaryFile(e.kind().to_string()))?;
        Ok(Self { path })
    }

    #[cfg(test)]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Writes `bytes` to a new file called `name` in the folder.
    pub fn write(&self, name: &str, bytes: &[u8]) -> Result<PathBuf, PrintError> {
        use std::io::Write;
        let path = self.path.join(name);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| PrintError::TemporaryFile(e.kind().to_string()))?;
        file.write_all(bytes)
            .map_err(|e| PrintError::TemporaryFile(e.kind().to_string()))?;
        Ok(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// The backend of this machine.
pub fn system_backend() -> Box<dyn PrintBackend> {
    #[cfg(windows)]
    {
        Box::new(windows::WindowsBackend::new(SystemRunner))
    }
    #[cfg(not(windows))]
    {
        Box::new(cups::CupsBackend::new(SystemRunner))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet(w: f64, h: f64) -> PlotSheet {
        PlotSheet {
            width: w,
            height: h,
            paths: Vec::new(),
            fills: Vec::new(),
            notes: Vec::new(),
        }
    }

    fn job(copies: u32, dpi: u32) -> PrintJob {
        PrintJob {
            printer: PrinterId("Office".into()),
            copies,
            dpi,
            sheet: sheet(297.0, 210.0),
        }
    }

    #[test]
    fn media_is_found_by_size_in_either_orientation_and_never_substituted() {
        let caps = Capabilities {
            media: vec![
                Medium {
                    name: "Letter".into(),
                    width_mm: 215.9,
                    height_mm: 279.4,
                },
                Medium {
                    name: "A4".into(),
                    width_mm: 210.0,
                    height_mm: 297.0,
                },
            ],
        };
        assert_eq!(
            resolve_media(&caps, &sheet(297.0, 210.0)).unwrap().name,
            "A4"
        );
        assert_eq!(
            resolve_media(&caps, &sheet(210.2, 296.8)).unwrap().name,
            "A4"
        );
        assert_eq!(
            resolve_media(&caps, &sheet(420.0, 297.0)),
            Err(PrintError::MissingMedia)
        );
    }

    #[test]
    fn a_job_is_checked_before_anything_is_written() {
        assert_eq!(validate_job(&job(1, 300)), Ok(()));
        for bad in [job(0, 300), job(1000, 300), job(1, 0), job(1, 2400)] {
            assert!(matches!(validate_job(&bad), Err(PrintError::Invalid(_))));
        }
        let mut hostile = job(1, 300);
        hostile.printer = PrinterId("-o evil".into());
        assert!(validate_job(&hostile).is_err());
        hostile.printer = PrinterId("a\nb".into());
        assert!(validate_job(&hostile).is_err());
        let mut flat = job(1, 300);
        flat.sheet = sheet(f64::NAN, 10.0);
        assert!(validate_job(&flat).is_err());
    }

    #[test]
    fn a_raster_over_the_pixel_cap_is_refused_before_it_is_made() {
        assert_eq!(raster_size(297.0, 210.0, 300), Ok((3508, 2480)));
        assert_eq!(
            raster_size(9000.0, 9000.0, 1200),
            Err(PrintError::RasterTooLarge)
        );
        assert_eq!(
            raster_size(f64::MAX, 1.0, 1200),
            Err(PrintError::RasterTooLarge)
        );
    }

    #[test]
    fn a_temp_folder_is_private_to_the_job_and_removed() {
        let path = {
            let dir = TempDir::new().unwrap();
            let file = dir.write("a.pdf", b"x").unwrap();
            assert!(file.exists());
            assert!(dir.write("a.pdf", b"y").is_err(), "never overwrites");
            dir.path().to_owned()
        };
        assert!(!path.exists());
    }

    #[test]
    fn the_system_runner_reports_a_missing_program_and_a_cancel() {
        let call = Call {
            program: "opendraft-no-such-program",
            args: Vec::new(),
            envs: Vec::new(),
            timeout: Duration::from_secs(1),
            kill: true,
        };
        let cancel = AtomicBool::new(false);
        assert!(matches!(
            SystemRunner.run(&call, &cancel),
            Err(PrintError::BackendUnavailable(_))
        ));
        cancel.store(true, Ordering::Relaxed);
        assert_eq!(SystemRunner.run(&call, &cancel), Err(PrintError::Cancelled));
    }
}
