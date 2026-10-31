#![allow(clippy::print_stdout, clippy::print_stderr, clippy::unwrap_used)] // a command-line tool
//! CPU benchmark for what was built after the canvas benchmark: dashed
//! linetypes, dimensions, block inserts, hatches, editing and saving, and
//! real files.
//!
//! Run in release mode:
//!   cargo run --release -p od-app --example bench_features
//!
//! Like `od-canvas`'s `bench`, it measures the CPU cost of a frame (input,
//! painting into egui shapes, tessellation) and not the GPU. Files that are
//! not there are skipped.

mod scenes;

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, Event, Pos2, RawInput, Rect, Vec2};
use od_canvas::{GridSettings, SnapModes, View};
use od_core::{Document, Point};
use od_files::{DwgFormat, DxfFormat, FileFormat};
use scenes::{dashed, dimensions, generate, hatches, inserts, read_file, style};

const FRAMES: usize = 20;
const SCREEN: Vec2 = Vec2::new(1600.0, 900.0);

struct Case<'a> {
    name: String,
    document: &'a Document,
    selection: BTreeSet<od_core::EntityId>,
    zoom: Option<f64>,
}

fn run(case: &Case) -> (Vec<Duration>, Vec<Duration>, usize) {
    let ctx = egui::Context::default();
    let mut view = View::default();
    match case.zoom {
        None => view.request_zoom_extents(),
        Some(zoom) => {
            view.camera.zoom = zoom;
            view.camera.center = Point::new(5_000.0, 3_000.0);
        }
    }
    let (style, grid, snap) = (
        style(),
        GridSettings {
            visible: true,
            spacing: 10.0,
        },
        SnapModes::default(),
    );
    let (mut run_times, mut tess_times, mut triangles) = (Vec::new(), Vec::new(), 0);
    for frame in 0..=FRAMES {
        let pointer = Pos2::new(800.0 + frame as f32 * 3.0, 450.0 + frame as f32 * 2.0);
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
            events: vec![Event::PointerMoved(pointer)],
            ..Default::default()
        };
        let start = Instant::now();
        let output = ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::new())
                .show(ctx, |ui| {
                    view.show(ui, &style, &grid, case.document, &case.selection, &snap);
                });
        });
        let run_time = start.elapsed();
        let start = Instant::now();
        let meshes = ctx.tessellate(output.shapes, output.pixels_per_point);
        let tess = start.elapsed();
        if frame > 0 {
            run_times.push(run_time);
            tess_times.push(tess);
            triangles = meshes
                .iter()
                .map(|m| match &m.primitive {
                    egui::epaint::Primitive::Mesh(mesh) => mesh.indices.len() / 3,
                    _ => 0,
                })
                .sum();
        }
    }
    (run_times, tess_times, triangles)
}

fn ms(samples: &[Duration]) -> f64 {
    samples.iter().map(|d| d.as_secs_f64() * 1e3).sum::<f64>() / samples.len() as f64
}

fn report(case: &Case) {
    let (run_times, tess_times, triangles) = run(case);
    println!(
        "{:<52} {:>8.2}ms {:>8.2}ms {:>9.2}ms {:>10}",
        case.name,
        ms(&run_times),
        ms(&tess_times),
        ms(&run_times) + ms(&tess_times),
        triangles
    );
}

/// The standard views of one drawing: all of it, and zoomed in.
fn views(label: &str, document: &Document, zooms: &[(&str, Option<f64>)]) {
    for (what, zoom) in zooms {
        report(&Case {
            name: format!("{label}, {what}"),
            document,
            selection: BTreeSet::new(),
            zoom: *zoom,
        });
    }
}

