#![allow(clippy::print_stdout, clippy::print_stderr)] // a command-line tool
//! CPU benchmark for the canvas: frame cost with many entities.
//!
//! Run in release mode:
//!   cargo run --release -p od-canvas --example bench
//!
//! Measures what the app spends per frame on the CPU (input handling,
//! hit-testing, snapping, painting into egui shapes, and tessellation into
//! triangles). GPU draw time is not included. Budget: 16.6 ms for 60 fps.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use egui::{Color32, Event, Pos2, RawInput, Rect, Vec2};
use od_canvas::{CanvasStyle, GridSettings, PointPrompt, SnapModes, View};
use od_core::{Document, Ellipse, Entity, EntityId, Point, Spline};

const FRAMES: usize = 30;
const SCREEN: Vec2 = Vec2::new(1600.0, 900.0);

/// Tiny deterministic generator so runs are comparable.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as f64 / (1u64 << 31) as f64
    }
}

/// `n` mixed entities scattered over a 10000 x 6000 sheet.
fn generate(n: usize) -> Vec<Entity> {
    let mut rng = Lcg(42);
    (0..n)
        .map(|i| {
            let (x, y) = (rng.next() * 10_000.0, rng.next() * 6_000.0);
            let size = 20.0 + rng.next() * 180.0;
            match i % 3 {
                0 => Entity::Line {
                    start: Point::new(x, y),
                    end: Point::new(x + size, y + rng.next() * size - size / 2.0),
                },
                1 => Entity::Circle {
                    center: Point::new(x, y),
                    radius: size / 2.0,
                },
                _ => Entity::Rectangle {
                    min: Point::new(x, y),
                    max: Point::new(x + size, y + size * 0.6),
                },
            }
        })
        .collect()
}

/// `n` ellipses, a third of them arcs, at assorted sizes, turns and ratios.
fn generate_ellipses(n: usize) -> Vec<Entity> {
    let mut rng = Lcg(7);
    (0..n)
        .filter_map(|i| {
            let center = Point::new(rng.next() * 10_000.0, rng.next() * 6_000.0);
            let (size, turn) = (10.0 + rng.next() * 90.0, rng.next() * std::f64::consts::TAU);
            let axis = Point::new(size * turn.cos(), size * turn.sin());
            let ratio = 0.05 + rng.next() * 0.95;
            let (start, sweep) = if i % 3 == 0 {
                (rng.next() * 6.0, 0.5 + rng.next() * 5.0)
            } else {
                (0.0, std::f64::consts::TAU)
            };
            Ellipse::new(center, axis, ratio, start, start + sweep).ok()
        })
        .map(Entity::Ellipse)
        .collect()
}

/// `n` cubic splines of `cvs` control points each, every one a random walk
/// about a spot on the sheet.
fn generate_splines(n: usize, cvs: usize) -> Vec<Entity> {
    let mut rng = Lcg(11);
    (0..n)
        .filter_map(|_| {
            let mut at = Point::new(rng.next() * 10_000.0, rng.next() * 6_000.0);
            let step = 200.0 / cvs as f64;
            let points = (0..cvs)
                .map(|_| {
                    at = Point::new(
                        at.x + step * rng.next(),
                        at.y + step * (rng.next() - 0.5) * 4.0,
                    );
                    at
                })
                .collect();
            Spline::from_control_points(3, points).ok()
        })
        .map(|s| Entity::Spline(Box::new(s)))
        .collect()
}

fn style() -> CanvasStyle {
    CanvasStyle {
        background: Color32::from_rgb(0x25, 0x25, 0x25),
        grid_minor: Color32::from_white_alpha(18),
        grid_major: Color32::from_white_alpha(36),
        axis_x: Color32::GRAY,
        axis_y: Color32::GRAY,
        entity: Color32::WHITE,
        hover: Color32::from_rgba_unmultiplied(20, 115, 230, 60),
        selected: Color32::LIGHT_BLUE,
        selected_glow: Color32::from_rgba_unmultiplied(20, 115, 230, 120),
        snap: Color32::YELLOW,
        window: Color32::LIGHT_BLUE,
        crossing: Color32::LIGHT_GREEN,
    }
}

#[derive(Default)]
struct Timings {
    run: Vec<Duration>,
    tessellate: Vec<Duration>,
    triangles: usize,
}

