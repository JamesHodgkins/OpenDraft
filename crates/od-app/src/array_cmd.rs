//! ARRAY, ARRAYRECT and ARRAYPOLAR (ARRAYPATH is in `array_path.rs`):
//! copies of the selected objects in rows and columns or round a centre,
//! worded as AutoCAD 2020's. The arrays are not associative: each item is a
//! plain copy, as AutoCAD's with Associative = No. The geometry is
//! `od_core::array`.

use std::f64::consts::TAU;
use std::sync::Arc;

use od_commands::{Answer, Cancelled, Editor, NumberOptions, Options, PointOptions};
use od_core::array::{polar, polar_step, rectangular, Placement, MAX_ITEMS};
use od_core::{Attributes, Bounds, Entity, EntityId, Point};

use crate::array_path::path_with;
use crate::text::plural;

/// The most objects one array may add.
const MAX_COPIES: usize = 200_000;
/// The most objects the preview draws.
const PREVIEW_LIMIT: usize = 2_000;

/// The objects an array copies.
pub(crate) struct Source {
    pub objects: Vec<(Entity, Attributes)>,
    pub bounds: Bounds,
}

impl Source {
    /// The width and height of the objects together, for default spacings:
    /// one and a half times each (a side with no size takes the other's).
    pub fn spacing(&self) -> (f64, f64) {
        let (w, h) = (
            self.bounds.max.x - self.bounds.min.x,
            self.bounds.max.y - self.bounds.min.y,
        );
        let side = |a: f64, b: f64| match (a > 1e-9, b > 1e-9) {
            (true, _) => a,
            (false, true) => b,
            _ => 1.0,
        };
        (1.5 * side(w, h), 1.5 * side(h, w))
    }

    pub fn center(&self) -> Point {
        Point::new(
            (self.bounds.min.x + self.bounds.max.x) / 2.0,
            (self.bounds.min.y + self.bounds.max.y) / 2.0,
        )
    }
}

/// The objects `ids` names, with their size together; `None` when none are
/// left.
pub(crate) fn read_source(ed: &Editor, ids: Vec<EntityId>) -> Result<Option<Source>, Cancelled> {
    ed.read(move |doc| {
        let found: Vec<_> = ids.iter().filter_map(|id| doc.object(*id)).collect();
        let bounds = found.iter().map(|o| o.bounds()).reduce(Bounds::union)?;
        let objects = found
            .iter()
            .map(|o| (o.entity.clone(), o.attrs.clone()))
            .collect();
        Some(Source { objects, bounds })
    })
}

/// What the preview shows: the copies, up to [`PREVIEW_LIMIT`] objects.
pub(crate) fn ghost(source: &Source, items: &[Placement]) -> impl Fn(Point) -> Vec<Entity> {
    let shown: Vec<Entity> = items
        .iter()
        .skip(1)
        .flat_map(|item| source.objects.iter().map(|(e, _)| item.apply(e)))
        .take(PREVIEW_LIMIT)
        .collect();
    let shown = Arc::new(shown);
    move |_| shown.as_ref().clone()
}

/// Whether an array of `items` copies of the objects is small enough; says
/// why not.
pub(crate) fn fits(ed: &Editor, source: &Source, items: usize) -> bool {
    let copies = items.saturating_sub(1).saturating_mul(source.objects.len());
    if items > MAX_ITEMS || copies > MAX_COPIES {
        ed.say(format!(
            "The array would have {}; the most is {MAX_ITEMS}.",
            plural(items, "item")
        ));
        return false;
    }
    true
}

/// Add every item but the first (the objects themselves) as one undo step.
pub(crate) fn make(ed: &Editor, source: Source, items: Vec<Placement>) -> Result<(), Cancelled> {
    let count = items.len();
    let copies: Vec<(Entity, Attributes)> = items
        .iter()
        .skip(1)
        .flat_map(|item| {
            source
                .objects
                .iter()
                .map(move |(e, attrs)| (item.apply(e), attrs.clone()))
        })
        .collect();
    if copies.is_empty() {
        return Ok(());
    }
    ed.transaction("Array", move |tx| {
        tx.add_with(copies);
        Ok(())
    })?;
    ed.say(format!("Array of {}.", plural(count, "item")));
    Ok(())
}

