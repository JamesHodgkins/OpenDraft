//! SCALE and ROTATE: the plain form (a factor or angle, typed or picked) plus
//! the Copy and Reference options of AutoCAD.

use od_commands::{Answer, Cancelled, Editor, NumberOptions, NumberOrPoint, Options, PointOptions};
use od_core::{Entity, Object, Point};

use crate::modify::{entities_of, objects_of};
use crate::text::plural;
use crate::units_state::angle_message;

/// Scale factors at or below this are refused.
const MIN_FACTOR: f64 = 1e-9;

fn distance(a: Point, b: Point) -> f64 {
    (b.x - a.x).hypot(b.y - a.y)
}

fn direction(a: Point, b: Point) -> f64 {
    (b.y - a.y).atan2(b.x - a.x)
}

/// Apply `change` to the objects, or to copies of them when `copy` is set.
fn apply(
    ed: &Editor,
    name: &'static str,
    objects: Vec<Object>,
    copy: bool,
    change: impl Fn(&Entity) -> Entity + Send + 'static,
) -> Result<(), Cancelled> {
    ed.transaction(name, move |tx| {
        if copy {
            tx.add_with(
                objects
                    .iter()
                    .map(|o| (change(&o.entity), o.attrs.clone()))
                    .collect(),
            );
        } else {
            for o in &objects {
                tx.modify(o.id, |e| change(e));
            }
        }
        Ok(())
    })
}

/// The factor that takes a reference length to a new one: both are typed,
/// picked from the base point, or (new length) given by two points.
fn reference_factor(ed: &Editor, base: Point, originals: &[Entity]) -> Result<f64, Cancelled> {
    let reference = loop {
        let options = NumberOptions::new("Specify reference length").default(1.0);
        let length = match ed.ask_distance_or_point(options)? {
            Answer::Value(NumberOrPoint::Number(length)) => length,
            Answer::Value(NumberOrPoint::Point(first)) => {
                let options = PointOptions::new("Specify second point").base(first);
                distance(first, ed.get_point(options)?)
            }
            _ => return Err(Cancelled),
        };
        if length > MIN_FACTOR {
            break length;
        }
        ed.say("Requires a positive value.");
    };
    loop {
        let shown = originals.to_vec();
        let options = NumberOptions::new("Specify new length")
            .base(base)
            .keyword("Points", "P")
            .ghost(move |at| {
                let factor = distance(base, at) / reference;
                if factor <= MIN_FACTOR {
                    return Vec::new();
                }
                shown.iter().map(|e| e.scaled(base, factor)).collect()
            });
        let length = match ed.ask_distance(options)? {
            Answer::Value(length) => length,
            Answer::Keyword(_) => {
                let first = ed.get_point("Specify first point")?;
                let second = ed.get_point(PointOptions::new("Specify second point").base(first))?;
                distance(first, second)
            }
            Answer::Enter => return Err(Cancelled),
        };
        if length > MIN_FACTOR {
            return Ok(length / reference);
        }
        ed.say("Requires a positive value.");
    }
}

/// SCALE: enlarge or shrink objects about a base point by a typed factor, by
/// the distance from the base point to a picked point (relative to 1), or by
/// a reference length and a new one. Copy keeps the originals.
pub fn scale_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    let ids = ed.pickset_or_select("Select objects")?;
    let objects = objects_of(ed, &ids)?;
    let originals = entities_of(ed, &ids)?;
    if let Some(why) = originals.iter().find_map(Entity::scale_refusal) {
        ed.say(why);
        return Ok(());
    }
    let base = ed.get_point("Specify base point")?;
    let mut copy = false;
    let factor = loop {
        let shown = originals.clone();
        let options = Options::new("Specify scale factor")
            .base(base)
            .keyword("Copy", "C")
            .keyword("Reference", "R")
            .ghost(move |at| {
                let factor = distance(base, at);
                if factor <= MIN_FACTOR {
                    return Vec::new();
                }
                shown.iter().map(|e| e.scaled(base, factor)).collect()
            });
        let factor = match ed.ask_distance(options)? {
            Answer::Value(factor) => factor,
            Answer::Keyword(k) if k == "Copy" => {
                copy = true;
                ed.say("Scale a copy of the selected objects.");
                continue;
            }
            Answer::Keyword(_) => reference_factor(ed, base, &originals)?,
            Answer::Enter => return Err(Cancelled),
        };
        if factor > MIN_FACTOR {
            break factor;
        }
        ed.say("Requires a positive value.");
    };
    let count = plural(objects.len(), "object");
    apply(ed, "Scale", objects, copy, move |e| e.scaled(base, factor))?;
    ed.say(format!("Scaled {count} by {factor}"));
    Ok(())
}

