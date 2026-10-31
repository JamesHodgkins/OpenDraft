//! HATCH: fill the area bounded by the selected objects with a pattern, a
//! solid colour or a user-defined set of lines. The geometry is
//! `od_core::hatch`; this only asks.
//!
//! The command follows the Select Objects route of AutoCAD's `-HATCH`: the
//! objects are chosen (or were already selected), the hatch is drawn as a
//! preview at the pointer, and Enter or a click accepts it. Pattern, Scale,
//! Angle, Spacing, Double, Style and Origin change the preview and are
//! remembered as `HPNAME`, `HPSCALE`, `HPANG`, `HPSPACE`, `HPDOUBLE`,
//! `HPISLANDDETECTION` (see `hatch_memory.rs`). A hatch is put behind its
//! boundary in the drawing order, as AutoCAD's default `HPDRAWORDER` does.
//! Without a selection the command asks for an internal point: the boundary
//! is traced from the curves that are visible, with the objects inside it as
//! islands (`od_core::hatch::trace`). Associativity and several hatches at
//! once are not built.

use od_commands::{Answer, Cancelled, Editor, NumberOptions, Options, PointOptions};
use od_core::hatch::{
    builtin, loops_and_sources, trace, BoundarySource, HatchError, HatchLimits, HatchLoop,
    HatchStyle,
};
use od_core::{Entity, EntityId, Point};

use crate::defaults::EditorDefaults;
use crate::hatch_memory::{FillChoice, Memory};
use crate::text::plural;

/// Ends closer than this count as meeting, as for JOIN.
const GAP: f64 = 1e-6;

const SELECT: &str = "Select objects";

/// What the current settings are, as a line for the command line.
pub(crate) fn summary(m: &Memory) -> String {
    let style = m.style.label();
    let style = format!(
        "{style}   Associative={}",
        if m.associative { "Yes" } else { "No" }
    );
    match &m.fill {
        FillChoice::Solid => format!("Current hatch: SOLID   Island style={style}"),
        FillChoice::UserDefined => format!(
            "Current hatch: User defined   Spacing={}   Angle={}   Double={}   Island style={style}",
            m.spacing,
            m.angle.to_degrees(),
            if m.double { "Yes" } else { "No" },
        ),
        FillChoice::Named(name) => format!(
            "Current hatch: {name}   Scale={}   Angle={}   Island style={style}",
            m.scale,
            m.angle.to_degrees()
        ),
    }
}

/// The options that apply to the current fill.
pub(crate) fn with_keywords<T>(
    options: Options<T>,
    fill: &FillChoice,
    creating: bool,
) -> Options<T> {
    let options = options.keyword("Pattern", "P");
    let options = match fill {
        FillChoice::Solid => options,
        FillChoice::UserDefined => options
            .keyword("Angle", "A")
            .keyword("Spacing", "SP")
            .keyword("Double", "D"),
        FillChoice::Named(_) => options.keyword("Scale", "S").keyword("Angle", "A"),
    };
    let options = options.keyword("Style", "T");
    let options = if creating {
        options.keyword("Associativity", "AS")
    } else {
        options
    };
    match fill {
        FillChoice::Solid => options,
        _ => options.keyword("Origin", "O"),
    }
}

fn ask_pattern(ed: &Editor) -> Result<(), Cancelled> {
    loop {
        let typed = ed.get_string(
            Options::<String>::new("Enter a pattern name, SOLID, USER, or ? to list")
                .default(ed.defaults().hatch.get().name()),
        )?;
        let text = typed.trim();
        if text == "?" {
            let names: Vec<&str> = builtin().names().collect();
            ed.say(format!("Patterns: SOLID, USER, {}", names.join(", ")));
            continue;
        }
        let name = if text.eq_ignore_ascii_case("u") {
            "_USER"
        } else {
            text
        };
        match Memory::choice_named(name) {
            Some(choice) => {
                ed.defaults().hatch.change(|m| m.fill = choice);
                return Ok(());
            }
            None => ed.say(format!("Pattern \"{text}\" not found. Enter ? to list.")),
        }
    }
}

