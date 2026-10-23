//! DIVIDE and MEASURE: points (or copies of a block) along an object, at
//! equal parts or every distance. The geometry is `od_core::divide`; the
//! commands only ask and record, as one undo step each.
//!
//! MEASURE starts at the end of the object nearest the point it was picked
//! at; a closed object starts where it starts.

use std::sync::Arc;

use od_commands::{Answer, Cancelled, Editor, NumberOptions, Options, Picked};
use od_core::array::Path;
use od_core::divide::{divide, measure, Mark, MAX_SEGMENTS};
use od_core::{BlockDef, Entity, Insert, Point};

use crate::insert_cmd::choose_block;
use crate::text::plural;

/// What goes at each mark: a point, or a block, turned with the object or not.
enum Placed {
    Points,
    Block(Arc<BlockDef>, bool),
}

/// The path of the object picked, or `None` after saying why there is none.
fn pick_path(ed: &Editor, message: &str, verb: &str) -> Result<Option<(Path, Picked)>, Cancelled> {
    let picked = match ed.ask_object(Options::<Picked>::new(message))? {
        Answer::Value(picked) => picked,
        _ => return Ok(None),
    };
    let id = picked.id;
    let entity = ed.read(move |doc| doc.get(id).cloned())?;
    match entity.as_ref().and_then(Path::of) {
        Some(path) => Ok(Some((path, picked))),
        None => {
            ed.say(format!("Cannot {verb} that object."));
            Ok(None)
        }
    }
}

/// The block to place at each mark, and whether it turns with the object.
fn ask_block(ed: &Editor) -> Result<Option<Placed>, Cancelled> {
    let Some(block) = choose_block(ed)? else {
        return Ok(None);
    };
    let align = ed.get_keyword(
        Options::<String>::new("Align block with object?")
            .keyword("Yes", "Y")
            .keyword("No", "N")
            .default("Yes".to_owned()),
    )?;
    Ok(Some(Placed::Block(block, align == "Yes")))
}

/// Add a point or a block at each mark.
fn place(ed: &Editor, marks: Vec<Mark>, what: Placed, name: &'static str) -> Result<(), Cancelled> {
    if marks.is_empty() {
        ed.say("Nothing to mark: the object is shorter than that.");
        return Ok(());
    }
    let count = marks.len();
    ed.transaction(name, move |tx| {
        for mark in marks {
            tx.add(match &what {
                Placed::Points => Entity::Point { at: mark.at },
                Placed::Block(block, align) => {
                    let mut insert = Insert::new(block.clone(), mark.at);
                    if *align {
                        insert.rotation = mark.direction;
                    }
                    Entity::Insert(Box::new(insert))
                }
            });
        }
        Ok(())
    })?;
    ed.say(format!("{} placed.", plural(count, "mark")));
    Ok(())
}

/// DIVIDE: the number of segments, or Block to place a block instead of
/// points, then the marks between the parts.
pub fn divide_object(ed: &mut Editor) -> Result<(), Cancelled> {
    let Some((path, _)) = pick_path(ed, "Select object to divide", "divide")? else {
        return Ok(());
    };
    let mut what = Placed::Points;
    loop {
        let options = NumberOptions::new("Enter the number of segments").keyword("Block", "B");
        match ed.ask_number(options)? {
            Answer::Keyword(_) => match ask_block(ed)? {
                Some(block) => what = block,
                None => return Ok(()),
            },
            Answer::Value(n) => {
                let whole = n.round();
                let marks = (n - whole).abs() < 1e-9 && whole >= 0.0;
                match marks.then(|| divide(&path, whole as usize)).flatten() {
                    Some(marks) => return place(ed, marks, what, "Divide"),
                    None => ed.say(format!("Requires an integer between 2 and {MAX_SEGMENTS}.")),
                }
            }
            Answer::Enter => return Ok(()),
        }
    }
}

/// The path run from the end nearest `at` (a closed one has one start).
fn from_nearest(path: Path, at: Point) -> Path {
    let near = |p: Point| (p.x - at.x).hypot(p.y - at.y);
    if !path.is_closed() && near(path.end()) < near(path.start()) {
        path.reversed()
    } else {
        path
    }
}

