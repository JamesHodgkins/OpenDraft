//! Editing objects by their grips. A press on a grip starts this as a command,
//! so everything a command point prompt gives comes with it: snapping, typed
//! coordinates (relative to the base point), a live preview and Esc.
//! `Entity::grips` and `with_grip_moved` in `od_core` do the stretching.
//!
//! The first edit is a stretch of the pressed grip (or of every grip picked
//! with Shift, together). Enter cycles the edit through Move, Rotate, Scale
//! and Mirror of the whole objects, as in AutoCAD. The options are Base point,
//! Copy (leave the original and add an edited copy, again and again), Undo
//! (take back the last copy) and eXit. A single grip on the end of a line, an
//! arc or an open polyline can also Lengthen (Ctrl, or the keyword, switches
//! between Stretch and Lengthen): the end slides along the object to the
//! pointer instead of leaving it.

use od_canvas::GripHit;
use od_commands::{Answer, Cancelled, Editor, NumberOptions, PointOptions};
use od_core::{Attributes, Entity, EntityId, Point};

use crate::state::AppState;

/// Scale factors at or below this are refused.
const MIN_FACTOR: f64 = 1e-9;

impl AppState {
    /// Start editing the grips that were pressed on the canvas, the first one
    /// being the base point.
    pub fn start_grip_edit(&mut self, hits: Vec<GripHit>) {
        if hits.is_empty() {
            return;
        }
        self.start_command("Stretch", move |ed| edit_grips(ed, &hits));
    }

    /// Ctrl while editing an end grip: switch between Stretch and Lengthen.
    /// Does nothing when the waiting prompt offers neither.
    pub fn cycle_grip_option(&mut self) {
        let label = self.waiting().and_then(|ask| {
            ask.keywords
                .iter()
                .map(|k| k.label.as_str())
                .find(|l| matches!(*l, "Lengthen" | "Stretch"))
                .map(str::to_owned)
        });
        if let Some(label) = label {
            self.answer_keyword(&label);
        }
    }