/// What one call costs, per object, for the things done to every object on
/// every frame or mouse move: its bounds (culling) and a hit-test.
fn per_call(label: &str, document: &Document) {
    let n = document.objects().len() as f64;
    // The same objects with nothing remembered yet (copying keeps what an
    // object remembers).
    let document = &Document::from_entities(document.objects().iter().map(|o| o.entity.clone()));
    // The box of every object: the first look builds it (for a dimension,
    // everything it draws), later looks read what the object remembers.
    let mut area = 0.0;
    let mut look = || {
        let start = Instant::now();
        for object in document.objects() {
            let b = object.bounds();
            area += (b.max.x - b.min.x) * (b.max.y - b.min.y);
        }
        start.elapsed().as_secs_f64() * 1e6 / n
    };
    let first = look();
    let later = look();
    let start = Instant::now();
    let mut hits = 0;
    for i in 0..20 {
        let p = Point::new(100.0 + f64::from(i) * 37.0, 50.0 + f64::from(i) * 11.0);
        hits += usize::from(od_canvas::pick(document.objects(), p, 6.0).is_some());
    }
    let pick = start.elapsed().as_secs_f64() * 1e3 / 20.0;
    println!(
        "{label:<26} box first {first:>5.2} us, later {later:>5.2} us/object   one hit-test over all {pick:>6.2} ms   ({area:.0}, {hits})"
    );
}

/// What editing every object costs: moving all, erasing all, and undoing
/// each, which is what Move or Erase on a select-all does.
fn edits(n: usize) {
    let start = Instant::now();
    let doc = Document::from_entities(generate(n));
    let build = start.elapsed();
    let ids: Vec<_> = doc.objects().iter().map(|o| o.id).collect();
    let mut session = od_commands::Session::with_document(doc);
    let time = |f: &mut dyn FnMut()| {
        let start = Instant::now();
        f();
        start.elapsed().as_secs_f64() * 1e3
    };
    let changes: Vec<_> = session
        .document()
        .objects()
        .iter()
        .map(|o| (o.id, o.entity.translated(5.0, 5.0)))
        .collect();
    let move_all = time(&mut || {
        session.perform(od_commands::ModifyEntities::new("Move", changes.clone()));
    });
    let undo_move = time(&mut || {
        session.undo();
    });
    let redo_move = time(&mut || {
        session.redo();
    });
    let erase = time(&mut || {
        session.perform(od_commands::RemoveEntities::new(ids.clone()));
    });
    let undo_erase = time(&mut || {
        session.undo();
    });
    let extents = time(&mut || {
        let _ = session.document().bounds();
    });
    println!(
        "{n:>7} objects: build {:>6.1} ms | move all {move_all:>6.1}, undo {undo_move:>6.1}, redo {redo_move:>6.1} | erase all {erase:>6.1}, undo {undo_erase:>6.1} | extents {extents:>5.1}   (ms)",
        build.as_secs_f64() * 1e3
    );
}

/// What saving costs: writing the drawing as DXF, which turns each block
/// reference and dimension into the objects it draws first.
fn save(label: &str, document: &Document) {
    let start = Instant::now();
    let written = DxfFormat.write(document);
    let ms = start.elapsed().as_secs_f64() * 1e3;
    match written {
        Ok(out) => println!(
            "{label:<40} DXF write {ms:>9.1} ms ({} KB)",
            out.bytes.len() / 1024
        ),
        Err(e) => println!("{label:<40} DXF write failed: {e}"),
    }
}