fn positive(ed: &Editor, message: &str, current: f64) -> Result<f64, Cancelled> {
    loop {
        let value = ed.get_number(NumberOptions::new(message).default(current))?;
        if value.is_finite() && value > 0.0 {
            return Ok(value);
        }
        ed.say("Requires a positive number.");
    }
}

fn ask_style(ed: &Editor) -> Result<(), Cancelled> {
    let answer = ed.get_keyword(
        Options::<String>::new("Enter island detection style")
            .keyword("Normal", "N")
            .keyword("Outer", "O")
            .keyword("Ignore", "I")
            .default(ed.defaults().hatch.get().style.label().to_owned()),
    )?;
    let style = match answer.as_str() {
        "Outer" => HatchStyle::Outer,
        "Ignore" => HatchStyle::Ignore,
        _ => HatchStyle::Normal,
    };
    ed.defaults().hatch.change(|m| m.style = style);
    Ok(())
}

/// Carries out one of the option keywords.
pub(crate) fn adjust(ed: &Editor, keyword: &str) -> Result<(), Cancelled> {
    match keyword {
        "Pattern" => ask_pattern(ed)?,
        "Scale" => {
            let scale = positive(
                ed,
                "Specify a scale for the pattern",
                ed.defaults().hatch.get().scale,
            )?;
            ed.defaults().hatch.change(|m| m.scale = scale);
        }
        "Angle" => {
            let angle = ed.get_angle(
                NumberOptions::new("Specify an angle for the pattern")
                    .default(ed.defaults().hatch.get().angle),
            )?;
            ed.defaults().hatch.change(|m| m.angle = angle);
        }
        "Spacing" => {
            let spacing = positive(
                ed,
                "Specify the spacing between lines",
                ed.defaults().hatch.get().spacing,
            )?;
            ed.defaults().hatch.change(|m| m.spacing = spacing);
        }
        "Double" => {
            let answer = ed.get_keyword(
                Options::<String>::new("Double hatch area")
                    .keyword("Yes", "Y")
                    .keyword("No", "N")
                    .default(
                        if ed.defaults().hatch.get().double {
                            "Yes"
                        } else {
                            "No"
                        }
                        .to_owned(),
                    ),
            )?;
            ed.defaults().hatch.change(|m| m.double = answer == "Yes");
        }
        "Style" => ask_style(ed)?,
        "Associativity" => {
            let answer = ed.get_keyword(
                Options::<String>::new("Associate the hatch with its boundary")
                    .keyword("Yes", "Y")
                    .keyword("No", "N")
                    .default(
                        if ed.defaults().hatch.get().associative {
                            "Yes"
                        } else {
                            "No"
                        }
                        .to_owned(),
                    ),
            )?;
            ed.defaults()
                .hatch
                .change(|m| m.associative = answer == "Yes");
        }
        "Origin" => {
            let origin = ed.get_point(PointOptions::new("Specify an origin point"))?;
            ed.defaults().hatch.change(|m| m.origin = origin);
        }
        _ => {}
    }
    Ok(())
}

/// How far a curve of the boundary may stray when the preview is checked.
pub(crate) fn check_tolerance(h: &od_core::HatchEntity) -> f64 {
    h.bounds().map_or(1e-6, |b| {
        ((b.max.x - b.min.x).hypot(b.max.y - b.min.y) * 1e-4).max(1e-9)
    })
}

/// What a hatch is to be made from, once found.
struct Found {
    loops: Vec<HatchLoop>,
    /// The objects the outer boundary is made of, for the drawing order.
    sources: Vec<EntityId>,
    /// The point it was found from.
    seed: Option<Point>,
}

