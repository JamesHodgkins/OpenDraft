use std::path::PathBuf;

use super::*;

/// Home with ten recent drawings that are not on any disk (as when a drive
/// is unmounted), two of them pinned.
fn crowded_home() -> OpenDraftApp {
    let mut app = app();
    app.state.start_on_home();
    for i in 0..10 {
        let id = app.state.recent.add(&PathBuf::from(format!(
            "Z:/unmounted/a-rather-long-folder-name/plan {i} with a long file name.dwg"
        )));
        if i % 5 == 0 {
            app.state.recent.pin(id).unwrap();
        }
    }
    app
}

/// How wide the Home page came out, and how tall.
fn run_at(app: &mut OpenDraftApp, size: Vec2, scale: f32) -> (f32, f32) {
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(scale);
    for (i, t) in [0.0, 0.1, 0.2].into_iter().enumerate() {
        sized_frame(app, &ctx, Vec::new(), t + i as f64 * 0.01, size);
    }
    let width = ctx
        .data(|d| d.get_temp::<f32>(egui::Id::new("home_content_width")))
        .unwrap_or(f32::MAX);
    (width, ctx.used_rect().height())
}

#[test]
fn home_never_spills_past_the_window_at_any_size_or_scale() {
    for (width, height) in [
        (320.0, 480.0),
        (540.0, 600.0),
        (800.0, 600.0),
        (899.0, 700.0),
        (900.0, 700.0),
        (1280.0, 800.0),
        (3840.0, 1200.0),
    ] {
        for scale in [1.0, 1.5, 2.0] {
            let mut app = crowded_home();
            let (used, _) = run_at(&mut app, Vec2::new(width, height), scale);
            assert!(
                used <= width + 0.5,
                "{width}x{height} at {scale}x: the page is {used} wide"
            );
        }
    }
}

#[test]
fn recovery_offers_and_missing_files_lay_out_without_trouble() {
    let mut app = crowded_home();
    // Missing files show as such, and the page still draws.
    let (_, height) = run_at(&mut app, Vec2::new(800.0, 600.0), 1.0);
    assert!(height > 100.0);
    app.state
        .recovery_report
        .failed
        .push(("plan".to_owned(), "a very long reason ".repeat(20)));
    let (used, _) = run_at(&mut app, Vec2::new(400.0, 600.0), 1.0);
    assert!(used <= 400.5, "{used}");
}
