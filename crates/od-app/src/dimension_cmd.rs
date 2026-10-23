//! DIMALIGNED, DIMLINEAR, DIMRADIUS, DIMDIAMETER and DIMARC, written against
//! the `Editor`. Each makes one dimension in the current dimension style; the
//! geometry (and what the points mean) is `od_core::dimension`.

use std::f64::consts::FRAC_PI_2;
use std::sync::Arc;

use od_commands::{Answer, Cancelled, Editor, Options, Picked, PointOptions};
use od_core::dimension::{auto_rotation, object_ends};
use od_core::{DimStyle, DimText, Dimension, Entity, Point};

use crate::draw::{add, distance, MIN_SIZE};

/// What the user has changed from the defaults while placing a dimension.
#[derive(Clone, Default)]
pub(crate) struct Tweaks {
    pub text: DimText,
    /// A dimension line angle chosen with Horizontal, Vertical or Rotated.
    rotation: Option<f64>,
    /// DIMARC: the leader option, and the two points of a partial arc.
    leader: bool,
    partial: Option<(Point, Point)>,
}

pub(crate) type Make = Arc<dyn Fn(Point, &Tweaks) -> Option<Dimension> + Send + Sync>;

pub(crate) fn style(ed: &Editor) -> Result<DimStyle, Cancelled> {
    ed.read(|doc| doc.dim_style().clone())
}

/// Click an object; `None` when the user ends the command with Enter.
pub(crate) fn pick(ed: &Editor, message: &str) -> Result<Option<(Picked, Entity)>, Cancelled> {
    let Answer::Value(picked) = ed.ask_object(Options::<Picked>::new(message))? else {
        return Ok(None);
    };
    let id = picked.id;
    let entity = ed.read(move |doc| doc.get(id).cloned())?;
    Ok(entity.map(|e| (picked, e)))
}

/// Ask where the dimension goes, showing it follow the cursor, with the
/// given keywords on offer. `sample` is any point, used to word the Text
/// prompt with the measurement.
pub(crate) fn place(
    ed: &Editor,
    message: &str,
    keywords: &[(&str, &str)],
    sample: Point,
    make: Make,
) -> Result<(), Cancelled> {
    let mut tweaks = Tweaks::default();
    loop {
        let (shown, current) = (make.clone(), tweaks.clone());
        let mut options = PointOptions::new(message).ghost(move |at| {
            shown(at, &current)
                .map(|d| Entity::Dimension(Box::new(d)))
                .into_iter()
                .collect()
        });
        for (label, key) in keywords {
            options = options.keyword(*label, *key);
        }
        match ed.ask_point(options)? {
            Answer::Value(at) => {
                let Some(dimension) = make(at, &tweaks) else {
                    ed.say("Those points make no dimension.");
                    continue;
                };
                return add(ed, Entity::Dimension(Box::new(dimension)), "Dimension");
            }
            Answer::Keyword(k) => match k.as_str() {
                "Text" => tweaks.text = ask_text(ed, &make, sample, &tweaks)?,
                "Horizontal" => tweaks.rotation = Some(0.0),
                "Vertical" => tweaks.rotation = Some(FRAC_PI_2),
                "Leader" => tweaks.leader = true,
                "Partial" => {
                    let first = ed.get_point(PointOptions::new(
                        "Specify first point for arc length dimension",
                    ))?;
                    let second = ed.get_point(
                        PointOptions::new("Specify second point for arc length dimension")
                            .base(first),
                    )?;
                    tweaks.partial = Some((first, second));
                }
                "Rotated" => {
                    tweaks.rotation = Some(ed.get_angle("Specify angle of dimension line")?);
                }
                _ => {}
            },
            Answer::Enter => return Err(Cancelled),
        }
    }
}

/// The Text option: Enter keeps the measurement, `<>` stands for it.
fn ask_text(
    ed: &Editor,
    make: &Make,
    sample: Point,
    tweaks: &Tweaks,
) -> Result<DimText, Cancelled> {
    let measured = make(sample, &Tweaks::default()).map(|d| d.label());
    let message = match &measured {
        Some(m) => format!("Enter dimension text <{m}>"),
        None => "Enter dimension text".to_owned(),
    };
    Ok(match ed.ask_string(Options::<String>::new(message))? {
        Answer::Value(typed) if measured.as_deref() != Some(typed.as_str()) => {
            DimText::from_entry(&typed)
        }
        Answer::Value(_) => DimText::Measured,
        _ => tweaks.text.clone(),
    })
}