/// The array's edit prompt, AutoCAD's "Select grip to edit array", with the
/// array previewed. `Some(option)` for a keyword, `None` to finish (Enter
/// or eXit). There are no grips yet, so a click asks again.
pub(crate) fn edit_prompt(
    ed: &Editor,
    keywords: &[(&str, &str)],
    ghost: impl Fn(Point) -> Vec<Entity> + Send + Sync + 'static,
) -> Result<Option<String>, Cancelled> {
    let names: Vec<&str> = keywords.iter().map(|(label, _)| *label).collect();
    let message = format!("Select grip to edit array or [{}/eXit]", names.join("/"));
    let mut options = PointOptions::new(message).ghost(ghost);
    for (label, key) in keywords {
        options = options.keyword(*label, *key);
    }
    let options = options.keyword("eXit", "X");
    loop {
        match ed.ask_point(options.clone())? {
            Answer::Keyword(k) if k == "eXit" => return Ok(None),
            Answer::Keyword(k) => return Ok(Some(k)),
            Answer::Enter => return Ok(None),
            Answer::Value(_) => ed.say("Grip editing of arrays is not built yet."),
        }
    }
}

/// A whole number from 1 to [`MAX_ITEMS`], `current` on Enter.
pub(crate) fn ask_count(ed: &Editor, message: &str, current: usize) -> Result<usize, Cancelled> {
    loop {
        let typed = ed.get_number(NumberOptions::new(message).default(current as f64))?;
        let whole = typed.round();
        if (typed - whole).abs() < 1e-9 && whole >= 1.0 && whole <= MAX_ITEMS as f64 {
            return Ok(whole as usize);
        }
        ed.say(format!("Requires an integer between 1 and {MAX_ITEMS}."));
    }
}

/// ARRAY: select objects, then the kind of array.
pub fn array_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    let ids = ed.pickset_or_select("Select objects")?;
    let kind = ed.get_keyword(
        Options::new("Enter array type")
            .keyword("Rectangular", "R")
            .keyword("PAth", "PA")
            .keyword("POlar", "PO")
            .default("Rectangular".to_owned()),
    )?;
    match kind.as_str() {
        "PAth" => path_with(ed, ids),
        "POlar" => polar_with(ed, ids),
        _ => rect_with(ed, ids),
    }
}

/// ARRAYRECT: select objects; 4 columns and 3 rows to begin with, changed
/// by the options until Enter.
pub fn array_rect(ed: &mut Editor) -> Result<(), Cancelled> {
    let ids = ed.pickset_or_select("Select objects")?;
    rect_with(ed, ids)
}

/// The settings of a rectangular array.
#[derive(Clone, Copy)]
struct Grid {
    columns: usize,
    rows: usize,
    column_spacing: f64,
    row_spacing: f64,
}

fn rect_with(ed: &mut Editor, ids: Vec<EntityId>) -> Result<(), Cancelled> {
    let Some(source) = read_source(ed, ids)? else {
        return Ok(());
    };
    ed.say("Type = Rectangular  Associative = No");
    let (dx, dy) = source.spacing();
    let mut grid = Grid {
        columns: 4,
        rows: 3,
        column_spacing: dx,
        row_spacing: dy,
    };
    let keywords = [
        ("COUnt", "COU"),
        ("Spacing", "S"),
        ("COLumns", "COL"),
        ("Rows", "R"),
    ];
    loop {
        let items = rectangular(
            grid.columns,
            grid.rows,
            grid.column_spacing,
            grid.row_spacing,
        )
        .unwrap_or_else(|| vec![Placement::STAY]);
        let Some(option) = edit_prompt(ed, &keywords, ghost(&source, &items))? else {
            return make(ed, source, items);
        };
        let next = change_grid(ed, grid, &option)?;
        if fits(ed, &source, next.columns.saturating_mul(next.rows)) {
            grid = next;
        }
    }
}

