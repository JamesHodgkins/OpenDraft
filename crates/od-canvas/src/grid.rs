use std::ops::RangeInclusive;

/// Finest step the grid will subdivide to.
const FINEST_STEP: f64 = 1e-9;

/// Grid step to draw: `base`, coarsened by factors of 5 until lines are at
/// least `min_px` apart, so the grid never turns into a solid wash, and
/// subdivided by 5 while they are more than `5 * min_px` apart, so zooming
/// in keeps adding lines.
pub fn grid_step(base: f64, zoom: f64, min_px: f64) -> f64 {
    if !(base.is_finite() && base > 0.0 && zoom.is_finite() && zoom > 0.0) {
        return base;
    }
    let mut step = base;
    while step * zoom < min_px && step.is_finite() {
        step *= 5.0;
    }
    while step * zoom > 5.0 * min_px && step / 5.0 > FINEST_STEP {
        step /= 5.0;
    }
    step
}

/// Indices of the grid lines (`index * step`) that fall inside `min..=max`.
pub fn line_indices(min: f64, max: f64, step: f64) -> RangeInclusive<i64> {
    if !(step.is_finite() && step > 0.0 && min.is_finite() && max.is_finite()) {
        return RangeInclusive::new(1, 0);
    }
    (min / step).ceil() as i64..=(max / step).floor() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_is_unchanged_when_lines_are_far_enough_apart() {
        assert_eq!(grid_step(10.0, 2.0, 8.0), 10.0);
    }

    #[test]
    fn step_coarsens_by_five_when_zoomed_out() {
        assert_eq!(grid_step(10.0, 0.1, 8.0), 250.0);
        assert!(grid_step(10.0, 0.1, 8.0) * 0.1 >= 8.0);
    }

    #[test]
    fn step_subdivides_by_five_when_zoomed_in() {
        assert_eq!(grid_step(10.0, 20.0, 8.0), 2.0);
        assert!(grid_step(10.0, 1000.0, 8.0) * 1000.0 <= 40.0);
        assert!(grid_step(10.0, 1000.0, 8.0) * 1000.0 >= 8.0);
    }

    #[test]
    fn invalid_input_is_returned_untouched() {
        assert_eq!(grid_step(0.0, 1.0, 8.0), 0.0);
        assert_eq!(grid_step(10.0, 0.0, 8.0), 10.0);
    }

    #[test]
    fn indices_cover_the_visible_range() {
        assert_eq!(line_indices(-25.0, 25.0, 10.0), -2..=2);
        assert_eq!(line_indices(1.0, 9.0, 10.0).count(), 0);
        assert_eq!(line_indices(0.0, 0.0, 0.0).count(), 0);
    }
}
