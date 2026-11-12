//! Tests of tracking (`\T`): AutoCAD adds the same space after every
//! character, whatever its width, in proportion to the text height. Measured
//! from AutoCAD's own plots of Arial text at heights 1 to 5 (the
//! `ACAD Letter Spacing` samples): about 0.44 em, or 0.6145 of the cap
//! height, for each whole step of tracking above or below 1.0.

use super::tests::lay;
use super::*;
use crate::text::charstate::TRACKING_PER_HEIGHT;
use crate::text_style::TextStyleId;
use crate::Point;

const HEIGHT: f64 = 2.5;

/// The x of every glyph of `contents`.
fn xs(contents: &str) -> Vec<f64> {
    let m = MText::new(
        Point::new(0.0, 0.0),
        HEIGHT,
        contents,
        TextStyleId::STANDARD,
    );
    lay(&m).glyphs.iter().map(|g| g.x).collect()
}

/// How far each glyph moved on from the one before, beyond what it did untracked.
fn extras(tracked: &str, plain: &str) -> Vec<f64> {
    let (a, b) = (xs(tracked), xs(plain));
    assert_eq!(a.len(), b.len());
    (1..a.len())
        .map(|i| (a[i] - a[i - 1]) - (b[i] - b[i - 1]))
        .collect()
}

#[test]
fn tracking_adds_the_same_space_after_wide_and_narrow_letters() {
    let step = TRACKING_PER_HEIGHT * HEIGHT;
    for extra in extras("{\\T2;IWMLiO}", "IWMLiO") {
        assert!((extra - step).abs() < 1e-9, "{extra} != {step}");
    }
    for extra in extras("{\\T3;IWMLiO}", "IWMLiO") {
        assert!(
            (extra - 2.0 * step).abs() < 1e-9,
            "{extra} != {}",
            2.0 * step
        );
    }
}

#[test]
fn tracking_is_linear_and_below_one_tightens() {
    let step = TRACKING_PER_HEIGHT * HEIGHT;
    for (t, want) in [("0.75", -0.25), ("1.25", 0.25), ("1.5", 0.5)] {
        let extras = extras(&format!("{{\\T{t};IWMLiO}}"), "IWMLiO");
        for extra in extras {
            assert!((extra - want * step).abs() < 1e-9, "T{t}: {extra}");
        }
    }
    assert_eq!(xs("{\\T1;IWMLiO}"), xs("IWMLiO"), "1.0 is plain");
}

#[test]
fn spaces_take_the_same_extra_as_letters() {
    // "A B": the space is a character like any other, so the gap between A
    // and B grows by two steps, one after the A and one after the space.
    let step = TRACKING_PER_HEIGHT * HEIGHT;
    let (plain, tracked) = (xs("A B"), xs("{\\T2;A B}"));
    assert_eq!((plain.len(), tracked.len()), (2, 2));
    let (gap, plain_gap) = (tracked[1] - tracked[0], plain[1] - plain[0]);
    assert!(
        (gap - plain_gap - 2.0 * step).abs() < 1e-9,
        "{gap} {plain_gap}"
    );
}

#[test]
fn it_scales_with_the_text_height_and_never_runs_backwards() {
    let at = |h: f64, contents: &str| {
        let m = MText::new(Point::new(0.0, 0.0), h, contents, TextStyleId::STANDARD);
        lay(&m).glyphs.iter().map(|g| g.x).collect::<Vec<_>>()
    };
    let (small, large) = (at(1.0, "{\\T2;ab}"), at(4.0, "{\\T2;ab}"));
    let (small_plain, large_plain) = (at(1.0, "ab"), at(4.0, "ab"));
    let extra_small = (small[1] - small[0]) - (small_plain[1] - small_plain[0]);
    let extra_large = (large[1] - large[0]) - (large_plain[1] - large_plain[0]);
    assert!((extra_large / extra_small - 4.0).abs() < 1e-9);
    // Far below 1.0 the letters can close up completely but not overlap backwards.
    let tight = at(2.5, "{\\T0.01;iiii}");
    assert!(tight.windows(2).all(|w| w[1] >= w[0] - 1e-9), "{tight:?}");
}