fn source(id: EntityId) -> BoundarySource {
    BoundarySource::Entity(id)
}

/// The loops of `ids`, or `None` after saying why there are none.
fn from_objects(ed: &Editor, ids: Vec<EntityId>) -> Result<Option<Found>, Cancelled> {
    let wanted = ids.clone();
    let objects = ed.read(move |doc| -> Vec<(EntityId, Entity)> {
        wanted
            .iter()
            .filter_map(|id| doc.object(*id).map(|o| (*id, o.entity.clone())))
            .collect()
    })?;
    let found: Vec<Entity> = objects.iter().map(|(_, e)| e.clone()).collect();
    match loops_and_sources(&found, GAP) {
        Ok((pairs, ignored)) => {
            let loops = pairs
                .into_iter()
                .map(|(l, used)| {
                    l.with_sources(used.iter().map(|&i| source(objects[i].0)).collect())
                })
                .collect();
            if ignored > 0 {
                ed.say(format!(
                    "{} cannot be hatched and {} left out.",
                    plural(ignored, "object"),
                    if ignored == 1 { "was" } else { "were" }
                ));
            }
            Ok(Some(Found {
                loops,
                sources: ids,
                seed: None,
            }))
        }
        Err(why) => {
            ed.say(format!("Unable to hatch: {why}."));
            Ok(None)
        }
    }
}

/// The boundary around `at` made by what is visible in the space being
/// edited, or `None` after saying why there is none.
fn from_point(ed: &Editor, at: Point) -> Result<Option<Found>, Cancelled> {
    let drawn = ed.read(|doc| -> Vec<(EntityId, Entity)> {
        doc.visible_objects()
            .map(|o| (o.id, o.entity.clone()))
            .collect()
    })?;
    let entities: Vec<Entity> = drawn.iter().map(|(_, e)| e.clone()).collect();
    let extent = entities
        .iter()
        .map(Entity::bounds)
        .filter(od_core::Bounds::is_finite)
        .reduce(|a, b| a.union(b))
        .map_or(1.0, |b| (b.max.x - b.min.x).hypot(b.max.y - b.min.y));
    // Curves are followed to a hundred-thousandth of the drawing's size.
    match trace(&entities, at, (extent * 1e-5).max(1e-9)) {
        Ok(t) => Ok(Some(Found {
            sources: t.outer_sources.iter().map(|&i| drawn[i].0).collect(),
            loops: t
                .loops
                .into_iter()
                .zip(&t.loop_sources)
                .map(|(l, used)| l.with_sources(used.iter().map(|&i| source(drawn[i].0)).collect()))
                .collect(),
            seed: Some(at),
        })),
        Err(why) => {
            ed.say(format!("{why}."));
            Ok(None)
        }
    }
}

