//! Alternate units and tolerances on a dimension's text: what is written
//! besides the measurement. A tolerance is stacked text, so the drawing
//! uses the text as multiline text contents ([`Dimension::contents`]); the
//! plain label ([`Dimension::label`]) has the alternate value but no
//! tolerance.

use super::style::DimStyle;
use super::text::{expand_codes, measurement_text};
use super::{DimKind, DimText, Dimension};
use crate::text::escape_for_contents;
use crate::units::format_length;

/// `value` written with `precision` decimals in the style's units.
fn number(style: &DimStyle, value: f64, precision: u8) -> String {
    let with = DimStyle {
        precision,
        post: String::new(),
        round_to: 0.0,
        ..style.clone()
    };
    let mut shown = measurement_text(&with, value, "");
    if shown.is_empty() {
        shown = format_length(&with.units(), value);
    }
    shown
}

/// A signed tolerance value: `+0.10`, `-0.05`, `0`.
fn signed(style: &DimStyle, value: f64, plus: bool) -> String {
    let shown = number(style, value.abs(), style.tol_precision);
    if value == 0.0 {
        return number(style, 0.0, style.tol_precision);
    }
    let positive = (value > 0.0) == plus;
    format!("{}{shown}", if positive { '+' } else { '-' })
}

/// The tolerance text size as a markup prefix, empty at full size.
fn scaled(style: &DimStyle, text: &str) -> String {
    if (style.tol_scale - 1.0).abs() < 1e-9 {
        text.to_owned()
    } else {
        format!("{{\\H{}x;{text}}}", style.tol_scale)
    }
}

impl Dimension {
    /// The alternate value, `[1.18]`, when the style has one and the
    /// dimension measures a length.
    fn alternate(&self, measured: f64) -> Option<String> {
        let s = &self.style;
        let length = matches!(self.kind, DimKind::Linear { .. } | DimKind::Radial { .. })
            || matches!(self.kind, DimKind::ArcLength { .. });
        (s.alt_on && length).then(|| {
            let value = number(
                s,
                measured * s.linear_factor * s.alt_factor,
                s.alt_precision,
            );
            format!("[{value}{}]", expand_codes(&s.alt_post))
        })
    }

    /// The measurement as written: with its prefix and suffix and the
    /// alternate value. Empty when there is nothing to measure.
    pub(super) fn measured_label(&self) -> String {
        let Some(m) = self.measurement() else {
            return String::new();
        };
        match &self.kind {
            DimKind::Angular { .. } => super::text::angle_text(&self.style, m),
            _ => {
                let primary = measurement_text(&self.style, m, self.prefix());
                match self.alternate(m) {
                    Some(alt) => format!("{primary} {alt}"),
                    None => primary,
                }
            }
        }
    }

    /// The measurement as text contents: [`Dimension::measured_label`] with
    /// the tolerance stacked on or in place of it.
    fn measured_contents(&self) -> String {
        let (s, plain) = (&self.style, self.measured_label());
        let Some(m) = self.measurement() else {
            return String::new();
        };
        let tolerable = matches!(
            self.kind,
            DimKind::Linear { .. } | DimKind::Radial { .. } | DimKind::ArcLength { .. }
        );
        if !tolerable || s.tol_mode == 0 {
            return escape_for_contents(&plain);
        }
        let factor = s.linear_factor;
        match s.tol_mode {
            1 => format!(
                "{}{}",
                escape_for_contents(&plain),
                scaled(
                    s,
                    &format!("±{}", number(s, s.tol_plus.abs() * factor, s.tol_precision))
                )
            ),
            2 => format!(
                "{}{}",
                escape_for_contents(&plain),
                scaled(
                    s,
                    &format!(
                        "\\S{}^{};",
                        signed(s, s.tol_plus * factor, true),
                        signed(s, s.tol_minus * factor, false)
                    )
                )
            ),
            _ => {
                let upper = measurement_text(s, m + s.tol_plus, self.prefix());
                let lower = measurement_text(s, m - s.tol_minus.abs(), self.prefix());
                scaled(
                    s,
                    &format!(
                        "\\S{}^{};",
                        escape_for_contents(&upper),
                        escape_for_contents(&lower)
                    ),
                )
            }
        }
    }

    /// The text drawn, as multiline text contents.
    pub(crate) fn contents(&self) -> String {
        match &self.text {
            DimText::Measured => self.measured_contents(),
            DimText::Hidden => String::new(),
            DimText::Custom(typed) => {
                let typed = expand_codes(typed);
                match typed.split_once("<>") {
                    Some((before, after)) => format!(
                        "{}{}{}",
                        escape_for_contents(before),
                        self.measured_contents(),
                        escape_for_contents(after)
                    ),
                    None => escape_for_contents(&typed),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DimStyle, Point};

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn with(style: DimStyle) -> Dimension {
        Dimension::aligned(p(0.0, 0.0), p(10.0, 0.0), p(0.0, 5.0), &style)
    }

    #[test]
    fn the_alternate_value_follows_in_brackets() {
        let d = with(DimStyle {
            alt_on: true,
            alt_post: "in".to_owned(),
            ..DimStyle::default()
        });
        assert_eq!(d.label(), "10.00 [254.00in]");
        let angular = Dimension::angular(
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(0.0, 10.0),
            p(5.0, 5.0),
            false,
            &DimStyle {
                alt_on: true,
                ..DimStyle::default()
            },
        );
        assert_eq!(angular.label(), "90", "angles have no alternate");
    }

    #[test]
    fn a_symmetrical_tolerance_follows_the_value() {
        let d = with(DimStyle {
            tol_mode: 1,
            tol_plus: 0.1,
            ..DimStyle::default()
        });
        assert_eq!(d.contents(), "10.00±0.10");
        assert_eq!(d.label(), "10.00", "the plain label has no tolerance");
    }

    #[test]
    fn a_deviation_stacks_plus_over_minus_and_a_scale_shrinks_it() {
        let d = with(DimStyle {
            tol_mode: 2,
            tol_plus: 0.1,
            tol_minus: 0.05,
            tol_scale: 0.7,
            ..DimStyle::default()
        });
        assert_eq!(d.contents(), "10.00{\\H0.7x;\\S+0.10^-0.05;}");
    }

    #[test]
    fn limits_replace_the_value_with_its_bounds() {
        let d = with(DimStyle {
            tol_mode: 3,
            tol_plus: 0.1,
            tol_minus: 0.2,
            ..DimStyle::default()
        });
        assert_eq!(d.contents(), "\\S10.10^9.80;");
    }

    #[test]
    fn typed_text_keeps_its_own_words_around_the_measurement() {
        let mut d = with(DimStyle {
            tol_mode: 1,
            tol_plus: 0.1,
            ..DimStyle::default()
        });
        d.text = DimText::Custom("{<>} TYP".to_owned());
        assert_eq!(d.contents(), "\\{10.00±0.10\\} TYP");
        d.text = DimText::Custom("none".to_owned());
        assert_eq!(d.contents(), "none");
        d.text = DimText::Hidden;
        assert_eq!(d.contents(), "");
    }

    #[test]
    fn the_stacked_text_lays_out_as_text() {
        let d = with(DimStyle {
            tol_mode: 2,
            tol_plus: 0.1,
            tol_minus: 0.05,
            ..DimStyle::default()
        });
        let plain = crate::dimension::font::text_width("10.00", 2.5);
        let with_tolerance = crate::dimension::font::contents_width(&d.contents(), 2.5);
        assert!(with_tolerance > plain);
    }
}
