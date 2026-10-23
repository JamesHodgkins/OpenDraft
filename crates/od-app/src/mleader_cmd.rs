//! MLEADER: the arrowhead, then the leader's other points up to the
//! landing, then the text in the in-place editor. The multileader is made in
//! the current multileader style.

use od_commands::{Answer, Cancelled, Editor, PointOptions};
use od_core::mleader::{MLeader, MLeaderContent, MLeaderLineType, MLeaderStyle};
use od_core::{Entity, Point};

use crate::draw::add;

/// Points close enough to be the same.
const SAME: f64 = 1e-9;

/// What the ghost shows when the style has no default text.
const PLACEHOLDER: &str = "Text";

fn same(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < SAME && (a.y - b.y).abs() < SAME
}

/// The multileader the ghost shows: `points` and the pointer as the last one.
fn ghost(style: &MLeaderStyle, points: &[Point], at: Point) -> Vec<Entity> {
    let mut all = points.to_vec();
    if all.last().is_some_and(|last| same(*last, at)) {
        return Vec::new();
    }
    all.push(at);
    let text = match style.content.default_text.trim() {
        "" => PLACEHOLDER,
        text => text,
    };
    MLeader::build(style, all, text)
        .map(|m| Entity::MLeader(Box::new(m)))
        .into_iter()
        .collect()
}

pub fn draw_mleader(ed: &mut Editor) -> Result<(), Cancelled> {
    let style = ed.read(|doc| {
        doc.mleader_style(doc.current_mleader_style())
            .cloned()
            .unwrap_or_else(MLeaderStyle::standard)
    })?;
    if style.leader_type == MLeaderLineType::Spline {
        ed.say("Spline leaders are not supported yet; make a style with straight leaders current.");
        return Ok(());
    }
    let mut points = vec![ed.get_point("Specify leader arrowhead location")?];
    let most = style.max_points.map_or(usize::MAX, |n| n as usize);
    loop {
        // The point after this one would still be a leader point when the
        // style allows more than the arrowhead and this one.
        let more = points.len() + 1 < most;
        let message = if more {
            "Specify next point (Enter to finish)"
        } else {
            "Specify leader landing location"
        };
        let (shown, so_far) = (style.clone(), points.clone());
        let options = PointOptions::new(message)
            .base(points[points.len() - 1])
            .ghost(move |at| ghost(&shown, &so_far, at));
        match ed.ask_point(options)? {
            Answer::Value(at) if same(at, points[points.len() - 1]) => {
                ed.say("That point is where the last one is.");
            }
            Answer::Value(at) => {
                points.push(at);
                if !more {
                    break;
                }
            }
            Answer::Enter if points.len() >= 2 => break,
            _ => return Err(Cancelled),
        }
    }
    let default = style.content.default_text.clone();
    let Some(mut made) = MLeader::build(&style, points, &default) else {
        return Err(Cancelled);
    };
    if let MLeaderContent::MText(t) = &made.content {
        match ed.edit_mtext(t.text.clone())? {
            Some(text) if !text.contents.trim().is_empty() => {
                if let MLeaderContent::MText(t) = &mut made.content {
                    t.text = text;
                }
            }
            _ => return Ok(()),
        }
    }
    add(ed, Entity::MLeader(Box::new(made)), "Multileader")
}

#[cfg(test)]
mod tests {
    use od_commands::Wants;

    use super::*;
    use crate::mtext_editor::Outcome;
    use crate::state::AppState;
    use crate::text_memory::take_turn_for_tests;

    fn start(state: &mut AppState) {
        state.start_command("MLEADER", draw_mleader);
        state.settle_command();
    }

    fn submit(state: &mut AppState, inputs: &[&str]) {
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
    }

    fn made(state: &AppState) -> Vec<MLeader> {
        state
            .session()
            .document()
            .objects()
            .iter()
            .filter_map(|o| match &o.entity {
                Entity::MLeader(m) => Some((**m).clone()),
                _ => None,
            })
            .collect()
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

    #[test]
    fn two_points_then_the_editor_make_one_multileader_in_one_undo_step() {
        let _turn = take_turn_for_tests();
        let mut state = AppState::new();
        start(&mut state);
        submit(&mut state, &["0,0", "20,10"]);
        finish_text(&mut state, "Note", Outcome::Accept);
        assert!(!state.command_running());
        let found = made(&state);
        assert_eq!(found.len(), 1);
        let m = &found[0];
        assert_eq!(m.lines[0], [Point::new(0.0, 0.0), Point::new(20.0, 10.0)]);
        assert_eq!(m.text().unwrap().text.contents, "Note");
        assert_eq!(m.style_name, "Standard");
        assert!(state.session().document().validate().is_ok());
        state.undo();
        assert!(made(&state).is_empty());
        state.redo();
        assert_eq!(made(&state).len(), 1);
    }

    #[test]
    fn cancelling_the_editor_or_leaving_it_empty_adds_nothing() {
        let _turn = take_turn_for_tests();
        let mut state = AppState::new();
        start(&mut state);
        submit(&mut state, &["0,0", "20,10"]);
        finish_text(&mut state, "", Outcome::Accept);
        assert!(made(&state).is_empty());
        start(&mut state);
        submit(&mut state, &["0,0", "20,10"]);
        finish_text(&mut state, "Note", Outcome::Cancel);
        assert!(made(&state).is_empty());
    }

    #[test]
    fn a_style_allowing_more_points_takes_them_until_enter() {
        let _turn = take_turn_for_tests();
        let mut state = AppState::new();
        let mut style = state.mleader_style();
        style.max_points = None;
        state.modify_mleader_style(style.id, style);
        start(&mut state);
        submit(&mut state, &["0,0", "5,5", "5,5", "20,10", ""]);
        finish_text(&mut state, "Note", Outcome::Accept);
        let found = made(&state);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].lines[0].len(), 3, "the repeated point was ignored");
    }

    #[test]
    fn a_spline_style_is_refused_by_name() {
        let mut state = AppState::new();
        let mut style = state.mleader_style();
        style.leader_type = MLeaderLineType::Spline;
        state.modify_mleader_style(style.id, style);
        start(&mut state);
        assert!(!state.command_running());
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("Spline leaders are not supported yet")));
    }

    #[test]
    fn restyling_follows_a_style_change_and_comes_back_on_undo() {
        let _turn = take_turn_for_tests();
        let mut state = AppState::new();
        start(&mut state);
        submit(&mut state, &["0,0", "20,10"]);
        finish_text(&mut state, "Note", Outcome::Accept);
        let mut style = state.mleader_style();
        style.arrow_size = 7.0;
        state.modify_mleader_style(style.id, style);
        assert_eq!(made(&state)[0].look.arrow_size, 7.0);
        assert!(!state.can_delete_mleader_style(od_core::MLeaderStyleId(0)));
        state.undo();
        assert_eq!(made(&state)[0].look.arrow_size, 2.5);
    }
}