    /// Esc with grips picked but not used: forget them.
    pub(crate) fn clear_warm_grips(&mut self) -> bool {
        let view = &mut self.docs[self.active].view;
        let had = view.has_warm_grips();
        view.clear_warm_grips();
        had
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Stretch,
    /// An end grip slides its end along the object (`Entity::lengthened_to`).
    Lengthen,
    Move,
    Rotate,
    Scale,
    Mirror,
}

impl Mode {
    fn next(self) -> Mode {
        match self {
            Mode::Stretch | Mode::Lengthen => Mode::Move,
            Mode::Move => Mode::Rotate,
            Mode::Rotate => Mode::Scale,
            Mode::Scale => Mode::Mirror,
            Mode::Mirror => Mode::Stretch,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Mode::Stretch => "Stretch",
            Mode::Lengthen => "Lengthen",
            Mode::Move => "Move",
            Mode::Rotate => "Rotate",
            Mode::Scale => "Scale",
            Mode::Mirror => "Mirror",
        }
    }
}

/// An object being edited and the grips of it that are hot.
#[derive(Clone)]
struct Hot {
    id: EntityId,
    entity: Entity,
    attrs: Attributes,
    grips: Vec<usize>,
}

/// What the user gave for the current mode.
#[derive(Clone, Copy)]
enum Given {
    /// A point: the stretch point, move point or second mirror point.
    At(Point),
    /// A rotation angle in radians.
    Angle(f64),
    /// A scale factor.
    Factor(f64),
}

/// What `hot` becomes in `mode` for what was given, measured from `base`.
fn result(mode: Mode, hot: &Hot, base: Point, given: Given) -> Option<Entity> {
    match (mode, given) {
        (Mode::Stretch, Given::At(at)) => {
            let (dx, dy) = (at.x - base.x, at.y - base.y);
            let original = hot.entity.grips();
            let mut made = hot.entity.clone();
            for &index in &hot.grips {
                let from = original.get(index)?.at;
                made = made.with_grip_moved(index, Point::new(from.x + dx, from.y + dy))?;
            }
            Some(made)
        }
        (Mode::Lengthen, Given::At(at)) => {
            let end = hot.entity.end_grip(*single(hot)?)?;
            hot.entity.lengthened_to(end, at)
        }
        (Mode::Move, Given::At(at)) => Some(hot.entity.translated(at.x - base.x, at.y - base.y)),
        (Mode::Rotate, Given::Angle(angle)) => Some(hot.entity.rotated(base, angle)),
        (Mode::Scale, Given::Factor(factor)) => {
            (factor.is_finite() && factor > MIN_FACTOR && hot.entity.scale_refusal().is_none())
                .then(|| hot.entity.scaled(base, factor))
        }
        (Mode::Mirror, Given::At(at)) => ((at.x - base.x).hypot(at.y - base.y) > MIN_FACTOR
            && hot.entity.mirror_refusal().is_none())
        .then(|| hot.entity.mirrored(base, at)),
        _ => None,
    }
}

/// The grip of `hot`, when it has just the one.
fn single(hot: &Hot) -> Option<&usize> {
    match hot.grips.as_slice() {
        [one] => Some(one),
        _ => None,
    }
}

/// Whether the grips being edited are one end grip of one object, which can
/// be lengthened.
fn lengthenable(hot: &[Hot]) -> bool {
    matches!(hot, [h] if single(h).is_some_and(|i| h.entity.end_grip(*i).is_some()))
}

/// What the pointer at `at` stands for in `mode`.
fn from_pointer(mode: Mode, base: Point, at: Point) -> Given {
    match mode {
        Mode::Rotate => Given::Angle((at.y - base.y).atan2(at.x - base.x)),
        Mode::Scale => Given::Factor((at.x - base.x).hypot(at.y - base.y)),
        _ => Given::At(at),
    }
}

/// The answer to one prompt.
enum Reply {
    Given(Given),
    Keyword(String),
    Enter,
}

fn ask(
    ed: &Editor,
    mode: Mode,
    base: Point,
    hot: &[Hot],
    copy: bool,
    copies: usize,
) -> Result<Reply, Cancelled> {
    let ghosts = hot.to_vec();
    let ghost = move |at: Point| -> Vec<Entity> {
        ghosts
            .iter()
            .filter_map(|h| result(mode, h, base, from_pointer(mode, base, at)))
            .collect()
    };
    let message = match (mode, copy) {
        (Mode::Stretch, false) => "Specify stretch point",
        (Mode::Stretch, true) => "Specify stretch point (copy)",
        (Mode::Lengthen, false) => "Specify new end point",
        (Mode::Lengthen, true) => "Specify new end point (copy)",
        (Mode::Move, false) => "Specify move point",
        (Mode::Move, true) => "Specify move point (copy)",
        (Mode::Rotate, false) => "Specify rotation angle",
        (Mode::Rotate, true) => "Specify rotation angle (copy)",
        (Mode::Scale, false) => "Specify scale factor",
        (Mode::Scale, true) => "Specify scale factor (copy)",
        (Mode::Mirror, false) => "Specify second point of mirror line",
        (Mode::Mirror, true) => "Specify second point of mirror line (copy)",
    };
    macro_rules! options {
        ($options:expr) => {{
            let mut options = $options.base(base).ghost(ghost).keyword("Base point", "B");
            if !copy {
                options = options.keyword("Copy", "C");
            }
            if copies > 0 {
                options = options.keyword("Undo", "U");
            }
            // Ctrl answers this one (see `AppState::cycle_grip_option`).
            if lengthenable(&hot) {
                options = match mode {
                    Mode::Stretch => options.keyword("Lengthen", "L"),
                    Mode::Lengthen => options.keyword("Stretch", "S"),
                    _ => options,
                };
            }
            options.keyword("eXit", "X")
        }};
    }
    Ok(match mode {
        Mode::Rotate => match ed.ask_angle(options!(NumberOptions::new(message)))? {
            Answer::Value(angle) => Reply::Given(Given::Angle(angle)),
            Answer::Keyword(k) => Reply::Keyword(k),
            Answer::Enter => Reply::Enter,
        },
        Mode::Scale => match ed.ask_distance(options!(NumberOptions::new(message)))? {
            Answer::Value(factor) => Reply::Given(Given::Factor(factor)),
            Answer::Keyword(k) => Reply::Keyword(k),
            Answer::Enter => Reply::Enter,
        },
        _ => match ed.ask_point(options!(
            PointOptions::new(message).dimensions(od_commands::DynamicDimensions::Polar)
        ))? {
            Answer::Value(at) => Reply::Given(Given::At(at)),
            Answer::Keyword(k) => Reply::Keyword(k),
            Answer::Enter => Reply::Enter,
        },
    })
}

/// Read the objects the hot grips belong to, one entry per object.
fn read_hot(ed: &Editor, hits: &[GripHit]) -> Result<Vec<Hot>, Cancelled> {
    let hits = hits.to_vec();
    ed.read(move |doc| {
        let mut found: Vec<Hot> = Vec::new();
        for hit in &hits {
            if let Some(known) = found.iter_mut().find(|h| h.id == hit.id) {
                known.grips.push(hit.index);
            } else if let Some(object) = doc.object(hit.id) {
                found.push(Hot {
                    id: hit.id,
                    entity: object.entity.clone(),
                    attrs: object.attrs.clone(),
                    grips: vec![hit.index],
                });
            }
        }
        found
    })
}

/// The command: edit the hot grips until the user places the result, or, in
/// Copy mode, until they exit.
fn edit_grips(ed: &mut Editor, hits: &[GripHit]) -> Result<(), Cancelled> {
    let hot = read_hot(ed, hits)?;
    let Some(base_grip) = hits.first().and_then(|h| {
        hot.iter()
            .find(|o| o.id == h.id)?
            .entity
            .grips()
            .get(h.index)
            .copied()
    }) else {
        return Ok(());
    };
    let mut mode = Mode::Stretch;
    let mut base = base_grip.at;
    let (mut copy, mut copies) = (false, 0_usize);
    loop {
        let given = match ask(ed, mode, base, &hot, copy, copies)? {
            Reply::Enter => {
                mode = mode.next();
                continue;
            }
            Reply::Keyword(k) => {
                match k.as_str() {
                    "Base point" => {
                        base = ed.get_point(PointOptions::new("Specify base point").base(base))?;
                    }
                    "Copy" => copy = true,
                    "Lengthen" => mode = Mode::Lengthen,
                    "Stretch" => mode = Mode::Stretch,
                    "Undo" => {
                        ed.undo()?;
                        copies = copies.saturating_sub(1);
                    }
                    _ => return Ok(()),
                }
                continue;
            }
            Reply::Given(given) => given,
        };
        let changes: Vec<(Hot, Entity)> = hot
            .iter()
            .filter_map(|h| Some((h.clone(), result(mode, h, base, given)?)))
            .collect();
        if changes.is_empty() {
            ed.say("The objects cannot be changed that way.");
            if copy {
                continue;
            }
            return Ok(());
        }
        let name = mode.name();
        ed.transaction(name, move |tx| {
            for (h, made) in changes {
                if copy {
                    tx.add_with(vec![(made, h.attrs)]);
                } else {
                    tx.modify(h.id, move |_| made);
                }
            }
            Ok(())
        })?;
        if !copy {
            return Ok(());
        }
        copies += 1;
    }
}

#[cfg(test)]
mod tests {
    use od_core::Point;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(a: Point, b: Point) -> Entity {
        Entity::Line { start: a, end: b }
    }