/// MEASURE: the length of a segment, or Block to place a block instead of
/// points, then the marks from the end nearest the pick.
pub fn measure_object(ed: &mut Editor) -> Result<(), Cancelled> {
    let Some((path, picked)) = pick_path(ed, "Select object to measure", "measure")? else {
        return Ok(());
    };
    let path = from_nearest(path, picked.at);
    let mut what = Placed::Points;
    loop {
        let options = NumberOptions::new("Specify length of segment").keyword("Block", "B");
        match ed.ask_distance(options)? {
            Answer::Keyword(_) => match ask_block(ed)? {
                Some(block) => what = block,
                None => return Ok(()),
            },
            Answer::Value(d) => match measure(&path, d) {
                Some(marks) => return place(ed, marks, what, "Measure"),
                None => ed.say("Requires a length above zero that makes at most 100,000 marks."),
            },
            Answer::Enter => return Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    fn state_with_line() -> AppState {
        let mut state = AppState::new();
        state.add_entity(Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0),
        });
        state
    }

    fn run(
        state: &mut AppState,
        command: fn(&mut Editor) -> Result<(), Cancelled>,
        pick: Point,
        inputs: &[&str],
    ) {
        state.start_command("Test", command);
        state.settle_command();
        let id = state
            .session()
            .document()
            .objects()
            .last()
            .map(|o| o.id)
            .unwrap();
        state.answer_object(pick, Some(id), false);
        state.settle_command();
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
    }

    fn points(state: &AppState) -> Vec<f64> {
        let mut found: Vec<f64> = state
            .session()
            .document()
            .objects()
            .iter()
            .filter_map(|o| match o.entity {
                Entity::Point { at } => Some((at.x * 1e6).round() / 1e6),
                _ => None,
            })
            .collect();
        found.sort_by(f64::total_cmp);
        found
    }

    #[test]
    fn divide_puts_points_between_the_parts_in_one_undo_step() {
        let mut state = state_with_line();
        run(&mut state, divide_object, Point::new(5.0, 0.0), &["1", "4"]);
        assert_eq!(points(&state), [2.5, 5.0, 7.5]);
        state.undo();
        assert!(points(&state).is_empty());
    }

    #[test]
    fn measure_starts_at_the_end_nearest_the_pick() {
        let mut state = state_with_line();
        run(&mut state, measure_object, Point::new(9.0, 0.0), &["3"]);
        assert_eq!(points(&state), [1.0, 4.0, 7.0]);
        let mut state = state_with_line();
        run(&mut state, measure_object, Point::new(1.0, 0.0), &["3"]);
        assert_eq!(points(&state), [3.0, 6.0, 9.0]);
    }

    #[test]
    fn a_block_is_placed_and_turned_with_the_object() {
        let mut state = AppState::new();
        state.add_entity(Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(1.0, 0.0),
        });
        state.select_all();
        state.start_command("BLOCK", crate::block_cmd::define_block);
        state.settle_command();
        for answer in ["MARK", "0,0"] {
            state.cmd_submit(answer);
            state.settle_command();
        }
        state.add_entity(Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(0.0, 10.0),
        });
        run(
            &mut state,
            divide_object,
            Point::new(0.0, 5.0),
            &["b", "MARK", "", "2"],
        );
        let objects = state.session().document().objects();
        let turned: Vec<f64> = objects
            .iter()
            .filter_map(|o| match &o.entity {
                Entity::Insert(i) if i.insertion.y > 0.0 => Some(i.rotation),
                _ => None,
            })
            .collect();
        assert_eq!(turned.len(), 1, "one mark between two parts");
        assert!((turned[0] - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    }

    #[test]
    fn a_bad_count_is_asked_again_and_a_point_is_not_dividable() {
        let mut state = state_with_line();
        run(
            &mut state,
            divide_object,
            Point::new(5.0, 0.0),
            &["1", "2.5"],
        );
        assert!(points(&state).is_empty());
        assert!(state.command_running(), "still asking for a count");
        state.cmd_submit("2");
        state.settle_command();
        assert_eq!(points(&state), [5.0]);
    }
}