/// DIMALIGNED and DIMLINEAR: two extension line origins (or an object), then
/// where the dimension line goes. A linear dimension runs horizontally or
/// vertically, whichever way the cursor is, unless told otherwise.
fn linear(ed: &mut Editor, aligned: bool) -> Result<(), Cancelled> {
    let style = style(ed)?;
    let options = PointOptions::new("Specify first extension line origin or <select object>");
    let (p1, p2) = match ed.ask_point(options)? {
        Answer::Value(p1) => {
            let second = PointOptions::new("Specify second extension line origin").base(p1);
            (p1, ed.get_point(second)?)
        }
        Answer::Enter => loop {
            let Some((picked, entity)) = pick(ed, "Select object to dimension")? else {
                return Err(Cancelled);
            };
            match object_ends(&entity, picked.at) {
                Some(ends) => break ends,
                None => ed.say("Select a line, an arc or a straight segment."),
            }
        },
        Answer::Keyword(_) => return Err(Cancelled),
    };
    place_linear(ed, style, (p1, p2), aligned)
}

/// The dimension line of a linear or aligned dimension between two origins.
fn place_linear(
    ed: &mut Editor,
    style: DimStyle,
    (p1, p2): (Point, Point),
    aligned: bool,
) -> Result<(), Cancelled> {
    if distance(p1, p2) < MIN_SIZE {
        ed.say("The extension line origins are the same point.");
        return Ok(());
    }
    let make: Make = Arc::new(move |at, tweaks| {
        let mut dimension = if aligned {
            Dimension::aligned(p1, p2, at, &style)
        } else {
            let rotation = tweaks.rotation.unwrap_or_else(|| auto_rotation(p1, p2, at));
            Dimension::linear(p1, p2, at, rotation, &style)
        };
        dimension.text = tweaks.text.clone();
        dimension.measurement().map(|_| dimension)
    });
    let keywords: &[(&str, &str)] = if aligned {
        &[("Text", "T")]
    } else {
        &[
            ("Text", "T"),
            ("Horizontal", "H"),
            ("Vertical", "V"),
            ("Rotated", "R"),
        ]
    };
    let sample = Point::new((p1.x + p2.x) / 2.0, (p1.y + p2.y) / 2.0);
    place(
        ed,
        "Specify dimension line location",
        keywords,
        sample,
        make,
    )
}

pub fn dim_aligned(ed: &mut Editor) -> Result<(), Cancelled> {
    linear(ed, true)
}

pub fn dim_linear(ed: &mut Editor) -> Result<(), Cancelled> {
    linear(ed, false)
}

/// DIMRADIUS and DIMDIAMETER: pick an arc or circle, then where the text goes.
fn radial(ed: &mut Editor, diameter: bool) -> Result<(), Cancelled> {
    let style = style(ed)?;
    let (center, radius) = loop {
        let Some((_, entity)) = pick(ed, "Select arc or circle")? else {
            return Err(Cancelled);
        };
        match entity {
            Entity::Circle { center, radius } | Entity::Arc { center, radius, .. } => {
                break (center, radius.abs());
            }
            _ => ed.say("Select an arc or a circle."),
        }
    };
    place_radial(ed, style, (center, radius), diameter)
}

/// The text position of a radius or diameter dimension of a circle.
fn place_radial(
    ed: &mut Editor,
    style: DimStyle,
    (center, radius): (Point, f64),
    diameter: bool,
) -> Result<(), Cancelled> {
    let make: Make = Arc::new(move |at, tweaks| {
        let mut dimension = Dimension::radial_toward(center, radius, at, diameter, &style)?;
        dimension.text = tweaks.text.clone();
        Some(dimension)
    });
    let sample = Point::new(center.x + radius, center.y);
    place(
        ed,
        "Specify dimension line location",
        &[("Text", "T")],
        sample,
        make,
    )
}

pub fn dim_radius(ed: &mut Editor) -> Result<(), Cancelled> {
    radial(ed, false)
}

pub fn dim_diameter(ed: &mut Editor) -> Result<(), Cancelled> {
    radial(ed, true)
}

/// DIM: pick an object and get the fitting dimension: a radius for a circle or
/// an arc, a linear dimension for a line or a straight segment. Enter starts
/// a linear dimension from two points instead.
pub fn dim_auto(ed: &mut Editor) -> Result<(), Cancelled> {
    loop {
        let Some((picked, entity)) = pick(ed, "Select object to dimension or <specify points>")?
        else {
            return linear(ed, false);
        };
        let style = style(ed)?;
        match &entity {
            Entity::Circle { center, radius } | Entity::Arc { center, radius, .. } => {
                return place_radial(ed, style, (*center, radius.abs()), false);
            }
            _ => match object_ends(&entity, picked.at) {
                Some(ends) => return place_linear(ed, style, ends, false),
                None => ed.say("Select a line, a circle, an arc or a straight segment."),
            },
        }
    }
}