    fn begin(objects: &[Entity], hits: &[(usize, usize)]) -> AppState {
        let mut state = AppState::new();
        for e in objects {
            state.add_entity(e.clone());
        }
        let ids: Vec<EntityId> = state
            .session()
            .document
            .objects()
            .iter()
            .map(|o| o.id)
            .collect();
        state.select_ids(ids.clone(), false);
        let hits = hits
            .iter()
            .map(|&(object, index)| GripHit {
                id: ids[object],
                index,
            })
            .collect();
        state.start_grip_edit(hits);
        state.settle_command();
        state
    }

    fn submit(state: &mut AppState, text: &str) {
        state.cmd_submit(text);
        state.settle_command();
    }

    fn entities(state: &AppState) -> Vec<Entity> {
        let objects = state.session().document().objects();
        objects.iter().map(|o| o.entity.clone()).collect()
    }

    fn entity(state: &AppState) -> Entity {
        entities(state)[0].clone()
    }

    #[test]
    fn dragging_an_end_grip_stretches_the_line_in_one_undo_step() {
        let mut state = begin(&[line(p(0.0, 0.0), p(10.0, 0.0))], &[(0, 1)]);
        assert!(state.command_running());
        assert!(state.cmd_prompt().text.starts_with("Specify stretch point"));
        state.answer_point(p(7.0, 3.0));
        state.settle_command();
        assert!(!state.command_running());
        assert_eq!(entity(&state), line(p(0.0, 0.0), p(7.0, 3.0)));
        state.undo();
        assert_eq!(entity(&state), line(p(0.0, 0.0), p(10.0, 0.0)));
    }

