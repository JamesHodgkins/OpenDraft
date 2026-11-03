//! The CUPS backend against a real CUPS, with the same runner the Plot dialog
//! uses. Ignored unless `.github/workflows/cups.yml` has made a cups-pdf queue
//! (it names the queue and where cups-pdf writes in the environment); the
//! page size and scale of what comes out are checked after, by
//! `scripts/check_cups_pdf.py`.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use od_core::plot::{PlotPath, PlotSheet};
use od_core::Point;

use super::cups::CupsBackend;
use super::{PrintBackend, PrintJob, PrinterId, SystemRunner};

fn variable(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is set by the cups workflow"))
}

fn pdfs_in(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "pdf"))
        .collect()
}

#[test]
#[ignore = "needs a CUPS queue; run by .github/workflows/cups.yml"]
fn a_job_goes_through_real_cups_and_comes_out_as_a_pdf() {
    let queue = variable("OD_CUPS_PRINTER");
    let out = PathBuf::from(variable("OD_CUPS_OUT"));
    let backend = CupsBackend::new(SystemRunner);

    let printers = backend.printers().unwrap();
    assert!(printers.iter().any(|p| p.id.0 == queue), "{printers:?}");
    let printer = PrinterId(queue);
    let media = backend.capabilities(&printer).unwrap().media;
    let a4 = media
        .iter()
        .find(|m| m.name == "A4")
        .unwrap_or_else(|| panic!("the queue offers no A4: {media:?}"));

    // A 100 mm line, so the checks after can see the scale come out right.
    let sheet = PlotSheet {
        width: a4.width_mm,
        height: a4.height_mm,
        paths: vec![PlotPath {
            points: vec![Point::new(50.0, 100.0), Point::new(150.0, 100.0)],
            closed: false,
            rgb: [0, 0, 0],
            width: 0.5,
        }],
        fills: Vec::new(),
        notes: Vec::new(),
    };
    let job = PrintJob {
        printer,
        copies: 1,
        dpi: 300,
        sheet,
    };
    let receipt = backend.submit(&job, &AtomicBool::new(false)).unwrap();
    assert!(receipt.queue_id.is_some(), "lp named no job");

    let deadline = Instant::now() + Duration::from_secs(60);
    let file = loop {
        if let Some(file) = pdfs_in(&out).into_iter().next() {
            break file;
        }
        assert!(
            Instant::now() < deadline,
            "no PDF in {out:?} a minute after the job was accepted"
        );
        std::thread::sleep(Duration::from_millis(500));
    };
    // cups-pdf may still be writing when the name appears.
    std::thread::sleep(Duration::from_secs(2));
    let bytes = std::fs::read(&file).unwrap();
    assert!(bytes.starts_with(b"%PDF-"), "{file:?} is not a PDF");
    eprintln!("CUPS wrote {} ({} bytes)", file.display(), bytes.len());
}