fn main() {
    if cfg!(debug_assertions) {
        eprintln!("warning: debug build; numbers are not meaningful. Use --release.");
    }
    println!(
        "{:<52} {:>10} {:>10} {:>11} {:>10}",
        "scenario", "run", "tessellate", "total", "triangles"
    );
    let zooms = [("extents", None), ("zoom 1.0", Some(1.0))];

    println!("\n-- dashed linetypes (10,000 objects: lines, circles, arcs, rectangles, polylines)");
    let solid = Document::from_entities(generate(10_000));
    views("solid", &solid, &zooms);
    // At the extents zoom (about 0.14) one repeat of DASHED, 19.05 units, is
    // about 2.7 px, so a scale of `px / 2.7` gives a repeat of about `px`.
    // Patterns finer than `MIN_REPEAT_PX` in `dash_paint.rs` are drawn solid.
    for px in [2, 4, 5, 6, 7, 8, 12, 24] {
        let scale = f64::from(px) / 2.7;
        views(
            &format!("DASHED, repeat ~{px} px"),
            &dashed(&solid, "DASHED", scale),
            &zooms[..1],
        );
    }

    println!("\n-- dimensions");
    let dims = dimensions(2_000);
    views("2,000 aligned dimensions", &dims, &zooms);

    println!("\n-- block inserts");
    let blocks = inserts(2_000);
    views("2,000 inserts of 12 objects", &blocks, &zooms);

    println!("\n-- per-call costs (what a mouse move pays)");
    per_call("10,000 plain objects", &solid);
    per_call("2,000 dimensions", &dims);
    per_call("2,000 inserts of 12", &blocks);

    println!("\n-- hatches (HatchTest.dxf, copied over 1,500 x 900, all in view at zoom 1.0)");
    match hatches(500) {
        Some(doc) => {
            // Frame 0 builds each hatch's strokes and mesh; the rest reuse them.
            views("500 hatches", &doc, &zooms[1..]);
            per_call("500 hatches", &doc);
        }
        None => println!("(private_material/HatchTest.dxf not found)"),
    }

    println!("\n-- editing every object (Move all, Erase all, and their undo)");
    for n in [10_000, 50_000, 100_000] {
        edits(n);
    }

    println!("\n-- one object erased and put back (Document::remove and insert)");
    for n in [10_000, 100_000] {
        let doc = Document::from_entities(generate(n));
        let ids: Vec<_> = doc.objects().iter().map(|o| o.id).collect();
        for (label, at) in [("first", 0), ("middle", n / 2), ("last", n - 1)] {
            let mut doc = doc.clone();
            let start = Instant::now();
            for _ in 0..100 {
                let (position, object) = doc.remove(ids[at]).unwrap();
                doc.insert(position, object);
            }
            let ms = start.elapsed().as_secs_f64() * 1e3 / 100.0;
            println!("{n:>7} objects, {label:<6} {ms:>8.3} ms");
        }
    }

    println!("\n-- saving as DXF");
    save("10,000 plain objects", &solid);
    save("2,000 dimensions", &dims);
    save("2,000 inserts of 12", &blocks);
    save("8,000 inserts of 12", &inserts(8_000));

    println!("\n-- real files");
    for file in ["Linetypes.dxf", "HatchTest.dxf", "Test Drawing 2.dxf"] {
        if let Some(doc) = read_file(file) {
            views(file, &doc, &zooms[..1]);
        }
    }
    let desktop: PathBuf = [
        std::env::var("USERPROFILE").unwrap_or_default().as_str(),
        "Desktop",
        "Pipe Fittings.dwg",
    ]
    .iter()
    .collect();
    if let Ok(bytes) = std::fs::read(&desktop) {
        let start = Instant::now();
        let loaded = DwgFormat.read(&bytes).unwrap();
        println!(
            "Pipe Fittings.dwg: read {} objects in {:.0} ms",
            loaded.document.objects().len(),
            start.elapsed().as_secs_f64() * 1e3
        );
        validate_cost("Pipe Fittings.dwg", &loaded.document);
        views("Pipe Fittings.dwg", &loaded.document, &zooms);
        save("Pipe Fittings.dwg", &loaded.document);
    } else {
        println!("(Pipe Fittings.dwg not found on the Desktop)");
    }
}

/// What a full `Document::validate()` costs: automation runs it before
/// recording every write (specification 11.5).
fn validate_cost(label: &str, document: &od_core::Document) {
    let runs = 5;
    let start = Instant::now();
    for _ in 0..runs {
        assert_eq!(document.validate(), Ok(()));
    }
    println!(
        "{label}: validate {} objects in {:.2} ms",
        document.len(),
        start.elapsed().as_secs_f64() * 1e3 / f64::from(runs)
    );
}
