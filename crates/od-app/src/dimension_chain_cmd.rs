//! DIMBASELINE and DIMCONTINUE: a run of linear dimensions from the one
//! made last (or one picked), each sharing an extension line with the one
//! before. What each new dimension is, is `Dimension::baseline` and
//! `Dimension::continued`.

use std::sync::Arc;

use od_commands::{Answer, Cancelled, Editor, PointOptions};
use od_core::{Dimension, Entity, Point};

use crate::dimension_cmd::pick;
use crate::draw::add;

/// The dimension chained from, and which of its extension lines the next
/// one starts at.
#[derive(Clone)]
struct Base {
    dimension: Dimension,
    from_second: bool,
}

/// The newest linear or aligned dimension in the drawing.
fn newest(ed: &Editor) -> Result<Option<Dimension>, Cancelled> {
    ed.read(|doc| {
        doc.objects().iter().rev().find_map(|o| match &o.entity {
            Entity::Dimension(d) if d.origin(false).is_some() => Some((**d).clone()),
            _ => None,
        })
    })
}

/// Ask for a linear dimension to chain from. The extension line nearer the
/// pick is the one the next dimension starts at.
fn select(ed: &mut Editor) -> Result<Base, Cancelled> {
    loop {
        let Some((picked, entity)) = pick(ed, "Select base dimension")? else {
            return Err(Cancelled);
        };
        match entity {
            Entity::Dimension(d) if d.origin(false).is_some() => {
                let from_second = d.nearer_origin_is_second(picked.at);
                return Ok(Base {
                    dimension: *d,
                    from_second,
                });
            }
            _ => ed.say("Select a linear or aligned dimension."),
        }
    }
}

fn chain(ed: &mut Editor, baseline: bool) -> Result<(), Cancelled> {
    let mut base = match newest(ed)? {
        Some(dimension) => Base {
            dimension,
            // A baseline grows from the first line, a continued one from
            // the second.
            from_second: !baseline,
        },
        None => select(ed)?,
    };
    // The bases the dimensions made here replaced, for Undo.
    let mut earlier: Vec<Base> = Vec::new();
    loop {
        let from = base.clone();
        let shown = from.clone();
        let next = move |at: Point, base: &Base| -> Option<Dimension> {
            if baseline {
                base.dimension.baseline(base.from_second, at)
            } else {
                base.dimension.continued(base.from_second, at)
            }
        };
        let make = Arc::new(next);
        let ghost = make.clone();
        let options = PointOptions::new("Specify a second extension line origin or [Undo/Select]")
            .keyword("Undo", "U")
            .keyword("Select", "S")
            .base(from.dimension.origin(from.from_second).unwrap_or_default())
            .ghost(move |at| {
                ghost(at, &shown)
                    .map(|d| Entity::Dimension(Box::new(d)))
                    .into_iter()
                    .collect()
            });
        match ed.ask_point(options)? {
            Answer::Value(at) => {
                let Some(dimension) = make(at, &from) else {
                    ed.say("Those points make no dimension.");
                    continue;
                };
                add(
                    ed,
                    Entity::Dimension(Box::new(dimension.clone())),
                    "Dimension",
                )?;
                earlier.push(from);
                base = Base {
                    dimension,
                    from_second: !baseline,
                };
            }
            Answer::Keyword(k) if k == "Undo" => match earlier.pop() {
                Some(previous) => {
                    ed.undo()?;
                    base = previous;
                }
                None => ed.say("Nothing to undo."),
            },
            Answer::Keyword(_) => base = select(ed)?,
            Answer::Enter => return Ok(()),
        }
    }
}

pub fn dim_baseline(ed: &mut Editor) -> Result<(), Cancelled> {
    chain(ed, true)
}

pub fn dim_continue(ed: &mut Editor) -> Result<(), Cancelled> {
    chain(ed, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use od_core::DimStyle;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn started(run: fn(&mut Editor) -> Result<(), Cancelled>) -> AppState {
        let mut state = AppState::new();
        let d = Dimension::linear(
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(5.0, 5.0),
            0.0,
            &DimStyle::default(),
        );
        state.add_entity(Entity::Dimension(Box::new(d)));
        state.start_command("Test", run);
        state.settle_command();
        state
    }

    fn submit(state: &mut AppState, text: &str) {
        state.cmd_submit(text);
        state.settle_command();
    }

    fn measurements(state: &AppState) -> Vec<f64> {
        state
            .session()
            .document()
            .objects()
            .iter()
            .filter_map(|o| match &o.entity {
                Entity::Dimension(d) => d.measurement(),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn baselines_all_start_at_the_first_origin_and_stack() {
        let mut state = started(dim_baseline);
        submit(&mut state, "25,0");
        submit(&mut state, "40,0");
        submit(&mut state, "");
        assert_eq!(measurements(&state), [10.0, 25.0, 40.0]);
        let doc = state.session().document();
        let lines: Vec<f64> = doc
            .objects()
            .iter()
            .filter_map(|o| match &o.entity {
                Entity::Dimension(d) => Some(d.points()[2].y),
                _ => None,
            })
            .collect();
        assert_eq!(lines, [5.0, 8.75, 12.5]);
    }

    #[test]
    fn continued_dimensions_follow_one_another_on_one_line() {
        let mut state = started(dim_continue);
        submit(&mut state, "25,0");
        submit(&mut state, "40,0");
        submit(&mut state, "");
        assert_eq!(measurements(&state), [10.0, 15.0, 15.0]);
    }

    #[test]
    fn undo_takes_back_the_last_dimension() {
        let mut state = started(dim_continue);
        submit(&mut state, "25,0");
        submit(&mut state, "u");
        assert_eq!(measurements(&state), [10.0]);
        submit(&mut state, "30,0");
        assert_eq!(measurements(&state), [10.0, 20.0]);
    }

    #[test]
    fn with_no_dimension_one_is_asked_for() {
        let mut state = AppState::new();
        state.start_command("Test", dim_baseline);
        state.settle_command();
        assert!(state.cmd_prompt().text.contains("Select base dimension"));
    }
}
