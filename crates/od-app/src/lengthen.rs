//! LENGTHEN: change the length of lines, arcs and open polylines, by an
//! amount, a percentage, a total or by dragging. The geometry is
//! `Entity::lengthened` and its relatives in `od_core`.

use od_commands::{Answer, Cancelled, Editor, NumberOptions, Options, Picked, PointOptions};
use od_core::units::{format_angle, format_length};
use od_core::{End, Entity};

/// What to do to the end of each object that is picked.
#[derive(Clone, Copy)]
enum Mode {
    Delta(f64),
    DeltaAngle(f64),
    /// Of the current length: 100 leaves it as it is.
    Percent(f64),
    Total(f64),
    TotalAngle(f64),
    /// The end follows a point the user gives.
    Dynamic,
}

impl Mode {
    /// The object with its `end` changed, if that can be done by this mode
    /// (not for dragging, which waits for its point).
    fn apply(self, target: &Entity, end: End) -> Option<Entity> {
        match self {
            Mode::Delta(d) => target.lengthened(end, d),
            Mode::DeltaAngle(a) => target.lengthened_by_angle(end, a),
            Mode::Percent(p) => target.lengthened(end, target.curve_length()? * (p / 100.0 - 1.0)),
            Mode::Total(t) => target.lengthened(end, t - target.curve_length()?),
            Mode::TotalAngle(t) => target.lengthened_by_angle(end, t - target.included_angle()?),
            Mode::Dynamic => None,
        }
    }
}

/// Why an object cannot be lengthened at all, if it cannot.
fn refusal(target: &Entity) -> Option<&'static str> {
    match target {
        Entity::Ellipse(_) => Some("LENGTHEN does not support ellipses yet."),
        Entity::Spline(_) => Some("LENGTHEN does not support splines yet."),
        other if other.curve_length().is_none() => Some("Object cannot be lengthened."),
        _ => None,
    }
}

/// LENGTHEN: click an object to be told its length, or pick DElta, Percent,
/// Total or DYnamic and then click the objects to change, near the end that
/// should move. Each change is one undo step; Enter ends.
pub fn lengthen_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    loop {
        let options = Options::<Picked>::new("Select an object or [DElta/Percent/Total/DYnamic]")
            .keyword("DElta", "DE")
            .keyword("Percent", "P")
            .keyword("Total", "T")
            .keyword("DYnamic", "DY")
            .allow_empty();
        match ed.ask_object(options)? {
            Answer::Value(Picked { id, .. }) => report(ed, id)?,
            Answer::Keyword(k) => {
                let Some(mode) = ask_mode(ed, &k)? else {
                    continue;
                };
                return change_objects(ed, mode);
            }
            Answer::Enter => return Ok(()),
        }
    }
}

/// Say how long an object is (and how far an arc turns).
fn report(ed: &mut Editor, id: od_core::EntityId) -> Result<(), Cancelled> {
    let found = ed.read(move |doc| {
        let target = doc.get(id)?;
        Some((
            *doc.units(),
            refusal(target),
            target.curve_length(),
            target.included_angle(),
        ))
    })?;
    match found {
        Some((_, Some(why), ..)) => ed.say(why),
        Some((units, None, Some(length), angle)) => {
            let mut text = format!("Current length: {}", format_length(&units, length));
            if let Some(angle) = angle {
                text += &format!(", included angle: {}", format_angle(&units, angle));
            }
            ed.say(text);
        }
        _ => {}
    }
    Ok(())
}