/// The angle that takes a reference direction to a new one. The reference
/// angle defaults to 0; the new one is typed, picked from the base point or
/// given by two points.
fn reference_angle(ed: &Editor, base: Point, originals: &[Entity]) -> Result<f64, Cancelled> {
    let options = NumberOptions::new("Specify the reference angle").default(0.0);
    let reference = match ed.ask_angle_or_point(options)? {
        Answer::Value(NumberOrPoint::Number(angle)) => angle,
        Answer::Value(NumberOrPoint::Point(first)) => {
            let options = PointOptions::new("Specify second point").base(first);
            direction(first, ed.get_point(options)?)
        }
        _ => return Err(Cancelled),
    };
    let shown = originals.to_vec();
    let options = NumberOptions::new("Specify new angle")
        .base(base)
        .keyword("Points", "P")
        .ghost(move |at| {
            let turn = direction(base, at) - reference;
            shown.iter().map(|e| e.rotated(base, turn)).collect()
        });
    let new = match ed.ask_angle(options)? {
        Answer::Value(angle) => angle,
        Answer::Keyword(_) => {
            let first = ed.get_point("Specify first point")?;
            let second = ed.get_point(PointOptions::new("Specify second point").base(first))?;
            direction(first, second)
        }
        Answer::Enter => return Err(Cancelled),
    };
    Ok(new - reference)
}

