//! Dashes along one pattern line. The dash cycle starts where the line's
//! own origin is, so every piece of a line that a boundary cuts in two keeps
//! the phase it would have had uncut.

use super::HatchError;

/// A piece of a dashed line, as distances along it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Piece {
    Dash(f64, f64),
    Dot(f64),
}

/// The length of one dash cycle, or an error if it is empty or not a number.
pub(super) fn period(dashes: &[f64]) -> Result<f64, HatchError> {
    if dashes.iter().any(|d| !d.is_finite()) {
        return Err(HatchError::NonFinite);
    }
    let period: f64 = dashes.iter().map(|d| d.abs()).sum();
    if period > 0.0 {
        Ok(period)
    } else {
        Err(HatchError::PatternDashes)
    }
}

/// The pieces of the dash pattern that fall within `[from, to]`, when the
/// pattern starts at `origin` and repeats every `period`. At most `room`
/// pieces are produced; more than that is [`HatchError::TooDense`].
pub(super) fn walk(
    dashes: &[f64],
    period: f64,
    origin: f64,
    from: f64,
    to: f64,
    room: usize,
    emit: &mut dyn FnMut(Piece),
) -> Result<usize, HatchError> {
    let cycles = ((to - from) / period).ceil() + 2.0;
    let estimate = cycles * dashes.len() as f64;
    if estimate.is_nan() || estimate > room as f64 {
        return Err(HatchError::TooDense {
            estimated: if estimate.is_finite() {
                estimate as u64
            } else {
                u64::MAX
            },
            limit: room as u64,
        });
    }
    let mut at = origin + ((from - origin) / period).floor() * period;
    let mut made = 0;
    while at <= to {
        for &length in dashes {
            if at > to {
                break;
            }
            if length > 0.0 {
                let (a, b) = (at.max(from), (at + length).min(to));
                if a < b {
                    emit(Piece::Dash(a, b));
                    made += 1;
                }
            } else if length == 0.0 && at >= from {
                emit(Piece::Dot(at));
                made += 1;
            }
            at += length.abs();
        }
    }
    Ok(made)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pieces(dashes: &[f64], origin: f64, from: f64, to: f64) -> Vec<Piece> {
        let mut out = Vec::new();
        let p = period(dashes).unwrap();
        walk(dashes, p, origin, from, to, 1000, &mut |x| out.push(x)).unwrap();
        out
    }

    #[test]
    fn dashes_and_gaps_repeat_from_the_origin() {
        let got = pieces(&[2.0, -1.0], 0.0, 0.0, 7.0);
        assert_eq!(
            got,
            [
                Piece::Dash(0.0, 2.0),
                Piece::Dash(3.0, 5.0),
                Piece::Dash(6.0, 7.0)
            ]
        );
    }

    #[test]
    fn an_interval_that_starts_mid_dash_keeps_the_phase() {
        // The dash from 3 to 5 is cut at 4; the next starts at 6 as before.
        let got = pieces(&[2.0, -1.0], 0.0, 4.0, 8.0);
        assert_eq!(got, [Piece::Dash(4.0, 5.0), Piece::Dash(6.0, 8.0)]);
        // Starting in a gap.
        let got = pieces(&[2.0, -1.0], 0.0, 5.5, 7.0);
        assert_eq!(got, [Piece::Dash(6.0, 7.0)]);
    }

    #[test]
    fn a_negative_start_and_a_shifted_origin_work() {
        let got = pieces(&[1.0, -1.0], 0.5, -2.0, 0.0);
        // Dashes begin at 0.5 + 2n: -1.5..-0.5 and -3.5..-2.5 (cut at -2).
        assert_eq!(got, [Piece::Dash(-1.5, -0.5)]);
    }

    #[test]
    fn zero_is_a_dot_and_it_sits_at_its_place_in_the_cycle() {
        let got = pieces(&[0.0, -2.0], 0.0, 0.0, 5.0);
        assert_eq!(got, [Piece::Dot(0.0), Piece::Dot(2.0), Piece::Dot(4.0)]);
    }

    #[test]
    fn an_empty_cycle_is_refused_and_a_dense_one_is_too_dense() {
        assert_eq!(period(&[0.0, 0.0]), Err(HatchError::PatternDashes));
        assert_eq!(period(&[f64::NAN]), Err(HatchError::NonFinite));
        let mut sink = |_| {};
        let r = walk(&[1e-9, -1e-9], 2e-9, 0.0, 0.0, 1.0, 1000, &mut sink);
        assert!(matches!(r, Err(HatchError::TooDense { .. })));
    }
}