/// Ask for the amount the chosen keyword needs; `None` to go back.
fn ask_mode(ed: &mut Editor, keyword: &str) -> Result<Option<Mode>, Cancelled> {
    Ok(match keyword {
        "DElta" => match ask_amount(ed, "Enter delta length", true)? {
            Amount::Length(d) => Some(Mode::Delta(d)),
            Amount::Angle(a) => Some(Mode::DeltaAngle(a)),
            Amount::Back => None,
        },
        "Total" => match ask_amount(ed, "Specify total length", false)? {
            Amount::Length(t) => Some(Mode::Total(t)),
            Amount::Angle(t) => Some(Mode::TotalAngle(t)),
            Amount::Back => None,
        },
        "Percent" => {
            let options = NumberOptions::new("Enter percentage length").default(100.0);
            match ed.ask_number(options)? {
                Answer::Value(p) if p > 0.0 => Some(Mode::Percent(p)),
                Answer::Value(_) => {
                    ed.say("Requires a positive value.");
                    None
                }
                _ => None,
            }
        }
        _ => Some(Mode::Dynamic),
    })
}

enum Amount {
    Length(f64),
    Angle(f64),
    Back,
}

/// A length, or (by the Angle keyword) an angle. `signed`: may be negative
/// (a change); otherwise it must be positive (a total).
fn ask_amount(ed: &mut Editor, message: &str, signed: bool) -> Result<Amount, Cancelled> {
    let options = NumberOptions::new(format!("{message} or [Angle]")).keyword("Angle", "A");
    let ok = |v: f64| signed || v > 0.0;
    Ok(match ed.ask_distance(options)? {
        Answer::Value(v) if ok(v) => Amount::Length(v),
        Answer::Value(_) => {
            ed.say("Requires a positive value.");
            Amount::Back
        }
        Answer::Keyword(_) => {
            let what = if signed { "delta" } else { "total" };
            match ed.ask_angle(NumberOptions::new(format!("Enter {what} angle")))? {
                Answer::Value(a) if ok(a) => Amount::Angle(a),
                Answer::Value(_) => {
                    ed.say("Requires a positive value.");
                    Amount::Back
                }
                _ => Amount::Back,
            }
        }
        Answer::Enter => Amount::Back,
    })
}

