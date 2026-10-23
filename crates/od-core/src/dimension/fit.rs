//! Deciding what fits between the extension lines (`DIMATFIT`, `DIMTIX`).

/// Whether the arrows and the text sit between the extension lines.
///
/// `span` is the room between them, `arrow` one arrow's length and `text`
/// the text's width with its clearance (0 for no text). `mode` is
/// `DIMATFIT`: 0 moves both out, 1 the arrows first, 2 the text first, 3
/// whichever leaves the better fit. `force_text` is `DIMTIX`.
pub(super) fn fit(span: f64, arrow: f64, text: f64, mode: u8, force_text: bool) -> (bool, bool) {
    let both = span >= 2.0 * arrow + text;
    let arrows = span >= 2.0 * arrow;
    let words = span >= text;
    let (arrows_in, text_in) = if both {
        (true, true)
    } else {
        match mode {
            0 => (false, false),
            1 => (false, words),
            2 => (arrows, false),
            _ => match (arrows, words) {
                (true, true) if text >= 2.0 * arrow => (false, true),
                (true, true) => (true, false),
                (false, true) => (false, true),
                (true, false) => (true, false),
                (false, false) => (false, false),
            },
        }
    };
    (arrows_in, text_in || force_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_fits_in_a_long_span() {
        assert_eq!(fit(100.0, 2.5, 10.0, 3, false), (true, true));
    }

    #[test]
    fn best_fit_keeps_the_text_when_only_it_fits() {
        // 2 arrows need 5, the text 10: room for the text alone.
        assert_eq!(fit(12.0, 2.5, 10.0, 3, false), (false, true));
    }

    #[test]
    fn best_fit_keeps_the_arrows_when_only_they_fit() {
        assert_eq!(fit(6.0, 2.5, 10.0, 3, false), (true, false));
    }

    #[test]
    fn nothing_fits_in_a_tiny_span() {
        assert_eq!(fit(2.0, 2.5, 10.0, 3, false), (false, false));
    }

    #[test]
    fn the_modes_move_in_their_own_order() {
        assert_eq!(fit(12.0, 2.5, 10.0, 0, false), (false, false));
        assert_eq!(fit(12.0, 2.5, 10.0, 1, false), (false, true));
        assert_eq!(fit(12.0, 2.5, 10.0, 2, false), (true, false));
    }

    #[test]
    fn forced_text_stays_inside() {
        assert_eq!(fit(2.0, 2.5, 10.0, 3, true), (false, true));
    }

    #[test]
    fn no_text_needs_only_the_arrows() {
        assert_eq!(fit(5.0, 2.5, 0.0, 3, false), (true, true));
    }
}
