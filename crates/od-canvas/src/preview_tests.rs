use od_core::plot::{PlotFill, PlotPath};
use od_core::{Attr, Attributes, Entity, LayerId};

use super::*;

fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn bounds(x0: f64, y0: f64, x1: f64, y1: f64) -> Bounds {
    Bounds::from_points(pt(x0, y0), pt(x1, y1))
}

/// FNV-1a over the pixels, to say whether a picture is the one expected.
fn hash(preview: &Preview) -> u64 {
    preview.rgba.iter().fold(0xcbf29ce484222325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}

fn dark_pixels(preview: &Preview) -> usize {
    preview
        .rgba
        .chunks(4)
        .filter(|p| p[0] < 128 && p[1] < 128 && p[2] < 128)
        .count()
}

fn pixel(preview: &Preview, x: u32, y: u32) -> [u8; 3] {
    let at = ((y * preview.width + x) * 4) as usize;
    [preview.rgba[at], preview.rgba[at + 1], preview.rgba[at + 2]]
}

#[test]
fn the_window_centres_the_drawing_with_six_percent_either_side() {
    let w = window(bounds(0.0, 0.0, 100.0, 50.0)).unwrap();
    // 100 units must span 88% of 512 px: 454.56 px/unit-less, so the window
    // is 512 / (0.88 * 512 / 100) = 113.636... units wide.
    let width = w.max.x - w.min.x;
    assert!((width - 100.0 / 0.88).abs() < 1e-9, "{width}");
    assert!(((w.max.y - w.min.y) - width * 320.0 / 512.0).abs() < 1e-9);
    assert!(((w.min.x + w.max.x) / 2.0 - 50.0).abs() < 1e-9);
    assert!(((w.min.y + w.max.y) / 2.0 - 25.0).abs() < 1e-9);
}

#[test]
fn a_tall_drawing_is_fitted_by_its_height() {
    let w = window(bounds(0.0, 0.0, 10.0, 100.0)).unwrap();
    let height = w.max.y - w.min.y;
    assert!((height - 100.0 / 0.88).abs() < 1e-9, "{height}");
}

#[test]
fn a_point_a_flat_drawing_and_bad_extents_are_all_handled() {
    assert!(window(bounds(5.0, 5.0, 5.0, 5.0)).is_some());
    let flat = window(bounds(0.0, 3.0, 40.0, 3.0)).unwrap();
    assert!(((flat.max.x - flat.min.x) - 40.0 / 0.88).abs() < 1e-9);
    assert!(window(bounds(0.0, 0.0, f64::INFINITY, 1.0)).is_none());
    let not_a_number = Bounds {
        min: pt(0.0, 0.0),
        max: pt(f64::NAN, 1.0),
    };
    assert!(window(not_a_number).is_none());
}

#[test]
fn an_empty_drawing_is_the_blank_grid() {
    let preview = render(&Document::default()).unwrap();
    assert_eq!((preview.width, preview.height), (WIDTH, HEIGHT));
    assert_eq!(preview, blank());
    assert_eq!(pixel(&preview, 1, 1), [255, 255, 255]);
    assert_eq!(pixel(&preview, 32, 1), [230, 230, 230]);
    assert_eq!(pixel(&preview, 128, 1), [208, 208, 208]);
    assert_eq!(dark_pixels(&preview), 0);
}

fn line_and_circle() -> Document {
    let mut doc = Document::default();
    doc.add(Entity::Line {
        start: pt(0.0, 0.0),
        end: pt(100.0, 50.0),
    });
    doc.add(Entity::Circle {
        center: pt(50.0, 25.0),
        radius: 20.0,
    });
    doc
}

// Fixture: a line from (0,0) to (100,50) and a circle of radius 20 at
// (50,25), black on paper, 512 x 320. Expected hash from the first accepted
// run; a change here is an intentional change of the renderer's output.
const LINE_AND_CIRCLE: u64 = 9472657424578411776;

#[test]
fn a_line_and_a_circle_are_drawn_where_they_belong() {
    let preview = render(&line_and_circle()).unwrap();
    assert_eq!((preview.width, preview.height), (512, 320));
    // The centre of the circle's line crossing is at the picture's centre.
    assert!(
        pixel(&preview, 256, 160)[0] < 200,
        "the line passes the centre"
    );
    // y points up: the line rises to the right, so the left end is lower.
    let left = (0..320).find(|y| pixel(&preview, 40, *y)[0] < 128);
    let right = (0..320).find(|y| pixel(&preview, 470, *y)[0] < 128);
    assert!(left.unwrap() > right.unwrap(), "{left:?} {right:?}");
    assert_eq!(hash(&preview), LINE_AND_CIRCLE);
}

fn dashed_line() -> Document {
    let mut doc = Document::default();
    doc.add_with(
        Entity::Line {
            start: pt(0.0, 0.0),
            end: pt(200.0, 0.0),
        },
        Attributes {
            linetype: Attr::Set("DASHED".into()),
            ..Attributes::on_layer(LayerId::DEFAULT)
        },
    );
    doc.add(Entity::Line {
        start: pt(0.0, 10.0),
        end: pt(200.0, 10.0),
    });
    doc
}

// Fixture: two horizontal lines 200 long and 10 apart, the lower one DASHED.
const DASHED: u64 = 18278527669051949093;

#[test]
fn a_dashed_line_has_gaps_where_a_continuous_one_has_none() {
    let preview = render(&dashed_line()).unwrap();
    let row_dark = |y: u32| (0..512).filter(|x| pixel(&preview, *x, y)[0] < 160).count();
    let rows: Vec<u32> = (0..320).filter(|y| row_dark(*y) > 20).collect();
    assert_eq!(rows.len(), 2, "{rows:?}");
    let (upper, lower) = (row_dark(rows[0]), row_dark(rows[1]));
    assert!(upper > 400, "continuous: {upper}");
    assert!(lower < upper * 3 / 4 && lower > 100, "dashed: {lower}");
    assert_eq!(hash(&preview), DASHED);
}

// Fixture: a 200 x 100 px rectangle with a 60 x 40 px hole, filled non-zero.
const FILL: u64 = 11435696777225297957;

#[test]
fn a_fill_covers_its_shape_not_its_hole_and_has_soft_edges() {
    let outer = vec![pt(2.0, 2.0), pt(12.0, 2.0), pt(12.0, 7.0), pt(2.0, 7.0)];
    // Wound the other way, as a hole is.
    let hole = vec![pt(5.0, 3.0), pt(5.0, 5.0), pt(8.0, 5.0), pt(8.0, 3.0)];
    let sheet = PlotSheet {
        width: 25.6,
        height: 16.0,
        paths: Vec::new(),
        fills: vec![PlotFill {
            contours: vec![outer, hole],
            rgb: [200, 0, 0],
        }],
        notes: Vec::new(),
    };
    let preview = render_sheet(&sheet);
    assert_eq!(pixel(&preview, 60, 60), [255, 255, 255], "outside, above");
    // The shape is x 40..240 px, rows 180..280; the hole x 100..160, rows 220..260.
    assert_eq!(pixel(&preview, 60, 200), [200, 0, 0], "inside the shape");
    assert_eq!(pixel(&preview, 130, 240), [255, 255, 255], "in the hole");
    assert_eq!(pixel(&preview, 20, 230), [255, 255, 255], "outside, left");
    let sheet_half = PlotSheet {
        fills: vec![PlotFill {
            contours: vec![vec![
                pt(0.0, 0.0),
                pt(0.125, 0.0),
                pt(0.125, 16.0),
                pt(0.0, 16.0),
            ]],
            rgb: [0, 0, 0],
        }],
        ..sheet
    };
    let half = render_sheet(&sheet_half);
    // 0.125 mm is 2.5 px: two full pixels and half of the third.
    assert_eq!(pixel(&half, 0, 100), [0, 0, 0]);
    assert_eq!(pixel(&half, 1, 100), [0, 0, 0]);
    let edge = pixel(&half, 2, 100)[0];
    assert!((120..=135).contains(&edge), "{edge}");
    assert_eq!(pixel(&half, 3, 100), [255, 255, 255]);
    assert_eq!(hash(&preview), FILL);
}

#[test]
fn strokes_off_the_sheet_or_not_numbers_do_no_harm() {
    let path = |points: Vec<Point>| PlotPath {
        points,
        closed: false,
        rgb: [0, 0, 0],
        width: 0.1,
    };
    let sheet = PlotSheet {
        width: 25.6,
        height: 16.0,
        paths: vec![
            path(vec![pt(-1e9, -1e9), pt(1e9, 1e9)]),
            path(vec![pt(f64::NAN, 0.0), pt(1.0, 1.0)]),
            path(vec![pt(1.0, 1.0), pt(1.0, 1.0)]),
        ],
        fills: Vec::new(),
        notes: Vec::new(),
    };
    let preview = render_sheet(&sheet);
    assert_eq!(preview.rgba.len(), 512 * 320 * 4);
    let empty = PlotSheet {
        width: 0.0,
        ..sheet
    };
    assert_eq!(dark_pixels(&render_sheet(&empty)), 0);
}

#[test]
fn a_very_large_drawing_still_gives_one_picture() {
    let mut doc = Document::default();
    for i in 0..4000 {
        let x = f64::from(i % 100) * 10.0;
        let y = f64::from(i / 100) * 10.0;
        doc.add(Entity::Circle {
            center: pt(x, y),
            radius: 4.0,
        });
        doc.add(Entity::Line {
            start: pt(x, y),
            end: pt(x + 9.0, y + 9.0),
        });
    }
    let preview = render(&doc).unwrap();
    assert_eq!((preview.width, preview.height), (512, 320));
    assert!(dark_pixels(&preview) > 1000);
}

#[test]
fn huge_extents_do_not_panic() {
    let mut doc = Document::default();
    doc.add(Entity::Line {
        start: pt(0.0, 0.0),
        end: pt(f64::MAX, 1.0),
    });
    let _ = render(&doc);
}