/// Pick objects one after another and change each by `mode`.
fn change_objects(ed: &mut Editor, mode: Mode) -> Result<(), Cancelled> {
    loop {
        let options = Options::<Picked>::new("Select an object to change or [Undo]")
            .keyword("Undo", "U")
            .allow_empty();
        let Picked { id, at, .. } = match ed.ask_object(options)? {
            Answer::Value(picked) => picked,
            Answer::Keyword(_) => {
                ed.undo()?;
                continue;
            }
            Answer::Enter => return Ok(()),
        };
        let Some(target) = ed.read(move |doc| doc.get(id).cloned())? else {
            continue;
        };
        if let Some(why) = refusal(&target) {
            ed.say(why);
            continue;
        }
        let Some(end) = target.nearer_end(at) else {
            continue;
        };
        let changed = match mode {
            Mode::Dynamic => {
                let shown = target.clone();
                let options = PointOptions::new("Specify new end point")
                    .ghost(move |p| shown.lengthened_to(end, p).into_iter().collect());
                let Answer::Value(p) = ed.ask_point(options)? else {
                    continue;
                };
                target.lengthened_to(end, p)
            }
            fixed => fixed.apply(&target, end),
        };
        let Some(changed) = changed else {
            ed.say("Cannot lengthen the object that way.");
            continue;
        };
        ed.transaction("Lengthen", move |tx| {
            tx.modify(id, move |_| changed);
            Ok(())
        })?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use od_core::Point;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(a: Point, b: Point) -> Entity {
        Entity::Line { start: a, end: b }
    }

    fn entities(state: &AppState) -> Vec<Entity> {
        let objects = state.session().document().objects();
        objects.iter().map(|o| o.entity.clone()).collect()
    }

    fn lengthening(entities: &[Entity]) -> AppState {
        let mut state = AppState::new();
        for e in entities {
            state.add_entity(e.clone());
        }
        state.start_command("Lengthen", lengthen_objects);
        state.settle_command();
        state
    }

    fn say(state: &mut AppState, text: &str) {
        state.cmd_submit(text);
        state.settle_command();
    }

    fn click(state: &mut AppState, index: usize, at: Point) {
        let id = state.session().document().objects()[index].id;
        state.answer_object(at, Some(id), false);
        state.settle_command();
    }

    fn said(state: &AppState, text: &str) -> bool {
        state.cmdline.log.iter().any(|l| l.contains(text))
    }

    #[test]
    fn clicking_an_object_reports_its_length() {
        let mut state = lengthening(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        click(&mut state, 0, p(5.0, 0.0));
        assert!(said(&state, "Current length: 10"));
        assert!(state.command_running(), "and asks again");
    }

    #[test]
    fn delta_changes_the_end_nearest_the_click_each_time_and_undoes_one_step_at_a_time() {
        let mut state = lengthening(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        say(&mut state, "de");
        say(&mut state, "2");
        click(&mut state, 0, p(9.0, 1.0));
        assert_eq!(entities(&state), [line(p(0.0, 0.0), p(12.0, 0.0))]);
        click(&mut state, 0, p(1.0, 1.0));
        assert_eq!(entities(&state), [line(p(-2.0, 0.0), p(12.0, 0.0))]);
        say(&mut state, "u");
        assert_eq!(entities(&state), [line(p(0.0, 0.0), p(12.0, 0.0))]);
        say(&mut state, "");
        assert!(!state.command_running());
        state.undo();
        assert_eq!(entities(&state), [line(p(0.0, 0.0), p(10.0, 0.0))]);
    }

    #[test]
    fn a_negative_delta_shortens_and_too_much_is_refused() {
        let mut state = lengthening(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        say(&mut state, "de");
        say(&mut state, "-4");
        click(&mut state, 0, p(9.0, 0.0));
        assert_eq!(entities(&state), [line(p(0.0, 0.0), p(6.0, 0.0))]);
        say(&mut state, "");
        let mut state = lengthening(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        say(&mut state, "de");
        say(&mut state, "-10");
        click(&mut state, 0, p(9.0, 0.0));
        assert_eq!(entities(&state), [line(p(0.0, 0.0), p(10.0, 0.0))]);
        assert!(said(&state, "Cannot lengthen the object that way."));
    }

    #[test]
    fn percent_and_total_set_the_length() {
        let mut state = lengthening(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        say(&mut state, "p");
        say(&mut state, "150");
        click(&mut state, 0, p(9.0, 0.0));
        assert_eq!(entities(&state), [line(p(0.0, 0.0), p(15.0, 0.0))]);
        say(&mut state, "");

        let mut state = lengthening(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        say(&mut state, "t");
        say(&mut state, "4");
        click(&mut state, 0, p(1.0, 0.0));
        assert_eq!(entities(&state), [line(p(6.0, 0.0), p(10.0, 0.0))]);
    }

    #[test]
    fn the_angle_option_turns_an_arc_and_dynamic_follows_a_point() {
        let arc = Entity::Arc {
            center: p(0.0, 0.0),
            radius: 2.0,
            start: 0.0,
            end: std::f64::consts::FRAC_PI_2,
        };
        let mut state = lengthening(&[arc]);
        say(&mut state, "de");
        say(&mut state, "a");
        say(&mut state, "90");
        click(&mut state, 0, p(2.0, 0.1));
        let Entity::Arc { start, end, .. } = entities(&state)[0] else {
            panic!("an arc")
        };
        assert!(
            (start - 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-9,
            "{start}"
        );
        assert!((end - std::f64::consts::FRAC_PI_2).abs() < 1e-9);

        let mut state = lengthening(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        say(&mut state, "dy");
        click(&mut state, 0, p(9.0, 0.0));
        say(&mut state, "14,5");
        assert_eq!(entities(&state), [line(p(0.0, 0.0), p(14.0, 0.0))]);
    }

    #[test]
    fn closed_and_unsupported_objects_are_named() {
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 1.0,
        };
        let mut state = lengthening(&[circle]);
        say(&mut state, "de");
        say(&mut state, "1");
        click(&mut state, 0, p(1.0, 0.0));
        assert!(said(&state, "Object cannot be lengthened."));
    }
}
