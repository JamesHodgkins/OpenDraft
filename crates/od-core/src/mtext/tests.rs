use std::f64::consts::FRAC_PI_2;

use super::*;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn text(contents: &str) -> MText {
    MText::new(p(10.0, 20.0), 2.0, contents, TextStyleId::STANDARD)
}

fn near(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

#[test]
fn codes_round_trip_and_unknown_ones_are_refused() {
    for code in 1..=9 {
        assert_eq!(
            MTextAttachment::from_code(code).unwrap().code() as i64,
            code
        );
    }
    for bad in [0, 10, -1] {
        assert_eq!(MTextAttachment::from_code(bad), None);
    }
    for code in [1, 3, 5] {
        assert_eq!(MTextFlow::from_code(code).unwrap().code() as i64, code);
    }
    assert_eq!(MTextFlow::from_code(2), None);
    assert_eq!(
        LineSpacingStyle::from_code(2),
        Some(LineSpacingStyle::Exact)
    );
    assert_eq!(LineSpacingStyle::from_code(3), None);
}

#[test]
fn a_new_text_is_valid_and_bad_ones_are_found() {
    assert_eq!(text("Hello").validate(), Ok(()));
    let bad: Vec<(&str, MText)> = vec![
        (
            "insertion",
            MText {
                insertion: p(f64::NAN, 0.0),
                ..text("a")
            },
        ),
        (
            "direction",
            MText {
                direction: p(0.0, 0.0),
                ..text("a")
            },
        ),
        (
            "direction",
            MText {
                direction: p(f64::INFINITY, 0.0),
                ..text("a")
            },
        ),
        (
            "height",
            MText {
                nominal_height: 0.0,
                ..text("a")
            },
        ),
        (
            "height",
            MText {
                nominal_height: f64::NAN,
                ..text("a")
            },
        ),
        (
            "width",
            MText {
                reference_width: -1.0,
                ..text("a")
            },
        ),
        (
            "spacing",
            MText {
                line_spacing: EntityLineSpacing {
                    factor: 5.0,
                    ..Default::default()
                },
                ..text("a")
            },
        ),
        (
            "mask",
            MText {
                background: Some(MTextBackground {
                    fill: BackgroundFill::Window,
                    scale: 0.5,
                }),
                ..text("a")
            },
        ),
        (
            "columns",
            MText {
                columns: MTextColumns {
                    kind: ColumnKind::Static,
                    count: 0,
                    ..Default::default()
                },
                ..text("a")
            },
        ),
        (
            "columns",
            MText {
                columns: MTextColumns {
                    kind: ColumnKind::Static,
                    count: 2,
                    heights: vec![1.0, 1.0, 1.0],
                    ..Default::default()
                },
                ..text("a")
            },
        ),
        ("contents", text(&"a".repeat((1 << 20) + 1))),
    ];
    for (what, entity) in bad {
        assert!(entity.validate().is_err(), "{what}");
    }
    // Column settings are ignored when there are no columns.
    let none = MText {
        columns: MTextColumns {
            count: 0,
            heights: vec![-1.0],
            ..Default::default()
        },
        ..text("a")
    };
    assert_eq!(none.validate(), Ok(()));
}

#[test]
fn the_frame_follows_attachment_and_direction() {
    let mut t = text("abcde\\Pfg");
    t.reference_width = 10.0;
    let (w, h) = t.extent();
    assert_eq!(w, 10.0);
    let [tl, tr, br, bl] = t.frame();
    assert!(near(tl, p(10.0, 20.0)), "top left is the insertion point");
    assert!(near(tr, p(20.0, 20.0)));
    assert!(near(br, p(20.0, 20.0 - h)));
    assert!(near(bl, p(10.0, 20.0 - h)));

    t.attachment = MTextAttachment::BottomRight;
    let [tl, _, br, _] = t.frame();
    assert!(near(br, p(10.0, 20.0)));
    assert!(near(tl, p(0.0, 20.0 + h)));

    t.attachment = MTextAttachment::MiddleCenter;
    let b = t.bounds();
    assert!(near(b.min, p(5.0, 20.0 - h / 2.0)) && near(b.max, p(15.0, 20.0 + h / 2.0)));

    // Turned a quarter, the box lies along the y axis, left edge at the insertion point.
    t.attachment = MTextAttachment::TopLeft;
    t.direction = p(0.0, 3.0);
    let [tl, tr, _, bl] = t.frame();
    assert!(near(tl, p(10.0, 20.0)));
    assert!(near(tr, p(10.0, 30.0)));
    assert!(near(bl, p(10.0 + h, 20.0)), "down is to the east");
}

#[test]
fn contains_follows_the_turned_frame() {
    let mut t = text("abcde");
    t.reference_width = 10.0;
    let (_, h) = t.extent();
    assert!(t.contains(p(15.0, 20.0 - h / 2.0)));
    assert!(t.contains(p(10.0, 20.0)), "an edge counts");
    assert!(!t.contains(p(9.9, 15.0)) && !t.contains(p(15.0, 20.1)));
    t.direction = p(0.0, 1.0);
    // Turned a quarter: the box runs north from the insertion point, down is east.
    assert!(t.contains(p(10.0 + h / 2.0, 25.0)));
    assert!(!t.contains(p(15.0, 20.0 - h / 2.0)));
}

#[test]
fn layout_points_and_drawing_points_map_to_each_other() {
    let mut t = text("abc");
    t.attachment = MTextAttachment::MiddleRight;
    t.direction = p(3.0, 4.0);
    let layout = t.layout(&crate::TextStyle::standard());
    for (x, y) in [(0.0, 0.0), (1.5, -0.7), (-2.0, 3.0)] {
        let world = t.world_point(&layout, x, y);
        let (lx, ly) = t.layout_point(&layout, world);
        assert!((lx - x).abs() < 1e-9 && (ly - y).abs() < 1e-9);
    }
}

#[test]
fn the_frame_without_a_width_fits_the_text() {
    let one = text("abcd");
    let two = text("abcd\\Pab");
    let (w1, h1) = one.extent();
    let (w2, h2) = two.extent();
    assert!((w1 - w2).abs() < 1e-9, "the longest line sets the width");
    assert!(h2 > h1 + 2.0 * 5.0 / 3.0 - 1e-9, "a line adds a pitch");
    assert!(w1 > 2.0 * 2.0, "four letters are wider than two heights");
    assert!(text("").extent().0 > 0.0, "an empty text still has a frame");
}

#[test]
fn snap_points_are_the_insertion_corners_and_edge_middles() {
    let mut t = text("a");
    t.reference_width = 8.0;
    let points = t.snap_points();
    assert_eq!(points.len(), 9);
    assert!(near(points[0], t.insertion));
    let (_, h) = t.extent();
    assert!(points.iter().any(|q| near(*q, p(14.0, 20.0))), "top middle");
    assert!(
        points.iter().any(|q| near(*q, p(10.0, 20.0 - h / 2.0))),
        "left middle"
    );
}

#[test]
fn grips_move_the_text_and_set_the_width() {
    let free = text("abc");
    assert_eq!(free.grips().len(), 1, "no width grip without a width");

    let mut t = text("abc");
    t.reference_width = 10.0;
    let grips = t.grips();
    assert_eq!(grips.len(), 4, "move, right, bottom and corner");
    let moved = t.with_grip_moved(0, p(15.0, 25.0)).unwrap();
    assert!(near(moved.insertion, p(15.0, 25.0)));
    assert_eq!(moved.reference_width, 10.0);

    let wider = t
        .with_grip_moved(1, p(grips[1].at.x + 5.0, grips[1].at.y + 7.0))
        .unwrap();
    assert!(
        (wider.reference_width - 15.0).abs() < 1e-9,
        "only the part along the text counts"
    );
    assert!(near(wider.insertion, t.insertion));
    assert!(
        t.with_grip_moved(1, p(grips[1].at.x - 10.0, 0.0)).is_none(),
        "no width left"
    );
    assert!(t.with_grip_moved(4, p(0.0, 0.0)).is_none());

    // The left edge stays put when the text is attached on the right.
    t.attachment = MTextAttachment::TopRight;
    let left_before = t.frame()[0];
    let grips = t.grips();
    let wider = t
        .with_grip_moved(1, p(grips[1].at.x + 4.0, grips[1].at.y))
        .unwrap();
    assert!(near(wider.frame()[0], left_before));
}

#[test]
fn moving_and_turning_keep_the_contents() {
    let mut t = text("a\\H3;b");
    t.direction = p(2.0, 0.0);
    let moved = t.translated(1.0, -1.0);
    assert!(near(moved.insertion, p(11.0, 19.0)));
    assert_eq!(moved.contents, t.contents);

    let turned = t.rotated(p(0.0, 0.0), FRAC_PI_2);
    assert!(near(turned.insertion, p(-20.0, 10.0)));
    assert!(near(turned.direction, p(0.0, 2.0)));
    assert_eq!(turned.nominal_height, 2.0);
}

#[test]
fn scaling_changes_sizes_and_absolute_heights_only() {
    let mut t = text("a\\H3;b\\H2x;c\\fArial|b1;\\C1;d");
    t.reference_width = 10.0;
    t.columns = MTextColumns {
        kind: ColumnKind::Static,
        count: 2,
        width: 4.0,
        gutter: 1.0,
        heights: vec![5.0],
        ..Default::default()
    };
    let big = t.scaled(p(0.0, 0.0), 2.0);
    assert!(near(big.insertion, p(20.0, 40.0)));
    assert_eq!(big.nominal_height, 4.0);
    assert_eq!(big.reference_width, 20.0);
    assert_eq!((big.columns.width, big.columns.gutter), (8.0, 2.0));
    assert_eq!(big.columns.heights, [10.0]);
    assert_eq!(big.contents, "a\\H6;b\\H2x;c\\fArial|b1;\\C1;d");
    assert_eq!(big.validate(), Ok(()));
}

#[test]
fn mirroring_is_refused_and_so_is_scaling_paragraph_formatting() {
    assert!(text("a").mirror_refusal().is_some());
    assert_eq!(text("plain \\H2;text").scale_refusal(), None);
    assert!(text("\\pxqd,l3;item").scale_refusal().is_some());
}

#[test]
fn scaling_text_scales_its_indents_and_tab_stops() {
    let t = text(r"\pxi-4,l4,t2,6;item");
    assert_eq!(t.scale_refusal(), None);
    let big = t.scaled(p(0.0, 0.0), 2.0);
    assert_eq!(big.contents, r"\pxi-8,l8,t4,12;item");
}

#[test]
fn a_set_box_height_is_the_frame_and_the_grips_resize_it() {
    let mut t = text("one");
    t.reference_width = 20.0;
    let line = t.extent().1;
    t.reference_height = 30.0;
    assert_eq!(
        t.extent(),
        (20.0, 30.0),
        "the box is as set, taller than its text"
    );
    let frame = t.frame();
    assert!(near(frame[2], p(30.0, -10.0)) && near(frame[0], p(10.0, 20.0)));
    assert!(
        t.contains(p(15.0, -5.0)),
        "empty space in the box is inside it"
    );
    let grips = t.grips();
    assert_eq!(grips.len(), 4);
    assert!(near(grips[2].at, p(20.0, -10.0)), "bottom middle");
    assert!(near(grips[3].at, p(30.0, -10.0)), "corner");
    // The bottom grip changes only the height; the top left stays.
    let taller = t.with_grip_moved(2, p(20.0, -20.0)).unwrap();
    assert_eq!(
        (taller.reference_width, taller.reference_height),
        (20.0, 40.0)
    );
    assert!(near(taller.insertion, t.insertion));
    // The corner changes both; no box is shorter than its text.
    let both = t.with_grip_moved(3, p(35.0, 20.0 - line / 2.0)).unwrap();
    assert_eq!(both.reference_width, 25.0);
    assert!((both.extent().1 - line).abs() < 1e-9);
    // A bottom-attached box moves its insertion with the bottom edge.
    let mut low = t.clone();
    low.attachment = MTextAttachment::BottomLeft;
    let moved = low
        .with_grip_moved(2, p(20.0, low.grips()[2].at.y - 10.0))
        .unwrap();
    assert!(
        (moved.frame()[2].y - (low.frame()[2].y - 10.0)).abs() < 1e-9,
        "the bottom edge goes where it is dragged"
    );
}
