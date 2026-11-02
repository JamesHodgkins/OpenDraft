//! Reading what a person types for a plot: a scale such as `1:50`, an offset
//! such as `10,5`. Kept apart from the prompts so it is tested on its own.

use crate::{PlotScale, Point, StandardScale};

/// A scale: `Fit`, `1:50`, `1=50` or `2:1` (plotted millimetres : drawing
/// units), or a bare number for millimetres per unit. Both sides must be
/// positive and finite.
pub fn parse_scale(text: &str) -> Option<PlotScale> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("fit") || text.eq_ignore_ascii_case("f") {
        return Some(PlotScale::Standard {
            kind: StandardScale::Fit,
            value: 1.0,
        });
    }
    let positive = |s: &str| {
        s.trim()
            .parse::<f64>()
            .ok()
            .filter(|v| *v > 0.0 && v.is_finite())
    };
    let (paper, drawing) = match text.split_once([':', '=']) {
        Some((paper, drawing)) => (positive(paper)?, positive(drawing)?),
        None => (positive(text)?, 1.0),
    };
    Some(PlotScale::Custom {
        paper_units: paper,
        drawing_units: drawing,
    })
}

/// A scale as it is typed back: `Fit`, or `1:50`.
pub fn scale_text(scale: &PlotScale) -> String {
    match scale {
        PlotScale::Standard {
            kind: StandardScale::Fit,
            ..
        } => "Fit".to_owned(),
        PlotScale::Standard { value, .. } => format!("{}:1", trim(*value)),
        PlotScale::Custom {
            paper_units,
            drawing_units,
        } => format!("{}:{}", trim(*paper_units), trim(*drawing_units)),
    }
}

/// An offset, `x,y` (a space also separates them).
pub fn parse_offset(text: &str) -> Option<Point> {
    let mut parts = text.split([',', ' ']).filter(|p| !p.is_empty());
    let number = |s: Option<&str>| s?.parse::<f64>().ok().filter(|v| v.is_finite());
    let (x, y) = (number(parts.next())?, number(parts.next())?);
    parts.next().is_none().then(|| Point::new(x, y))
}

fn trim(value: f64) -> String {
    let text = format!("{value:.6}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custom(a: f64, b: f64) -> Option<PlotScale> {
        Some(PlotScale::Custom {
            paper_units: a,
            drawing_units: b,
        })
    }

    #[test]
    fn scales_are_read_in_the_usual_spellings() {
        assert_eq!(parse_scale("1:50"), custom(1.0, 50.0));
        assert_eq!(parse_scale(" 1 = 50 "), custom(1.0, 50.0));
        assert_eq!(parse_scale("2:1"), custom(2.0, 1.0));
        assert_eq!(parse_scale("0.5"), custom(0.5, 1.0));
        assert!(matches!(
            parse_scale("FIT"),
            Some(PlotScale::Standard {
                kind: StandardScale::Fit,
                ..
            })
        ));
    }

    #[test]
    fn nonsense_zero_and_negative_scales_are_refused() {
        for bad in [
            "", "abc", "0:1", "1:0", "-1:2", "1:2:3", "1:", "nan", "1:inf",
        ] {
            assert_eq!(parse_scale(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_scale_reads_back_as_typed() {
        assert_eq!(scale_text(&custom(1.0, 50.0).unwrap()), "1:50");
        assert_eq!(scale_text(&custom(0.5, 1.0).unwrap()), "0.5:1");
        assert_eq!(scale_text(&parse_scale("fit").unwrap()), "Fit");
    }

    #[test]
    fn offsets_take_a_comma_or_a_space() {
        assert_eq!(parse_offset("10,5"), Some(Point::new(10.0, 5.0)));
        assert_eq!(parse_offset("-2.5 4"), Some(Point::new(-2.5, 4.0)));
        for bad in ["", "1", "1,2,3", "a,b", "1,nan"] {
            assert_eq!(parse_offset(bad), None, "{bad}");
        }
    }
}
