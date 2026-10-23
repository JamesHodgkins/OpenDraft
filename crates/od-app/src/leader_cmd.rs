//! LEADER: points from the arrowhead on (Undo takes the last one back, Enter
//! ends them), then the text in the in-place editor. The leader and its text
//! are two objects added as one undo step. Straight leaders with text or
//! none; spline, tolerance and block annotations are not built.

use od_commands::{Answer, Cancelled, Editor, PointOptions};
use od_core::leader::{Leader, MAX_VERTICES};
use od_core::mtext::MTextAttachment;
use od_core::{DimStyle, Entity, EntityId, MText, Point};

use crate::draw::add;

/// A last segment steeper than this from horizontal gets a hookline.
const HOOK_ANGLE: f64 = 15.0;
/// Points close enough to be the same.
const SAME: f64 = 1e-9;

fn same(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < SAME && (a.y - b.y).abs() < SAME
}

/// The leader through `points` made in `style`, with a hookline when the text
/// is to follow and the last segment is not nearly level. Returns where the
/// text goes (its insertion point and attachment).
fn build(
    style: &DimStyle,
    mut points: Vec<Point>,
    with_text: bool,
) -> (Leader, Point, MTextAttachment) {
    let arrow = style.arrow_size * style.scale;
    let gap = style.text_gap * style.scale;
    let mut hookline = false;
    let n = points.len();
    let (a, b) = (points[n - 2], points[n - 1]);
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let right = dx >= 0.0;
    if with_text && dy.abs().atan2(dx.abs()).to_degrees() > HOOK_ANGLE {
        let end = Point::new(b.x + if right { arrow } else { -arrow }, b.y);
        points.push(end);
        hookline = true;
    }
    let end = points[points.len() - 1];
    let at = Point::new(end.x + if right { gap } else { -gap }, end.y);
    let attachment = if right {
        MTextAttachment::MiddleLeft
    } else {
        MTextAttachment::MiddleRight
    };
    let mut leader = Leader::new(points, arrow);
    leader.hookline = hookline;
    leader.hook_same_direction = false;
    leader.dim_style_name = style.name.clone();
    (leader, at, attachment)
}

fn ghost(style: &DimStyle, points: &[Point], at: Point) -> Vec<Entity> {
    if points.last().is_some_and(|last| same(*last, at)) {
        return Vec::new();
    }
    let mut all = points.to_vec();
    all.push(at);
    vec![Entity::Leader(Box::new(build(style, all, false).0))]
}

