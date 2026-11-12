//! AutoCAD Color Index: the 255 numbered colours DXF files use.

/// The colour of index `i` (1..=255). Index 7 is "white or black, whichever
/// shows against the background" in AutoCAD; callers map it to the
/// foreground colour before asking for RGB.
pub fn aci_rgb(i: u8) -> (u8, u8, u8) {
    match i {
        1 => (255, 0, 0),
        2 => (255, 255, 0),
        3 => (0, 255, 0),
        4 => (0, 255, 255),
        5 => (0, 0, 255),
        6 => (255, 0, 255),
        7 => (255, 255, 255),
        8 => (65, 65, 65),
        9 => (128, 128, 128),
        10..=249 => {
            // 24 hues, 15 degrees apart; within each, ten shades.
            const SHADES: [(f64, f64); 10] = [
                (1.0, 1.0),
                (0.5, 1.0),
                (1.0, 0.8),
                (0.5, 0.8),
                (1.0, 0.6),
                (0.5, 0.6),
                (1.0, 0.5),
                (0.5, 0.5),
                (1.0, 0.3),
                (0.5, 0.3),
            ];
            let n = usize::from(i) - 10;
            let hue = (n / 10) as f64 * 15.0;
            let (saturation, value) = SHADES[n % 10];
            hsv_to_rgb(hue, saturation, value)
        }
        250..=255 => {
            let grey = [51, 91, 132, 173, 214, 255][usize::from(i) - 250];
            (grey, grey, grey)
        }
        // 0 (ByBlock) and anything else have no colour of their own.
        _ => (255, 255, 255),
    }
}

fn hsv_to_rgb(hue: f64, saturation: f64, value: f64) -> (u8, u8, u8) {
    let c = value * saturation;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = value - c;
    let (r, g, b) = match (hue / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    // Truncated, which is how the published table rounds.
    let channel = |v: f64| ((v + m) * 255.0 + 1e-9) as u8;
    (channel(r), channel(g), channel(b))
}

/// The index whose colour is closest to `rgb` (never 7: the caller handles
/// white and black as the foreground).
pub fn nearest_aci(rgb: (u8, u8, u8)) -> u8 {
    let distance = |i: u8| {
        let (r, g, b) = aci_rgb(i);
        let d = |a: u8, b: u8| i32::from(a) - i32::from(b);
        d(r, rgb.0).pow(2) + d(g, rgb.1).pow(2) + d(b, rgb.2).pow(2)
    };
    (1..=255u8)
        .filter(|&i| i != 7)
        .min_by_key(|&i| distance(i))
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_colours() {
        assert_eq!(aci_rgb(1), (255, 0, 0));
        assert_eq!(aci_rgb(3), (0, 255, 0));
        assert_eq!(aci_rgb(5), (0, 0, 255));
        assert_eq!(aci_rgb(250), (51, 51, 51));
        assert_eq!(aci_rgb(255), (255, 255, 255));
    }

    #[test]
    fn the_generated_shades_match_the_published_table() {
        // Spot checks against AutoCAD's documented values.
        assert_eq!(aci_rgb(10), (255, 0, 0));
        assert_eq!(aci_rgb(11), (255, 127, 127));
        assert_eq!(aci_rgb(12), (204, 0, 0));
        assert_eq!(aci_rgb(14), (153, 0, 0));
        assert_eq!(aci_rgb(18), (76, 0, 0));
        assert_eq!(aci_rgb(19), (76, 38, 38));
        assert_eq!(aci_rgb(30), (255, 127, 0));
        assert_eq!(aci_rgb(50), (255, 255, 0));
        assert_eq!(aci_rgb(70), (127, 255, 0));
        assert_eq!(aci_rgb(90), (0, 255, 0));
        assert_eq!(aci_rgb(150), (0, 127, 255));
    }

    #[test]
    fn nearest_finds_exact_matches_and_close_ones() {
        assert_eq!(nearest_aci((255, 0, 0)), 1);
        assert_eq!(nearest_aci((0, 0, 255)), 5);
        assert_eq!(nearest_aci((250, 10, 5)), 1);
        // Every index (but 7, which duplicates 255's white) maps to itself
        // or to an identical colour.
        for i in 1..=255u8 {
            if i == 7 {
                continue;
            }
            assert_eq!(aci_rgb(nearest_aci(aci_rgb(i))), aci_rgb(i), "index {i}");
        }
    }
}