/// HATCH. See the module notes.
pub fn hatch_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    let mut found: Option<Found> = None;
    if !ed.pickset().is_empty() {
        found = from_objects(ed, ed.pickset().to_vec())?;
    }
    loop {
        let Some(current) = &found else {
            ed.say(summary(&ed.defaults().hatch.get()));
            let options = with_keywords(
                PointOptions::new("Pick internal point"),
                &ed.defaults().hatch.get().fill,
                true,
            )
            .keyword(SELECT, "SE");
            match ed.ask_point(options)? {
                Answer::Value(at) => found = from_point(ed, at)?,
                Answer::Keyword(k) if k == SELECT => {
                    let ids = ed.get_selection("Select objects to hatch")?;
                    found = from_objects(ed, ids)?;
                }
                Answer::Keyword(k) => adjust(ed, &k)?,
                Answer::Enter => return Ok(()),
            }
            continue;
        };
        let mut hatch = match ed.defaults().hatch.get().hatch(current.loops.clone()) {
            Ok(h) => h,
            Err(why) => {
                ed.say(format!("Unable to hatch: {why}"));
                adjust(ed, "Pattern")?;
                continue;
            }
        };
        hatch.seed_points = current.seed.into_iter().collect();
        hatch.associative = ed.defaults().hatch.get().associative;
        if !hatch.associative {
            hatch = hatch.detached();
        }
        let problem: Option<HatchError> = hatch
            .evaluate(check_tolerance(&hatch), &HatchLimits::default())
            .err();
        if let Some(why) = &problem {
            ed.say(format!("The hatch cannot be drawn: {why}."));
        }
        ed.say(summary(&ed.defaults().hatch.get()));
        let shown = hatch.clone();
        let options = with_keywords(
            PointOptions::new(if problem.is_some() {
                "Change the settings"
            } else {
                "Press Enter or click to accept the hatch"
            })
            .ghost(move |_| vec![Entity::Hatch(Box::new(shown.clone()))]),
            &ed.defaults().hatch.get().fill,
            true,
        )
        .keyword(SELECT, "SE");
        match ed.ask_point(options)? {
            Answer::Keyword(k) if k == SELECT => found = None,
            Answer::Keyword(k) => adjust(ed, &k)?,
            Answer::Value(_) | Answer::Enter if problem.is_some() => {
                ed.say("Change the scale or spacing first.");
            }
            Answer::Value(_) | Answer::Enter => {
                let sources = current.sources.clone();
                ed.transaction("Hatch", move |tx| {
                    // Behind the first of its boundary objects.
                    let first = sources
                        .iter()
                        .filter_map(|id| tx.document().position(*id).map(|p| (p, *id)))
                        .min()
                        .map(|(_, id)| id);
                    let entity = Entity::Hatch(Box::new(hatch));
                    match first {
                        Some(id) => tx.add_behind(entity, id),
                        None => tx.add(entity),
                    }
                    Ok(())
                })?;
                ed.say("Hatch made.");
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use od_core::hatch::HatchFill;

    use super::*;
    use crate::state::AppState;

    fn rectangle(min: f64, max: f64) -> Entity {
        Entity::Rectangle {
            min: Point::new(min, min),
            max: Point::new(max, max),
        }
    }

    fn entities(state: &AppState) -> Vec<Entity> {
        let objects = state.session().document().objects();
        objects.iter().map(|o| o.entity.clone()).collect()
    }

    fn started(items: &[Entity]) -> AppState {
        started_with(items, |_| {})
    }

    /// As `started`, with the remembered settings changed first.
    fn started_with(items: &[Entity], remembered: impl FnOnce(&mut Memory)) -> AppState {
        let mut state = AppState::new();
        state.defaults().hatch.change(remembered);
        for e in items {
            state.add_entity(e.clone());
        }
        state.select_all();
        state.start_command("Hatch", hatch_objects);
        state.settle_command();
        state
    }

    fn the_hatch(state: &AppState) -> od_core::HatchEntity {
        entities(state)
            .into_iter()
            .find_map(|e| match e {
                Entity::Hatch(h) => Some(*h),
                _ => None,
            })
            .expect("a hatch")
    }

    #[test]
    fn selected_objects_are_hatched_behind_them_in_one_undo_step() {
        let mut state = started(&[rectangle(0.0, 10.0), rectangle(3.0, 7.0)]);
        assert!(state.is_running("Hatch"), "the preview waits for Enter");
        state.answer_enter();
        state.settle_command();
        assert!(!state.is_running("Hatch"));
        let now = entities(&state);
        assert_eq!(now.len(), 3);
        // Drawn behind the first boundary object.
        assert!(matches!(now[0], Entity::Hatch(_)), "{now:?}");
        let h = the_hatch(&state);
        assert_eq!(h.loops.len(), 2);
        assert!(matches!(h.fill, HatchFill::Pattern(_)));
        // Normal style: the island is a hole.
        assert!((h.area(1e-3).unwrap() - (100.0 - 16.0)).abs() < 1e-9);
        state.undo();
        assert_eq!(entities(&state).len(), 2);
        state.redo();
        assert!(matches!(entities(&state)[0], Entity::Hatch(_)));
    }

    #[test]
    fn the_options_change_the_hatch_and_are_remembered() {
        let mut state = started(&[rectangle(0.0, 10.0), rectangle(3.0, 7.0)]);
        state.answer_keyword("Style");
        state.settle_command();
        state.answer_keyword("Ignore");
        state.settle_command();
        state.answer_keyword("Scale");
        state.settle_command();
        assert!(state.answer_typed("4"));
        state.settle_command();
        state.answer_enter();
        state.settle_command();
        let h = the_hatch(&state);
        assert_eq!(h.style, HatchStyle::Ignore);
        assert!((h.area(1e-3).unwrap() - 100.0).abs() < 1e-9);
        let HatchFill::Pattern(p) = &h.fill else {
            panic!("pattern");
        };
        assert_eq!(p.scale_or_spacing, 4.0);
        assert_eq!(state.defaults().hatch.get().scale, 4.0);
        assert_eq!(state.defaults().hatch.get().style, HatchStyle::Ignore);
    }

    #[test]
    fn solid_and_user_defined_fills_come_from_the_pattern_option() {
        let mut state = started(&[rectangle(0.0, 10.0)]);
        state.answer_keyword("Pattern");
        state.settle_command();
        assert!(state.answer_typed("solid"));
        state.settle_command();
        state.answer_enter();
        state.settle_command();
        assert_eq!(the_hatch(&state).fill, HatchFill::Solid);
        // And lines of a spacing, at an angle.
        let mut state = started(&[rectangle(0.0, 10.0)]);
        state.answer_keyword("Pattern");
        state.settle_command();
        assert!(state.answer_typed("u"));
        state.settle_command();
        state.answer_keyword("Spacing");
        state.settle_command();
        assert!(state.answer_typed("2"));
        state.settle_command();
        state.answer_enter();
        state.settle_command();
        let HatchFill::Pattern(p) = the_hatch(&state).fill else {
            panic!("pattern");
        };
        assert_eq!(p.scale_or_spacing, 2.0);
        assert_eq!(p.name, "_USER");
        let _ = PI;
    }

    #[test]
    fn objects_that_do_not_close_are_refused_and_nothing_is_added() {
        let line = Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(5.0, 0.0),
        };
        let mut state = started(std::slice::from_ref(&line));
        // Back at the first prompt, with the problem said.
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("Unable to hatch")));
        assert_eq!(entities(&state), [line]);
        state.cancel();
        assert_eq!(entities(&state).len(), 1);
    }

    #[test]
    fn a_pattern_too_dense_to_draw_cannot_be_accepted() {
        let mut state = started_with(&[rectangle(0.0, 1000.0)], |m| m.scale = 1e-7);
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("The hatch cannot be drawn")));
        state.answer_enter();
        state.settle_command();
        assert!(state.is_running("Hatch"), "still waiting");
        assert_eq!(entities(&state).len(), 1);
        state.cancel();
    }

    fn picking(items: &[Entity]) -> AppState {
        let mut state = AppState::new();
        for e in items {
            state.add_entity(e.clone());
        }
        state.start_command("Hatch", hatch_objects);
        state.settle_command();
        state
    }

    #[test]
    fn an_internal_point_finds_the_boundary_and_the_islands_inside_it() {
        let circle = Entity::Circle {
            center: Point::new(10.0, 10.0),
            radius: 3.0,
        };
        let mut state = picking(&[rectangle(0.0, 20.0), circle]);
        assert!(state.is_running("Hatch"), "waiting for a point");
        state.answer_point(Point::new(1.0, 10.0));
        state.settle_command();
        state.answer_enter();
        state.settle_command();
        let h = the_hatch(&state);
        assert_eq!(h.loops.len(), 2, "the square and the circle as an island");
        assert_eq!(h.seed_points, [Point::new(1.0, 10.0)]);
        assert!((h.area(1e-3).unwrap() - (400.0 - 9.0 * PI)).abs() < 0.1);
        // Drawn behind the objects that bound it.
        assert!(matches!(entities(&state)[0], Entity::Hatch(_)));
        state.undo();
        assert_eq!(entities(&state).len(), 2);
    }

    #[test]
    fn a_point_that_nothing_surrounds_says_so_and_asks_again() {
        let mut state = picking(&[rectangle(0.0, 10.0)]);
        state.answer_point(Point::new(50.0, 50.0));
        state.settle_command();
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("valid hatch boundary not found")));
        assert!(state.is_running("Hatch"));
        assert_eq!(entities(&state).len(), 1);
        state.answer_point(Point::new(5.0, 5.0));
        state.settle_command();
        state.answer_enter();
        state.settle_command();
        assert_eq!(entities(&state).len(), 2);
    }

    #[test]
    fn a_new_hatch_follows_its_boundary_when_that_is_moved_and_undo_puts_both_back() {
        let mut state = started(&[rectangle(0.0, 10.0), rectangle(3.0, 7.0)]);
        state.answer_enter();
        state.settle_command();
        let h = the_hatch(&state);
        assert!(h.associative);
        assert!(h.loops.iter().all(|l| l.sources.len() == 1));
        let boundary = state.session().document().objects()[1].id; // the outer square
        let moved = rectangle(0.0, 10.0).translated(40.0, 0.0);
        state.perform(od_commands::ModifyEntities::new(
            "Move",
            vec![(boundary, moved)],
        ));
        let after = the_hatch(&state);
        assert!(after.associative);
        // The outer loop followed; the island stayed where it was.
        assert!(after.bounds().unwrap().max.x > 49.0);
        state.undo();
        assert_eq!(the_hatch(&state).bounds().unwrap().max.x, 10.0);
        assert!(the_hatch(&state).associative);
    }

    #[test]
    fn with_associativity_off_the_hatch_is_independent() {
        let mut state = started_with(&[rectangle(0.0, 10.0)], |m| m.associative = false);
        state.answer_enter();
        state.settle_command();
        let h = the_hatch(&state);
        assert!(!h.associative && h.loops[0].sources.is_empty());
    }

    #[test]
    fn a_picked_point_hatch_follows_a_boundary_made_of_lines() {
        let line = |a: (f64, f64), b: (f64, f64)| Entity::Line {
            start: Point::new(a.0, a.1),
            end: Point::new(b.0, b.1),
        };
        let walls = [
            line((0.0, 0.0), (10.0, 0.0)),
            line((10.0, 0.0), (10.0, 10.0)),
            line((10.0, 10.0), (0.0, 10.0)),
            line((0.0, 10.0), (0.0, 0.0)),
        ];
        let mut state = picking(&walls);
        state.answer_point(Point::new(5.0, 5.0));
        state.settle_command();
        state.answer_enter();
        state.settle_command();
        assert!(the_hatch(&state).associative);
        // Stretch the right wall and the top corner outward: still closed.
        let ids: Vec<_> = state
            .session()
            .document()
            .objects()
            .iter()
            .filter(|o| matches!(o.entity, Entity::Line { .. }))
            .map(|o| o.id)
            .collect();
        state.perform(od_commands::ModifyEntities::new(
            "Stretch",
            vec![
                (ids[1], line((14.0, 0.0), (14.0, 10.0))),
                (ids[0], line((0.0, 0.0), (14.0, 0.0))),
                (ids[2], line((14.0, 10.0), (0.0, 10.0))),
            ],
        ));
        let h = the_hatch(&state);
        assert!(h.associative, "still follows");
        assert!((h.area(1e-3).unwrap() - 140.0).abs() < 1e-6);
    }
}
