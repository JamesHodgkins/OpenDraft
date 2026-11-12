use super::*;

const SAMPLE: &str = "odroman 1\nmetrics unit=1000 cap=1000 weight=100\nglyph 0041 adv=700\n  stroke M 0 0  L 350 1000  C 400 900 500 500 700 0\n  stroke w=0.5 M 100 300  L 600 300\n  dot 10 20\n";

fn err(text: &str) -> StrokeFontError {
    StrokeFont::parse(text).unwrap_err()
}

#[test]
fn a_small_font_is_read() {
    let font = StrokeFont::parse(SAMPLE).unwrap();
    let a = font.glyph_of('A').unwrap();
    assert_eq!(a.advance, 700.0);
    assert_eq!(a.strokes.len(), 3);
    assert_eq!(a.strokes[0].segments.len(), 2);
    assert_eq!(a.strokes[1].weight, 0.5);
    assert!(a.strokes[2].segments.is_empty());
    assert_eq!(font.metrics.default_weight, 0.1);
    assert_eq!(font.slot_of('A'), Some(0));
    assert!(font.glyph_of('B').is_none());
}

#[test]
fn the_shipped_font_reads_and_covers_letters_digits_and_space() {
    let font = od_roman().unwrap();
    for ch in ('A'..='Z').chain('a'..='z').chain('0'..='9') {
        let g = font
            .glyph_of(ch)
            .unwrap_or_else(|| panic!("no glyph for {ch}"));
        assert!(!g.strokes.is_empty(), "{ch} has no strokes");
        assert!(g.advance > 0.0, "{ch} has no advance");
    }
    let space = font.glyph_of(' ').unwrap();
    assert!(space.strokes.is_empty());
    assert_eq!(space.advance, font.metrics.space);
}

fn extent(glyph: &StrokeGlyph) -> (f64, f64) {
    let mut ys = Vec::new();
    for s in &glyph.strokes {
        ys.push(s.start.1);
        for seg in &s.segments {
            match *seg {
                Segment::Line(p) | Segment::Cubic(_, _, p) => ys.push(p.1),
            }
        }
    }
    let low = ys.iter().copied().fold(f64::MAX, f64::min);
    let high = ys.iter().copied().fold(f64::MIN, f64::max);
    (low, high)
}

#[test]
fn capitals_stand_on_the_baseline_at_cap_height() {
    let font = od_roman().unwrap();
    for ch in "HEILTZ".chars() {
        let (low, high) = extent(font.glyph_of(ch).unwrap());
        assert!(low.abs() < 60.0, "{ch} bottom {low}");
        assert!((high - 1000.0).abs() < 60.0, "{ch} top {high}");
    }
    let (_, high) = extent(font.glyph_of('x').unwrap());
    assert!((high - font.metrics.x_height).abs() < 60.0);
}

#[test]
fn every_coordinate_is_in_a_sane_range() {
    let font = od_roman().unwrap();
    for g in (' '..='~').filter_map(|c| font.glyph_of(c)) {
        if g.strokes.is_empty() {
            continue;
        }
        let (low, high) = extent(g);
        assert!(low > -600.0 && high < 1500.0, "{}: {low}..{high}", g.ch);
    }
}

#[test]
fn bad_files_are_refused_with_a_line() {
    assert_eq!(err("").line, 0);
    assert_eq!(err("hello").line, 1);
    assert!(err("odroman 1\nglyph 0041 adv=1\n")
        .message
        .contains("metrics"));
    assert_eq!(err("odroman 1\nmetrics\nstroke M 0 0\n").line, 3);
    let head = "odroman 1\nmetrics\nglyph 0041 adv=1\n";
    let with = |tail: &str| err(&format!("{head}{tail}")).message;
    assert!(with("stroke L 1 1\n").contains("starts with M"));
    assert!(with("stroke M 0 0 Q 1 1\n").contains("unknown command"));
    assert!(with("stroke M 0 0 L 1\n").contains("two numbers"));
    assert!(with("glyph 0041 adv=1\n").contains("twice"));
    assert!(with("stroke w=0 M 0 0\n").contains("positive"));
    assert!(with("stroke M 0 NaN\n").contains("usable number"));
    assert!(err("odroman 1\nmetrics\nglyph 0041 adv=1e99\n")
        .message
        .contains("usable number"));
    assert!(err("odroman 1\nmetrics\nglyph D800 adv=1\n")
        .message
        .contains("code point"));
    assert!(err("odroman 1\nmetrics\nglyph 0041 adv=-1\n")
        .message
        .contains("negative"));
    assert!(err("odroman 1\nmetrics bogus=1\n")
        .message
        .contains("unknown metric"));
    assert!(err("odroman 1\nmetrics\nstroke M 0 0\n")
        .message
        .contains("before any glyph"));
}

#[test]
fn size_limits_are_enforced() {
    let mut many = String::from("odroman 1\nmetrics\nglyph 0041 adv=1\nstroke M 0 0");
    for _ in 0..300 {
        many.push_str(" L 1 1");
    }
    assert!(err(&many).message.contains("too many segments"));
    let mut strokes = String::from("odroman 1\nmetrics\nglyph 0041 adv=1\n");
    for _ in 0..70 {
        strokes.push_str("dot 1 1\n");
    }
    assert!(err(&strokes).message.contains("too many strokes"));
    assert!(err(&" ".repeat(parse::MAX_BYTES + 1))
        .message
        .contains("too large"));
}

#[test]
fn a_dot_takes_a_weight_like_a_stroke() {
    let text = "odroman 1\nmetrics\nglyph 002E adv=100\ndot w=0.5 10 20\ndot 30 40\n";
    let font = StrokeFont::parse(text).unwrap();
    let g = font.glyph_of('.').unwrap();
    assert_eq!(g.strokes[0].weight, 0.5);
    assert_eq!(g.strokes[0].start, (10.0, 20.0));
    assert_eq!(g.strokes[1].weight, 1.0);
    let bad = "odroman 1\nmetrics\nglyph 0041 adv=1\ndot w=0 1 2\n";
    assert!(err(bad).message.contains("positive"));
}