pub fn draw_leader(ed: &mut Editor) -> Result<(), Cancelled> {
    let style = ed.read(|doc| doc.dim_style().clone())?;
    let mut points = vec![ed.get_point("Specify leader start point")?];
    loop {
        let (shown, so_far) = (style.clone(), points.clone());
        let options = PointOptions::new("Specify next point (Enter to add text)")
            .base(points[points.len() - 1])
            .keyword("Undo", "U")
            .ghost(move |at| ghost(&shown, &so_far, at));
        match ed.ask_point(options)? {
            Answer::Value(at) if same(at, points[points.len() - 1]) => {
                ed.say("That point is where the last one is.");
            }
            Answer::Value(at) => {
                points.push(at);
                if points.len() >= MAX_VERTICES {
                    break;
                }
            }
            Answer::Keyword(_) => {
                if points.len() > 1 {
                    points.pop();
                } else {
                    ed.say("There is nothing to undo.");
                }
            }
            Answer::Enter if points.len() >= 2 => break,
            Answer::Enter => return Err(Cancelled),
        }
    }
    let text = MText::new(
        Point::default(),
        style.text_height * style.scale,
        "",
        style.text_style,
    );
    let typed = match ed.edit_mtext(text.clone())? {
        Some(t) if !t.contents.trim().is_empty() => Some(t),
        _ => None,
    };
    let (mut leader, at, attachment) = build(&style, points, typed.is_some());
    let Some(mut text) = typed else {
        return add(ed, Entity::Leader(Box::new(leader)), "Leader");
    };
    text.insertion = at;
    text.attachment = attachment;
    let bounds = Entity::MText(Box::new(text.clone())).bounds();
    leader.annotation_size = (bounds.max.y - bounds.min.y, bounds.max.x - bounds.min.x);
    ed.transaction("Leader", move |tx| {
        let id = EntityId(tx.document().next_ids().0);
        tx.add(Entity::MText(Box::new(text)));
        leader.annotation = Some(id);
        tx.add(Entity::Leader(Box::new(leader)));
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use od_commands::Wants;

    use super::*;
    use crate::mtext_editor::Outcome;
    use crate::state::AppState;
    use crate::text_memory::take_turn_for_tests;

    fn start(state: &mut AppState) {
        state.start_command("LEADER", draw_leader);
        state.settle_command();
    }

    fn submit(state: &mut AppState, inputs: &[&str]) {
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
    }

    fn finish_text(state: &mut AppState, typed: &str, outcome: Outcome) {
        let ask = state.waiting().expect("the command is waiting");
        assert_eq!(ask.wants, Wants::MTextEditor);
        state.sync_mtext_editor();
        let active = state.active;
        if let Some(editor) = state.docs[active].mtext_editor.as_mut() {
            editor.insert(typed, 0.0);
        }
        state.finish_mtext_editor(outcome);
        state.settle_command();
    }

    fn leaders(state: &AppState) -> Vec<(EntityId, Leader)> {
        state
            .session()
            .document()
            .objects()
            .iter()
            .filter_map(|o| match &o.entity {
                Entity::Leader(l) => Some((o.id, (**l).clone())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn points_then_text_make_a_leader_with_a_hookline_and_its_text_in_one_undo_step() {
        let _turn = take_turn_for_tests();
        let mut state = AppState::new();
        start(&mut state);
        submit(&mut state, &["0,0", "10,10", ""]);
        finish_text(&mut state, "Note", Outcome::Accept);
        assert!(!state.command_running());
        let found = leaders(&state);
        assert_eq!(found.len(), 1);
        let (id, leader) = &found[0];
        assert_eq!(leader.vertices.len(), 3, "a hook was added");
        assert!(leader.hookline);
        assert_eq!(leader.vertices[2].y, 10.0);
        let doc = state.session().document();
        let text = doc.leader_annotation(*id).expect("the text is there");
        assert_eq!(text.contents, "Note");
        assert!(text.insertion.x > leader.vertices[2].x);
        assert!(doc.validate().is_ok());
        assert_eq!(doc.len(), 2);
        state.undo();
        assert_eq!(state.session().document().len(), 0, "both go at once");
        state.redo();
        assert_eq!(state.session().document().len(), 2);
    }

    #[test]
    fn a_level_last_segment_gets_no_hookline_and_text_to_the_left_goes_left() {
        let _turn = take_turn_for_tests();
        let mut state = AppState::new();
        start(&mut state);
        submit(&mut state, &["0,0", "-10,2", ""]);
        finish_text(&mut state, "Left", Outcome::Accept);
        let (id, leader) = leaders(&state).remove(0);
        assert!(!leader.hookline);
        assert_eq!(leader.vertices.len(), 2);
        let doc = state.session().document();
        let text = doc.leader_annotation(id).unwrap();
        assert!(text.insertion.x < -10.0);
        assert_eq!(text.attachment, MTextAttachment::MiddleRight);
    }

    #[test]
    fn undo_takes_back_a_point_and_empty_text_leaves_a_leader_with_none() {
        let _turn = take_turn_for_tests();
        let mut state = AppState::new();
        start(&mut state);
        submit(&mut state, &["0,0", "5,5", "U", "10,0", ""]);
        finish_text(&mut state, "", Outcome::Accept);
        let (id, leader) = leaders(&state).remove(0);
        assert_eq!(leader.vertices.len(), 2);
        assert_eq!(leader.vertices[1], Point::new(10.0, 0.0));
        assert!(leader.annotation.is_none() && !leader.hookline);
        assert!(state.session().document().leader_annotation(id).is_none());
        assert_eq!(state.session().document().len(), 1);
    }

    #[test]
    fn escape_in_the_editor_or_enter_with_one_point_adds_nothing() {
        let _turn = take_turn_for_tests();
        let mut state = AppState::new();
        start(&mut state);
        submit(&mut state, &["0,0", "5,5", ""]);
        finish_text(&mut state, "x", Outcome::Cancel);
        assert!(leaders(&state).is_empty());
        start(&mut state);
        submit(&mut state, &["0,0", ""]);
        assert!(!state.command_running());
        assert!(leaders(&state).is_empty());
    }

    #[test]
    fn a_repeated_point_is_ignored() {
        let _turn = take_turn_for_tests();
        let mut state = AppState::new();
        start(&mut state);
        submit(&mut state, &["0,0", "0,0", "10,10", ""]);
        finish_text(&mut state, "x", Outcome::Accept);
        let (_, leader) = leaders(&state).remove(0);
        assert_eq!(
            leader.vertices[..2],
            [Point::new(0.0, 0.0), Point::new(10.0, 10.0)]
        );
    }
}