/// `grid` as the option chosen changes it.
fn change_grid(ed: &Editor, grid: Grid, option: &str) -> Result<Grid, Cancelled> {
    let mut g = grid;
    let distance = |message: &str, current: f64, total: Option<usize>| {
        let mut options = NumberOptions::new(message).default(current);
        if total.is_some() {
            options = options.keyword("Total", "T");
        }
        match ed.ask_distance(options)? {
            Answer::Value(d) => Ok(d),
            Answer::Keyword(_) => {
                let count = total.unwrap_or(2).max(2);
                let span = current * (count - 1) as f64;
                let what = if message.contains("column") {
                    "Specify the distance between first and last column"
                } else {
                    "Specify the distance between first and last row"
                };
                let total = ed.get_distance(NumberOptions::new(what).default(span))?;
                Ok(total / (count - 1) as f64)
            }
            Answer::Enter => Ok(current),
        }
    };
    match option {
        "COUnt" => {
            g.columns = ask_count(ed, "Enter number of columns", g.columns)?;
            g.rows = ask_count(ed, "Enter number of rows", g.rows)?;
        }
        "Spacing" => {
            let options = NumberOptions::new("Specify the distance between columns")
                .default(g.column_spacing)
                .keyword("Unit cell", "U");
            match ed.ask_distance(options)? {
                Answer::Keyword(_) => {
                    let a = ed.get_point("Specify first corner")?;
                    let b = ed.get_point(PointOptions::new("Specify other corner").base(a))?;
                    (g.column_spacing, g.row_spacing) = (b.x - a.x, b.y - a.y);
                }
                Answer::Value(d) => {
                    g.column_spacing = d;
                    g.row_spacing =
                        distance("Specify the distance between rows", g.row_spacing, None)?;
                }
                Answer::Enter => {
                    g.row_spacing =
                        distance("Specify the distance between rows", g.row_spacing, None)?;
                }
            }
        }
        "COLumns" => {
            g.columns = ask_count(ed, "Enter number of columns", g.columns)?;
            let message = "Specify the distance between columns";
            g.column_spacing = distance(message, g.column_spacing, Some(g.columns))?;
        }
        _ => {
            g.rows = ask_count(ed, "Enter the number of rows", g.rows)?;
            let message = "Specify the distance between rows";
            g.row_spacing = distance(message, g.row_spacing, Some(g.rows))?;
        }
    }
    Ok(g)
}

/// ARRAYPOLAR: select objects and the centre; 6 items round a whole turn,
/// turned as they go, to begin with, changed by the options until Enter.
pub fn array_polar(ed: &mut Editor) -> Result<(), Cancelled> {
    let ids = ed.pickset_or_select("Select objects")?;
    polar_with(ed, ids)
}