    #[test]
    fn typed_coordinates_are_relative_to_the_grip() {
        let mut state = begin(&[line(p(0.0, 0.0), p(10.0, 0.0))], &[(0, 1)]);
        state.cmd_submit("@0,5");
        state.settle_command();
        assert_eq!(entity(&state), line(p(0.0, 0.0), p(10.0, 5.0)));
    }

    #[test]
    fn the_middle_grip_moves_the_whole_object_and_a_quadrant_resizes_a_circle() {
        let mut state = begin(&[line(p(0.0, 0.0), p(10.0, 0.0))], &[(0, 2)]);
        state.answer_point(p(5.0, 4.0));
        state.settle_command();
        assert_eq!(entity(&state), line(p(0.0, 4.0), p(10.0, 4.0)));
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 2.0,
        };
        let mut state = begin(&[circle], &[(0, 1)]);
        state.answer_point(p(0.0, 6.0));
        state.settle_command();
        assert_eq!(
            entity(&state),
            Entity::Circle {
                center: p(0.0, 0.0),
                radius: 6.0
            }
        );
    }

    #[test]
    fn escape_leaves_the_object_and_keeps_it_selected() {
        let mut state = begin(&[line(p(0.0, 0.0), p(10.0, 0.0))], &[(0, 0)]);
        state.cancel();
        assert!(!state.command_running());
        assert_eq!(entity(&state), line(p(0.0, 0.0), p(10.0, 0.0)));
        assert_eq!(
            state.selection().len(),
            1,
            "the first Esc only ends the edit"
        );
    }