/// ROTATE: turn objects about a base point by a typed angle, by the direction
/// from the base point to a picked point, or from a reference angle to a new
/// one. Copy keeps the originals.
pub fn rotate_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    let ids = ed.pickset_or_select("Select objects")?;
    let objects = objects_of(ed, &ids)?;
    let originals = entities_of(ed, &ids)?;
    let base = ed.get_point("Specify base point")?;
    let mut copy = false;
    let angle = loop {
        let shown = originals.clone();
        let options = NumberOptions::new("Specify rotation angle")
            .base(base)
            .keyword("Copy", "C")
            .keyword("Reference", "R")
            .ghost(move |at| {
                let angle = direction(base, at);
                shown.iter().map(|e| e.rotated(base, angle)).collect()
            });
        match ed.ask_angle(options)? {
            Answer::Value(angle) => break angle,
            Answer::Keyword(k) if k == "Copy" => {
                copy = true;
                ed.say("Rotate a copy of the selected objects.");
            }
            Answer::Keyword(_) => break reference_angle(ed, base, &originals)?,
            Answer::Enter => return Err(Cancelled),
        }
    };
    let count = plural(objects.len(), "object");
    apply(ed, "Rotate", objects, copy, move |e| e.rotated(base, angle))?;
    let units = ed.read(|doc| *doc.units())?;
    ed.say(format!("Rotated {count} {}", angle_message(&units, angle)));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(a: (f64, f64), b: (f64, f64)) -> Entity {
        Entity::Line {
            start: p(a.0, a.1),
            end: p(b.0, b.1),
        }
    }

    /// A drawing with one selected line from (0,0) to (10,0).
    fn state_with_selected_line() -> AppState {
        let mut state = AppState::new();
        state.add_entity(line((0.0, 0.0), (10.0, 0.0)));
        state.select_all();
        state
    }

    fn run(
        state: &mut AppState,
        command: fn(&mut Editor) -> Result<(), Cancelled>,
        inputs: &[&str],
    ) {
        state.start_command("Test", command);
        state.settle_command();
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
    }

    fn entities(state: &AppState) -> Vec<Entity> {
        let objects = state.session().document().objects();
        objects.iter().map(|o| o.entity.clone()).collect()
    }

    fn end_of(e: &Entity) -> Point {
        let Entity::Line { end, .. } = e else {
            panic!("a line")
        };
        *end
    }

    #[test]
    fn scale_takes_a_factor_about_the_base_point() {
        let mut state = state_with_selected_line();
        run(&mut state, scale_objects, &["0,0", "2"]);
        assert_eq!(entities(&state), [line((0.0, 0.0), (20.0, 0.0))]);
        assert!(!state.command_running());
        state.undo();
        assert_eq!(entities(&state), [line((0.0, 0.0), (10.0, 0.0))]);
    }

    #[test]
    fn scale_refuses_text_it_cannot_scale_and_scales_the_rest_of_the_text() {
        use od_core::{MText, TextStyleId};
        let text = |contents: &str| {
            Entity::MText(Box::new(MText::new(
                p(1.0, 1.0),
                2.0,
                contents,
                TextStyleId::STANDARD,
            )))
        };
        let mut state = AppState::new();
        state.add_entity(text(r"\pxqd,l3;item"));
        state.select_all();
        let before = entities(&state);
        run(&mut state, scale_objects, &[]);
        assert!(!state.command_running(), "it does not ask for a base point");
        assert_eq!(entities(&state), before);

        let mut state = AppState::new();
        state.add_entity(text(r"plain \H3;text"));
        state.select_all();
        run(&mut state, scale_objects, &["0,0", "2"]);
        let Entity::MText(scaled) = &entities(&state)[0] else {
            panic!("text")
        };
        assert_eq!(scaled.nominal_height, 4.0);
        assert_eq!(scaled.contents, r"plain \H6;text");
    }

    #[test]
    fn scale_takes_the_distance_to_a_clicked_point() {
        let mut state = state_with_selected_line();
        state.start_command("Test", scale_objects);
        state.settle_command();
        state.cmd_submit("0,0");
        state.settle_command();
        // Clicking 3 units from the base point is a factor of 3.
        state.answer_point(p(0.0, 3.0));
        state.settle_command();
        assert_eq!(entities(&state), [line((0.0, 0.0), (30.0, 0.0))]);
    }

    #[test]
    fn scale_refuses_zero_and_negative_factors() {
        let mut state = state_with_selected_line();
        run(&mut state, scale_objects, &["0,0", "0", "-2"]);
        assert!(state.command_running(), "still asking for a factor");
        assert_eq!(entities(&state), [line((0.0, 0.0), (10.0, 0.0))]);
    }

    #[test]
    fn scale_copy_keeps_the_original_and_adds_a_scaled_copy() {
        let mut state = state_with_selected_line();
        run(&mut state, scale_objects, &["0,0", "c", "3"]);
        assert_eq!(
            entities(&state),
            [line((0.0, 0.0), (10.0, 0.0)), line((0.0, 0.0), (30.0, 0.0))]
        );
        state.undo();
        assert_eq!(entities(&state).len(), 1, "one undo step");
    }

    #[test]
    fn scale_reference_is_the_new_length_over_the_reference_length() {
        let mut state = state_with_selected_line();
        run(&mut state, scale_objects, &["0,0", "r"]);
        assert!(state
            .cmd_prompt()
            .text
            .starts_with("Specify reference length"));
        state.cmd_submit("5");
        state.settle_command();
        assert!(state.cmd_prompt().text.starts_with("Specify new length"));
        state.cmd_submit("15");
        state.settle_command();
        assert_eq!(entities(&state), [line((0.0, 0.0), (30.0, 0.0))]);
    }

    #[test]
    fn scale_reference_defaults_to_one_and_accepts_two_points_for_the_new_length() {
        let mut state = state_with_selected_line();
        run(&mut state, scale_objects, &["0,0", "r", ""]);
        run_more(&mut state, &["p", "1,1", "1,5"]);
        assert_eq!(entities(&state), [line((0.0, 0.0), (40.0, 0.0))]);
    }

    fn run_more(state: &mut AppState, inputs: &[&str]) {
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
    }

    #[test]
    fn rotate_takes_an_angle_in_degrees() {
        let mut state = state_with_selected_line();
        run(&mut state, rotate_objects, &["0,0"]);
        assert_eq!(state.cmd_prompt().text, "Specify rotation angle or");
        state.cmd_submit("90");
        state.settle_command();
        let end = end_of(&entities(&state)[0]);
        assert!(end.x.abs() < 1e-9 && (end.y - 10.0).abs() < 1e-9);
        assert!(state.message.contains("90.00"));
    }

    #[test]
    fn rotate_by_pointing_turns_toward_the_point() {
        let mut state = state_with_selected_line();
        run(&mut state, rotate_objects, &["0,0"]);
        // Straight up from the base is a quarter turn.
        state.answer_point(p(0.0, 5.0));
        state.settle_command();
        let end = end_of(&entities(&state)[0]);
        assert!(end.x.abs() < 1e-9 && (end.y - 10.0).abs() < 1e-9);
    }

    #[test]
    fn rotate_copy_keeps_the_original() {
        let mut state = state_with_selected_line();
        run(&mut state, rotate_objects, &["0,0", "c", "90"]);
        let all = entities(&state);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0], line((0.0, 0.0), (10.0, 0.0)));
        assert!((end_of(&all[1]).y - 10.0).abs() < 1e-9);
    }

    #[test]
    fn rotate_reference_turns_by_new_minus_reference() {
        let mut state = state_with_selected_line();
        run(&mut state, rotate_objects, &["0,0", "r", "30", "75"]);
        let end = end_of(&entities(&state)[0]);
        let want = 45.0_f64.to_radians();
        assert!((end.x - 10.0 * want.cos()).abs() < 1e-9);
        assert!((end.y - 10.0 * want.sin()).abs() < 1e-9);
    }

    #[test]
    fn rotate_reference_angle_can_be_two_points_not_measured_from_the_base() {
        let mut state = state_with_selected_line();
        // The reference vector (5,5) -> (5,15) points north; the new angle is east.
        run(&mut state, rotate_objects, &["0,0", "r", "5,5"]);
        assert_eq!(state.cmd_prompt().text, "Specify second point");
        run_more(&mut state, &["5,15", "0"]);
        let end = end_of(&entities(&state)[0]);
        assert!((end.x - 10.0 * (-90.0_f64).to_radians().cos()).abs() < 1e-9);
        assert!((end.y + 10.0).abs() < 1e-9, "turned back 90 degrees");
    }

    #[test]
    fn scale_reference_length_can_be_two_points_not_measured_from_the_base() {
        let mut state = state_with_selected_line();
        // A reference vector 2 long, scaled to a new length of 6: factor 3.
        run(&mut state, scale_objects, &["0,0", "r", "3,3", "3,5", "6"]);
        assert_eq!(entities(&state), [line((0.0, 0.0), (30.0, 0.0))]);
    }

    #[test]
    fn rotate_reference_defaults_to_zero_and_accepts_points_for_the_new_angle() {
        let mut state = state_with_selected_line();
        run(
            &mut state,
            rotate_objects,
            &["0,0", "r", "", "p", "5,5", "5,15"],
        );
        let end = end_of(&entities(&state)[0]);
        assert!(end.x.abs() < 1e-9 && (end.y - 10.0).abs() < 1e-9);
    }
}