fn polar_with(ed: &mut Editor, ids: Vec<EntityId>) -> Result<(), Cancelled> {
    let Some(source) = read_source(ed, ids)? else {
        return Ok(());
    };
    ed.say("Type = Polar  Associative = No");
    let center = ed.get_point("Specify center point of array")?;
    let base = source.center();
    let (mut items, mut fill, mut rotate) = (6, TAU, true);
    let keywords = [
        ("Items", "I"),
        ("Angle between", "A"),
        ("Fill angle", "F"),
        ("ROTate items", "ROT"),
    ];
    loop {
        let placed =
            polar(center, base, items, fill, rotate).unwrap_or_else(|| vec![Placement::STAY]);
        let Some(option) = edit_prompt(ed, &keywords, ghost(&source, &placed))? else {
            return make(ed, source, placed);
        };
        match option.as_str() {
            "Items" => {
                let count = ask_count(ed, "Enter number of items in array", items)?;
                if fits(ed, &source, count) {
                    items = count;
                }
            }
            "Angle between" => {
                let step = polar_step(items, fill);
                let options = NumberOptions::new("Specify the angle between items").default(step);
                let step = ed.get_angle(options)?;
                let span = step * (items.max(2) - 1) as f64;
                if (step * items as f64 - TAU).abs() < 1e-9 {
                    fill = TAU;
                } else if span.abs() <= TAU + 1e-9 && step != 0.0 {
                    fill = span;
                } else {
                    ed.say("The items would go round more than once.");
                }
            }
            "Fill angle" => {
                let options =
                    NumberOptions::new("Specify the angle to fill (+=ccw, -=cw)").default(fill);
                let angle = ed.get_angle(options)?;
                if angle != 0.0 && angle.abs() <= TAU + 1e-9 {
                    fill = angle;
                } else {
                    ed.say("Requires a value between -360 and 360, not 0.");
                }
            }
            _ => {
                let current = if rotate { "Yes" } else { "No" };
                let answer = ed.get_keyword(
                    Options::new("Rotate arrayed items?")
                        .keyword("Yes", "Y")
                        .keyword("No", "N")
                        .default(current.to_owned()),
                )?;
                rotate = answer == "Yes";
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use od_commands::Reply;

    use super::*;
    use crate::state::AppState;

    fn square() -> Entity {
        Entity::Rectangle {
            min: Point::new(0.0, 0.0),
            max: Point::new(2.0, 2.0),
        }
    }

    fn with_square() -> AppState {
        let mut state = AppState::new();
        state.add_entity(square());
        state.select_all();
        state
    }

    fn answer(state: &mut AppState, replies: &[Reply]) {
        for reply in replies {
            state.reply(reply.clone());
            state.settle_command();
        }
    }

    fn mins(state: &AppState) -> Vec<(f64, f64)> {
        let objects = state.session().document().objects();
        let mut found: Vec<(f64, f64)> = objects
            .iter()
            .map(|o| {
                let b = o.bounds();
                ((b.min.x * 1e6).round() / 1e6, (b.min.y * 1e6).round() / 1e6)
            })
            .collect();
        found.sort_by(|a, b| a.partial_cmp(b).unwrap());
        found
    }

    #[test]
    fn a_rectangular_array_starts_four_by_three_and_takes_its_options() {
        let mut state = with_square();
        state.start_command("ARRAYRECT", array_rect);
        state.settle_command();
        answer(
            &mut state,
            &[
                Reply::Keyword("COUnt".into()),
                Reply::Number(3.0),
                Reply::Number(2.0),
            ],
        );
        answer(
            &mut state,
            &[
                Reply::Keyword("Spacing".into()),
                Reply::Number(5.0),
                Reply::Number(-4.0),
            ],
        );
        answer(&mut state, &[Reply::Enter]);
        assert!(!state.command_running());
        assert_eq!(
            mins(&state),
            [
                (0.0, -4.0),
                (0.0, 0.0),
                (5.0, -4.0),
                (5.0, 0.0),
                (10.0, -4.0),
                (10.0, 0.0)
            ]
        );
        state.undo();
        assert_eq!(state.session().document().len(), 1, "one undo step");
    }

    #[test]
    fn the_default_rectangular_array_spaces_by_the_objects_size() {
        let mut state = with_square();
        state.start_command("ARRAYRECT", array_rect);
        state.settle_command();
        answer(&mut state, &[Reply::Keyword("eXit".into())]);
        let found = mins(&state);
        assert_eq!(found.len(), 12);
        assert_eq!(
            found.last(),
            Some(&(9.0, 6.0)),
            "three steps of 3 across, two up"
        );
    }

    #[test]
    fn a_column_total_is_shared_between_the_gaps() {
        let mut state = with_square();
        state.start_command("ARRAYRECT", array_rect);
        state.settle_command();
        answer(
            &mut state,
            &[
                Reply::Keyword("COLumns".into()),
                Reply::Number(5.0),
                Reply::Keyword("Total".into()),
                Reply::Number(20.0),
                Reply::Keyword("Rows".into()),
                Reply::Number(1.0),
                Reply::Enter,
                Reply::Enter,
            ],
        );
        assert_eq!(
            mins(&state),
            [
                (0.0, 0.0),
                (5.0, 0.0),
                (10.0, 0.0),
                (15.0, 0.0),
                (20.0, 0.0)
            ]
        );
    }

    #[test]
    fn a_polar_array_goes_round_the_centre() {
        let mut state = with_square();
        state.start_command("ARRAY", array_objects);
        state.settle_command();
        answer(
            &mut state,
            &[
                Reply::Keyword("POlar".into()),
                Reply::Point(Point::new(-9.0, 1.0)),
                Reply::Keyword("Items".into()),
                Reply::Number(4.0),
                Reply::Keyword("Fill angle".into()),
                Reply::Number(std::f64::consts::PI),
                Reply::Enter,
            ],
        );
        let found = mins(&state);
        assert_eq!(found.len(), 4);
        // Half a turn about (-9, 1) takes the square to the far side.
        assert_eq!(found[0], (-20.0, 0.0));
        assert!(state.cmdline.log.iter().any(|l| l == "Array of 4 items."));
    }
}