    #[test]
    fn a_drag_that_leaves_nothing_sensible_is_refused_and_changes_nothing() {
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 2.0,
        };
        let mut state = begin(std::slice::from_ref(&circle), &[(0, 1)]);
        state.answer_point(p(0.0, 0.0));
        state.settle_command();
        assert_eq!(entity(&state), circle);
        assert!(state
            .cmdline
            .log
            .last()
            .unwrap()
            .contains("cannot be changed"));
    }

    #[test]
    fn several_grips_stretch_together_by_the_same_amount() {
        // Two vertices of a polyline, hot together, move as one.
        let poly = Entity::Polyline {
            points: vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)],
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        };
        let mut state = begin(&[poly], &[(0, 1), (0, 2)]);
        // The first hit (vertex 1, at (10, 0)) is the base point.
        state.answer_point(p(14.0, 0.0));
        state.settle_command();
        let Entity::Polyline { points, .. } = entity(&state) else {
            panic!()
        };
        assert_eq!(
            points,
            vec![p(0.0, 0.0), p(14.0, 0.0), p(14.0, 10.0), p(0.0, 10.0)]
        );
        // Grips on two different objects move together too.
        let two = [
            line(p(0.0, 0.0), p(10.0, 0.0)),
            line(p(0.0, 5.0), p(10.0, 5.0)),
        ];
        let mut state = begin(&two, &[(0, 1), (1, 1)]);
        state.cmd_submit("@3,2");
        state.settle_command();
        assert_eq!(
            entities(&state),
            vec![
                line(p(0.0, 0.0), p(13.0, 2.0)),
                line(p(0.0, 5.0), p(13.0, 7.0))
            ]
        );
    }

    #[test]
    fn copy_leaves_the_original_and_adds_a_stretched_copy_each_time() {
        let mut state = begin(&[line(p(0.0, 0.0), p(10.0, 0.0))], &[(0, 1)]);
        state.cmd_submit("c");
        state.settle_command();
        assert!(state.cmd_prompt().text.contains("(copy)"));
        state.cmd_submit("10,5");
        state.settle_command();
        state.cmd_submit("10,8");
        state.settle_command();
        assert!(state.command_running(), "Copy keeps asking");
        let all = entities(&state);
        assert_eq!(all.len(), 3);
        assert_eq!(
            all[0],
            line(p(0.0, 0.0), p(10.0, 0.0)),
            "the original stays"
        );
        assert_eq!(all[1], line(p(0.0, 0.0), p(10.0, 5.0)));
        assert_eq!(all[2], line(p(0.0, 0.0), p(10.0, 8.0)));
        // Undo takes back the last copy.
        state.cmd_submit("u");
        state.settle_command();
        assert_eq!(entities(&state).len(), 2);
        state.cmd_submit("x");
        state.settle_command();
        assert!(!state.command_running());
    }

    #[test]
    fn base_point_changes_what_the_displacement_is_measured_from() {
        let mut state = begin(&[line(p(0.0, 0.0), p(10.0, 0.0))], &[(0, 1)]);
        state.cmd_submit("b");
        state.settle_command();
        state.cmd_submit("100,100");
        state.settle_command();
        // From the new base (100, 100) to (103, 104): a shift of (3, 4).
        state.cmd_submit("103,104");
        state.settle_command();
        assert_eq!(entity(&state), line(p(0.0, 0.0), p(13.0, 4.0)));
    }

    #[test]
    fn enter_cycles_through_move_rotate_scale_and_mirror() {
        let mut state = begin(&[line(p(0.0, 0.0), p(10.0, 0.0))], &[(0, 1)]);
        let prompt = |s: &mut AppState| s.cmd_prompt().text.to_string();
        state.cmd_submit("");
        state.settle_command();
        assert!(prompt(&mut state).starts_with("Specify move point"));
        state.cmd_submit("");
        state.settle_command();
        assert!(prompt(&mut state).starts_with("Specify rotation angle"));
        state.cmd_submit("");
        state.settle_command();
        assert!(prompt(&mut state).starts_with("Specify scale factor"));
        state.cmd_submit("");
        state.settle_command();
        assert!(prompt(&mut state).starts_with("Specify second point of mirror line"));
        state.cmd_submit("");
        state.settle_command();
        assert!(
            prompt(&mut state).starts_with("Specify stretch point"),
            "round again"
        );
    }

    #[test]
    fn move_rotate_scale_and_mirror_edit_the_whole_object_about_the_base_point() {
        // Base point: the grip at the end, (10, 0).
        let mut state = begin(&[line(p(0.0, 0.0), p(10.0, 0.0))], &[(0, 1)]);
        submit(&mut state, "");
        submit(&mut state, "3,4");
        assert_eq!(
            entity(&state),
            line(p(-7.0, 4.0), p(3.0, 4.0)),
            "moved by the base point's displacement"
        );

        let mut state = begin(&[line(p(0.0, 0.0), p(10.0, 0.0))], &[(0, 1)]);
        submit(&mut state, "");
        submit(&mut state, "");
        submit(&mut state, "90");
        let Entity::Line { start, end } = entity(&state) else {
            panic!()
        };
        // Turned a quarter about (10, 0): the start lands at (10, -10).
        assert!((start.x - 10.0).abs() < 1e-9 && (start.y + 10.0).abs() < 1e-9);
        assert!((end.x - 10.0).abs() < 1e-9 && end.y.abs() < 1e-9);

        let mut state = begin(&[line(p(0.0, 0.0), p(10.0, 0.0))], &[(0, 1)]);
        for _ in 0..3 {
            submit(&mut state, "");
        }
        submit(&mut state, "2");
        assert_eq!(
            entity(&state),
            line(p(-10.0, 0.0), p(10.0, 0.0)),
            "doubled about (10, 0)"
        );

        let mut state = begin(&[line(p(0.0, 0.0), p(10.0, 0.0))], &[(0, 1)]);
        for _ in 0..4 {
            submit(&mut state, "");
        }
        // Mirror in the vertical line through the base point.
        submit(&mut state, "10,5");
        assert_eq!(entity(&state), line(p(20.0, 0.0), p(10.0, 0.0)));
    }

    #[test]
    fn ctrl_switches_an_end_grip_between_stretch_and_lengthen() {
        let mut state = begin(&[line(p(0.0, 0.0), p(10.0, 0.0))], &[(0, 1)]);
        assert!(state.cmd_prompt().text.starts_with("Specify stretch point"));
        state.cycle_grip_option();
        state.settle_command();
        assert!(state.cmd_prompt().text.starts_with("Specify new end point"));
        submit(&mut state, "14,5");
        // The end slides along the line instead of moving off it.
        assert_eq!(entities(&state), [line(p(0.0, 0.0), p(14.0, 0.0))]);
        state.undo();
        assert_eq!(entities(&state), [line(p(0.0, 0.0), p(10.0, 0.0))]);

        // And back again.
        let mut state = begin(&[line(p(0.0, 0.0), p(10.0, 0.0))], &[(0, 0)]);
        state.cycle_grip_option();
        state.settle_command();
        state.cycle_grip_option();
        state.settle_command();
        assert!(state.cmd_prompt().text.starts_with("Specify stretch point"));
        submit(&mut state, "-3,4");
        assert_eq!(entities(&state), [line(p(-3.0, 4.0), p(10.0, 0.0))]);
    }

    #[test]
    fn lengthen_is_for_one_end_grip_only() {
        // The middle grip of a line, and two grips together, offer nothing.
        for hits in [vec![(0, 2)], vec![(0, 0), (0, 1)]] {
            let mut state = begin(&[line(p(0.0, 0.0), p(10.0, 0.0))], &hits);
            state.cycle_grip_option();
            state.settle_command();
            assert!(state.cmd_prompt().text.starts_with("Specify stretch point"));
        }
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 1.0,
        };
        let mut state = begin(&[circle], &[(0, 1)]);
        state.cycle_grip_option();
        state.settle_command();
        assert!(state.cmd_prompt().text.starts_with("Specify stretch point"));
    }

    #[test]
    fn an_arc_end_grip_lengthens_round_its_circle() {
        let arc = Entity::Arc {
            center: p(0.0, 0.0),
            radius: 2.0,
            start: 0.0,
            end: std::f64::consts::FRAC_PI_2,
        };
        let mut state = begin(&[arc], &[(0, 1)]);
        state.cycle_grip_option();
        state.settle_command();
        submit(&mut state, "-5,5");
        let Entity::Arc { radius, end, .. } = entities(&state)[0] else {
            panic!("an arc")
        };
        assert!((radius - 2.0).abs() < 1e-9);
        assert!((end - 0.75 * std::f64::consts::PI).abs() < 1e-9, "{end}");
    }
}
