//! The working copy the Dimension Style dialog (DIMSTYLE) edits, the way the
//! dialog's controls map onto the style's values, and the sample drawing its
//! preview shows. Nothing here touches the drawing until OK.

use od_core::{DimStyle, Dimension, Entity, Point};

/// A control that is one choice of several, over values the style keeps as
/// separate flags.
pub(crate) mod alignment {
    use od_core::DimStyle;

    pub const HORIZONTAL: usize = 0;
    pub const ALIGNED: usize = 1;
    pub const ISO: usize = 2;

    /// How text inside and outside the extension lines is turned (`DIMTIH`
    /// and `DIMTOH`).
    pub fn of(style: &DimStyle) -> usize {
        match (style.inside_horizontal, style.outside_horizontal) {
            (true, true) => HORIZONTAL,
            (false, true) => ISO,
            // Horizontal outside only is not one of the three; the nearest
            // is aligned.
            _ => ALIGNED,
        }
    }

    pub fn set(style: &mut DimStyle, choice: usize) {
        let (inside, outside) = match choice {
            HORIZONTAL => (true, true),
            ISO => (false, true),
            _ => (false, false),
        };
        style.inside_horizontal = inside;
        style.outside_horizontal = outside;
    }
}

/// `DIMPOST` holds the prefix and suffix as `prefix<>suffix`; with no `<>` it
/// is all suffix.
pub(crate) fn split_post(post: &str) -> (String, String) {
    match post.split_once("<>") {
        Some((prefix, suffix)) => (prefix.to_owned(), suffix.to_owned()),
        None => (String::new(), post.to_owned()),
    }
}

pub(crate) fn join_post(prefix: &str, suffix: &str) -> String {
    if prefix.is_empty() && suffix.is_empty() {
        String::new()
    } else {
        format!("{prefix}<>{suffix}")
    }
}

#[derive(Clone, Debug)]
pub struct Draft {
    pub style: DimStyle,
    /// The style as the dialog opened, which sizes the preview's drawing
    /// so that a change shows up against it.
    pub original: DimStyle,
    pub tab: usize,
    pub prefix: String,
    pub suffix: String,
    /// The "..." beside Text style was pressed: the app opens the Text
    /// Style dialog and clears this.
    pub want_text_styles: bool,
}

impl Draft {
    pub fn new(style: &DimStyle) -> Self {
        let (prefix, suffix) = split_post(&style.post);
        Self {
            style: style.clone(),
            original: style.clone(),
            tab: 0,
            prefix,
            suffix,
            want_text_styles: false,
        }
    }

    /// Put the typed prefix and suffix into the style.
    pub fn sync_post(&mut self) {
        self.style.post = join_post(&self.prefix, &self.suffix);
    }
}

/// The dimensions the preview shows, on a part drawn in drawing units sized
/// from `reference` (a style's text height times its scale), with the
/// part's outline. All of it is `style`'s doing apart from the outline.
pub fn sample(style: &DimStyle, reference: f64) -> Vec<Entity> {
    let h = reference.max(1e-6);
    let (w, d) = (28.0 * h, 18.0 * h);
    let p = Point::new;
    let corner = p(w, 0.55 * d);
    let top = p(0.72 * w, d);
    let hole = p(0.3 * w, 0.42 * d);
    let radius = 0.24 * d;
    let outline = Entity::Polyline {
        points: vec![p(0.0, 0.0), p(w, 0.0), corner, top, p(0.0, d)],
        closed: true,
        bulges: Vec::new(),
        widths: Vec::new(),
    };
    let on_hole = p(
        hole.x + radius * std::f64::consts::FRAC_1_SQRT_2,
        hole.y - radius * std::f64::consts::FRAC_1_SQRT_2,
    );
    let mut all = vec![
        outline,
        Entity::Circle {
            center: hole,
            radius,
        },
    ];
    let dims = [
        Dimension::linear(p(0.0, d), top, p(0.0, d + 3.4 * h), 0.0, style),
        Dimension::linear(
            p(0.0, 0.0),
            p(0.0, d),
            p(-3.4 * h, 0.0),
            std::f64::consts::FRAC_PI_2,
            style,
        ),
        Dimension::aligned(
            corner,
            top,
            p(corner.x + 3.0 * h, corner.y + 0.6 * h),
            style,
        ),
        Dimension::radius(hole, on_hole, style),
    ];
    for dimension in dims {
        all.extend(dimension.picture());
    }
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_alignments_round_trip_through_the_flags() {
        let mut style = DimStyle::default();
        assert_eq!(alignment::of(&style), alignment::ISO, "ISO-25 is the stock");
        for choice in [alignment::HORIZONTAL, alignment::ALIGNED, alignment::ISO] {
            alignment::set(&mut style, choice);
            assert_eq!(alignment::of(&style), choice);
        }
    }

    #[test]
    fn prefix_and_suffix_split_and_join_the_post() {
        assert_eq!(split_post(""), (String::new(), String::new()));
        assert_eq!(split_post("mm"), (String::new(), "mm".to_owned()));
        assert_eq!(split_post("~<> mm"), ("~".to_owned(), " mm".to_owned()));
        assert_eq!(join_post("", ""), "");
        assert_eq!(join_post("~", " mm"), "~<> mm");
        let style = DimStyle {
            post: "<> mm".to_owned(),
            ..DimStyle::default()
        };
        let mut draft = Draft::new(&style);
        draft.sync_post();
        assert_eq!(draft.style.post, "<> mm", "typing nothing changes nothing");
    }

    #[test]
    fn the_sample_has_every_kind_of_dimension_with_words() {
        let all = sample(&DimStyle::default(), 2.5);
        let texts = all.iter().filter(|e| matches!(e, Entity::MText(_))).count();
        assert_eq!(texts, 4);
        assert!(all.len() > 20);
        assert!(all.iter().all(|e| {
            let b = e.bounds();
            b.min.x.is_finite() && b.max.y.is_finite()
        }));
    }

    #[test]
    fn a_style_change_changes_the_sample() {
        let plain = sample(&DimStyle::default(), 2.5);
        let ticks = sample(
            &DimStyle {
                arrow: od_core::Arrow::Tick,
                ..DimStyle::default()
            },
            2.5,
        );
        assert_ne!(plain, ticks);
    }
}