fn percentile(samples: &[Duration], p: f64) -> f64 {
    let mut ms: Vec<f64> = samples.iter().map(|d| d.as_secs_f64() * 1e3).collect();
    ms.sort_by(f64::total_cmp);
    ms[((ms.len() - 1) as f64 * p).round() as usize]
}

struct Scenario<'a> {
    name: &'a str,
    document: &'a Document,
    selection: BTreeSet<EntityId>,
    /// A command is waiting for a point (snapping is on).
    point_prompt: bool,
    /// `None` frames the whole drawing; `Some(zoom)` zooms in on the middle.
    zoom: Option<f64>,
}

fn run(scenario: &Scenario) -> Timings {
    let ctx = egui::Context::default();
    let mut view = View::default();
    if scenario.point_prompt {
        view.set_point_prompt(Some(PointPrompt::default()));
    }
    match scenario.zoom {
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
    let mut timings = Timings::default();
    // Frame 0 applies the zoom request; the rest are measured. The pointer
    // moves a little each frame so hover/snap work is not cached away.
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
                    view.show(
                        ui,
                        &style,
                        &grid,
                        scenario.document,
                        &scenario.selection,
                        &snap,
                    );
                });
        });
        let run_time = start.elapsed();
        let start = Instant::now();
        let meshes = ctx.tessellate(output.shapes, output.pixels_per_point);
        let tess_time = start.elapsed();
        if frame > 0 {
            timings.run.push(run_time);
            timings.tessellate.push(tess_time);
            timings.triangles = meshes
                .iter()
                .map(|m| match &m.primitive {
                    egui::epaint::Primitive::Mesh(mesh) => mesh.indices.len() / 3,
                    _ => 0,
                })
                .sum();
        }
    }
    timings
}

fn main() {
    if cfg!(debug_assertions) {
        eprintln!("warning: debug build; numbers are not meaningful. Use --release.");
    }
    println!(
        "{:<34} {:>9} {:>9} {:>9} {:>9} {:>10}",
        "scenario", "run avg", "run p95", "tess avg", "total p95", "triangles"
    );
    let mixed = [1_000, 10_000, 50_000, 100_000].map(|n| (n, "", generate(n)));
    let ellipses = [1_000, 10_000, 50_000].map(|n| (n, "ellipses ", generate_ellipses(n)));
    let splines = [
        (10_000, "8-CV splines ", generate_splines(10_000, 8)),
        (1_000, "1000-CV splines ", generate_splines(1_000, 1_000)),
    ];
    for (n, kind, entities) in mixed.into_iter().chain(ellipses).chain(splines) {
        let doc = Document::from_entities(entities);
        let selected: BTreeSet<EntityId> = doc.objects().iter().step_by(10).map(|o| o.id).collect();
        let scenarios = [
            Scenario {
                name: "extents (all visible)",
                document: &doc,
                selection: BTreeSet::new(),
                point_prompt: false,
                zoom: None,
            },
            Scenario {
                name: "extents, 10% selected",
                document: &doc,
                selection: selected,
                point_prompt: false,
                zoom: None,
            },
            Scenario {
                name: "extents, point prompt (snapping)",
                document: &doc,
                selection: BTreeSet::new(),
                point_prompt: true,
                zoom: None,
            },
            Scenario {
                name: "zoomed far out (tiny)",
                document: &doc,
                selection: BTreeSet::new(),
                point_prompt: false,
                zoom: Some(0.04),
            },
            Scenario {
                name: "zoomed in (few visible)",
                document: &doc,
                selection: BTreeSet::new(),
                point_prompt: false,
                zoom: Some(4.0),
            },
        ];
        for s in &scenarios {
            let t = run(s);
            let avg = |v: &[Duration]| {
                v.iter().map(|d| d.as_secs_f64() * 1e3).sum::<f64>() / v.len() as f64
            };
            let total: Vec<Duration> = t
                .run
                .iter()
                .zip(&t.tessellate)
                .map(|(a, b)| *a + *b)
                .collect();
            println!(
                "{:<34} {:>7.2}ms {:>7.2}ms {:>7.2}ms {:>7.2}ms {:>10}",
                format!("{n:>6} {kind}{}", s.name),
                avg(&t.run),
                percentile(&t.run, 0.95),
                avg(&t.tessellate),
                percentile(&total, 0.95),
                t.triangles
            );
        }
        println!();
    }
}