/// The point of the arc through `on` (any point of it) in the direction of
/// `toward` from `center`.
fn on_arc(center: Point, on: Point, toward: Point) -> Point {
    let radius = distance(center, on);
    let (dx, dy) = (toward.x - center.x, toward.y - center.y);
    let length = dx.hypot(dy);
    if length < MIN_SIZE {
        return on;
    }
    Point::new(
        center.x + dx / length * radius,
        center.y + dy / length * radius,
    )
}

/// DIMARC: pick an arc, then where the dimension arc goes.
pub fn dim_arc(ed: &mut Editor) -> Result<(), Cancelled> {
    let style = style(ed)?;
    let (center, start, end) = loop {
        let Some((_, entity)) = pick(ed, "Select arc")? else {
            return Err(Cancelled);
        };
        match entity {
            Entity::Arc {
                center,
                radius,
                start,
                end,
            } => {
                let at = |angle: f64| od_core::arc::point_at(center, radius, angle);
                break (center, at(start), at(end));
            }
            _ => ed.say("Select an arc."),
        }
    };
    let make: Make = Arc::new(move |at, tweaks| {
        let (from, to) = match tweaks.partial {
            Some((a, b)) => (on_arc(center, start, a), on_arc(center, start, b)),
            None => (start, end),
        };
        let mut dimension = Dimension::arc_length(center, from, to, at, &style);
        if let od_core::DimKind::ArcLength { leader, .. } = &mut dimension.kind {
            *leader = tweaks.leader;
        }
        dimension.text = tweaks.text.clone();
        dimension.measurement().map(|_| dimension)
    });
    place(
        ed,
        "Specify arc length dimension location",
        &[("Text", "T"), ("Partial", "P"), ("Leader", "L")],
        start,
        make,
    )
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;
    use crate::state::AppState;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn dimensions(state: &AppState) -> Vec<Dimension> {
        state
            .session()
            .document
            .objects()
            .iter()
            .filter_map(|o| match &o.entity {
                Entity::Dimension(d) => Some((**d).clone()),
                _ => None,
            })
            .collect()
    }

    fn started(
        entities: &[Entity],
        name: &'static str,
        run: fn(&mut Editor) -> Result<(), Cancelled>,
    ) -> AppState {
        let mut state = AppState::new();
        for e in entities {
            state.add_entity(e.clone());
        }
        state.start_command(name, run);
        state.settle_command();
        state
    }

    fn submit(state: &mut AppState, text: &str) {
        state.cmd_submit(text);
        state.settle_command();
    }

    fn click(state: &mut AppState, index: usize, at: Point) {
        let id = state.session().document().objects()[index].id;
        state.answer_object(at, Some(id), false);
        state.settle_command();
    }

    #[test]
    fn dim_on_a_circle_makes_a_radius_dimension() {
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 5.0,
        };
        let mut state = started(&[circle], "DIM", dim_auto);
        click(&mut state, 0, p(5.0, 0.0));
        submit(&mut state, "8,0");
        assert_eq!(dimensions(&state).len(), 1);
        assert_eq!(dimensions(&state)[0].measurement(), Some(5.0));
    }

    #[test]
    fn aligned_takes_two_origins_and_a_location() {
        let mut state = started(&[], "DIMALIGNED", dim_aligned);
        submit(&mut state, "0,0");
        submit(&mut state, "30,40");
        submit(&mut state, "10,10");
        let all = dimensions(&state);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].measurement(), Some(50.0));
        assert!(!state.command_running());
        state.undo();
        assert!(dimensions(&state).is_empty(), "one undo step");
    }

    #[test]
    fn linear_runs_horizontally_when_the_line_is_above() {
        let mut state = started(&[], "DIMLINEAR", dim_linear);
        submit(&mut state, "0,0");
        submit(&mut state, "30,40");
        submit(&mut state, "15,60");
        assert_eq!(dimensions(&state)[0].measurement(), Some(30.0));
    }

    #[test]
    fn linear_runs_vertically_when_the_line_is_beside() {
        let mut state = started(&[], "DIMLINEAR", dim_linear);
        submit(&mut state, "0,0");
        submit(&mut state, "30,40");
        submit(&mut state, "60,20");
        assert_eq!(dimensions(&state)[0].measurement(), Some(40.0));
    }

    #[test]
    fn the_rotated_option_sets_the_angle() {
        let mut state = started(&[], "DIMLINEAR", dim_linear);
        submit(&mut state, "0,0");
        submit(&mut state, "30,40");
        submit(&mut state, "r");
        submit(&mut state, "0");
        submit(&mut state, "60,20");
        assert_eq!(dimensions(&state)[0].measurement(), Some(30.0));
    }

    #[test]
    fn enter_dimensions_an_object() {
        let line = Entity::Line {
            start: p(0.0, 0.0),
            end: p(25.0, 0.0),
        };
        let mut state = started(&[line], "DIMLINEAR", dim_linear);
        submit(&mut state, "");
        click(&mut state, 0, p(12.0, 0.0));
        submit(&mut state, "12,10");
        assert_eq!(dimensions(&state)[0].measurement(), Some(25.0));
    }

    #[test]
    fn the_text_option_replaces_the_measurement() {
        let mut state = started(&[], "DIMALIGNED", dim_aligned);
        submit(&mut state, "0,0");
        submit(&mut state, "10,0");
        submit(&mut state, "t");
        assert!(state.cmd_prompt().text.contains("<10.00>"));
        submit(&mut state, "<> TYP");
        submit(&mut state, "5,5");
        assert_eq!(dimensions(&state)[0].label(), "10.00 TYP");
    }

    #[test]
    fn radius_and_diameter_pick_a_circle() {
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 10.0,
        };
        let mut state = started(&[circle], "DIMRADIUS", dim_radius);
        click(&mut state, 0, p(10.0, 0.0));
        submit(&mut state, "5,5");
        assert_eq!(dimensions(&state)[0].label(), "R10.00");
        state.start_command("DIMDIAMETER", dim_diameter);
        state.settle_command();
        click(&mut state, 0, p(10.0, 0.0));
        submit(&mut state, "30,0");
        assert_eq!(dimensions(&state)[1].label(), "Ø20.00");
    }

    #[test]
    fn a_line_is_not_a_circle() {
        let line = Entity::Line {
            start: p(0.0, 0.0),
            end: p(5.0, 0.0),
        };
        let mut state = started(&[line], "DIMRADIUS", dim_radius);
        click(&mut state, 0, p(2.0, 0.0));
        assert!(state
            .cmdline
            .log
            .last()
            .unwrap()
            .contains("arc or a circle"));
        assert!(state.command_running());
    }

    #[test]
    fn arc_length_dimensions_an_arc() {
        let arc = Entity::Arc {
            center: p(0.0, 0.0),
            radius: 10.0,
            start: 0.0,
            end: FRAC_PI_2,
        };
        let mut state = started(&[arc], "DIMARC", dim_arc);
        click(&mut state, 0, p(7.0, 7.0));
        submit(&mut state, "12,12");
        let length = dimensions(&state)[0].measurement().unwrap();
        assert!((length - 10.0 * FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn partial_measures_between_two_points_of_the_arc() {
        let arc = Entity::Arc {
            center: p(0.0, 0.0),
            radius: 10.0,
            start: 0.0,
            end: PI,
        };
        let mut state = started(&[arc], "DIMARC", dim_arc);
        click(&mut state, 0, p(0.0, 10.0));
        submit(&mut state, "p");
        submit(&mut state, "10,0");
        submit(&mut state, "0,10");
        submit(&mut state, "l");
        submit(&mut state, "12,12");
        let d = &dimensions(&state)[0];
        assert!((d.measurement().unwrap() - 10.0 * FRAC_PI_2).abs() < 1e-9);
        assert!(matches!(
            d.kind,
            od_core::DimKind::ArcLength { leader: true, .. }
        ));
    }

    #[test]
    fn dimensions_use_the_current_style() {
        let mut state = AppState::new();
        state.set_dim_style(DimStyle {
            precision: 0,
            ..DimStyle::default()
        });
        state.start_command("DIMALIGNED", dim_aligned);
        state.settle_command();
        submit(&mut state, "0,0");
        submit(&mut state, "10.4,0");
        submit(&mut state, "5,5");
        assert_eq!(dimensions(&state)[0].label(), "10");
    }

    #[test]
    fn the_same_point_twice_makes_nothing() {
        let mut state = started(&[], "DIMALIGNED", dim_aligned);
        submit(&mut state, "3,3");
        submit(&mut state, "3,3");
        assert!(dimensions(&state).is_empty());
        assert!(!state.command_running());
    }
}
