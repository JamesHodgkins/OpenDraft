//! MTEXT: a box dragged out with two corners, then the in-place editor for
//! the text. MTEDIT opens the editor on a text already in the drawing.

use od_commands::{Answer, Cancelled, Editor, NumberOptions, Options, Picked, PointOptions};
use od_core::mtext::{place_in_corners, MTextAttachment};
use od_core::{Entity, MText, Point};

use crate::draw::{add, MIN_SIZE};
use crate::text_memory::{memory, remember};

/// Justification names and the attachments they stand for.
const JUSTIFY: [(&str, MTextAttachment); 9] = [
    ("TL", MTextAttachment::TopLeft),
    ("TC", MTextAttachment::TopCenter),
    ("TR", MTextAttachment::TopRight),
    ("ML", MTextAttachment::MiddleLeft),
    ("MC", MTextAttachment::MiddleCenter),
    ("MR", MTextAttachment::MiddleRight),
    ("BL", MTextAttachment::BottomLeft),
    ("BC", MTextAttachment::BottomCenter),
    ("BR", MTextAttachment::BottomRight),
];

/// The outline of the box for a drag from `first` to `at`.
fn box_outline(first: Point, at: Point, rotation: f64, width: Option<f64>) -> Entity {
    let (sin, cos) = rotation.sin_cos();
    let (dx, dy) = (at.x - first.x, at.y - first.y);
    let (mut x, y) = (dx * cos + dy * sin, -dx * sin + dy * cos);
    if let Some(width) = width {
        x = if x < 0.0 { -width } else { width };
    }
    let corner = |along: f64, across: f64| {
        Point::new(
            first.x + along * cos - across * sin,
            first.y + along * sin + across * cos,
        )
    };
    Entity::Polyline {
        points: vec![
            corner(0.0, 0.0),
            corner(x, 0.0),
            corner(x, y),
            corner(0.0, y),
        ],
        closed: true,
        bulges: Vec::new(),
        widths: Vec::new(),
    }
}

/// How tall the dragged box is, measured along the text's up direction.
fn drag_height(first: Point, second: Point, rotation: f64) -> f64 {
    let (sin, cos) = rotation.sin_cos();
    let (dx, dy) = (second.x - first.x, second.y - first.y);
    (-dx * sin + dy * cos).abs()
}

/// MTEXT: the first corner, the opposite corner (with Height, Justify,
/// Rotation and Width options), then the text. Enter on an empty line adds
/// nothing.
pub fn draw_mtext(ed: &mut Editor) -> Result<(), Cancelled> {
    let first = ed.get_point("Specify first corner")?;
    let (mut height, mut attachment) = (memory().height, memory().attachment);
    let (mut rotation, mut width) = (0.0, None);
    let second = loop {
        let options = PointOptions::new(format!("Specify opposite corner or (height {height})"))
            .keyword("Height", "H")
            .keyword("Justify", "J")
            .keyword("Rotation", "R")
            .keyword("Width", "W")
            .ghost(move |at| vec![box_outline(first, at, rotation, width)]);
        match ed.ask_corner(first, options)? {
            Answer::Value(at) => break at,
            Answer::Keyword(k) => match k.as_str() {
                "Height" => {
                    let options = NumberOptions::new("Specify height").default(height);
                    let value = ed.get_distance(options)?;
                    if value > MIN_SIZE {
                        height = value;
                        remember(|m| m.height = value);
                    } else {
                        ed.say("Requires a positive value.");
                    }
                }
                "Justify" => {
                    let mut options = Options::new("Enter justification");
                    for (name, _) in JUSTIFY {
                        options = options.keyword(name, name);
                    }
                    let name = ed.get_keyword(options)?;
                    if let Some((_, chosen)) = JUSTIFY.iter().find(|(n, _)| *n == name) {
                        attachment = *chosen;
                        remember(|m| m.attachment = *chosen);
                    }
                }
                "Rotation" => {
                    rotation = ed.get_angle(
                        NumberOptions::new("Specify rotation angle").default(rotation),
                    )?;
                }
                _ => {
                    let options = NumberOptions::new("Specify width");
                    let value = ed.get_distance(options)?;
                    width = (value > MIN_SIZE).then_some(value);
                }
            },
            Answer::Enter => return Err(Cancelled),
        }
    };
    let (insertion, drag_width) = place_in_corners(first, second, rotation, attachment);
    let style = ed.read(|doc| doc.current_text_style())?;
    let mut text = MText::new(insertion, height, "", style);
    text.direction = Point::new(rotation.cos(), rotation.sin());
    text.attachment = attachment;
    text.reference_width = width.unwrap_or(drag_width);
    text.reference_height = drag_height(first, second, rotation);
    match ed.edit_mtext(text)? {
        Some(text) if !text.contents.trim().is_empty() => {
            add(ed, Entity::MText(Box::new(text)), "Multiline Text")
        }
        _ => Ok(()),
    }
}

/// The text object to edit: the one preselected, or one the user clicks.
fn pick_text(ed: &mut Editor) -> Result<od_core::EntityId, Cancelled> {
    let is_text =
        |ed: &Editor, id| ed.read(move |doc| matches!(doc.get(id), Some(Entity::MText(_))));
    if let [id] = ed.pickset() {
        let id = *id;
        if is_text(ed, id)? {
            return Ok(id);
        }
    }
    loop {
        let options = Options::<Picked>::new("Select an MTEXT object");
        let Answer::Value(picked) = ed.ask_object(options)? else {
            return Err(Cancelled);
        };
        if is_text(ed, picked.id)? {
            return Ok(picked.id);
        }
        ed.say("That is not a multiline text object.");
    }
}

/// MTEDIT: open the editor on an existing multiline text. Accepting a
/// changed text replaces it in one undo step; Esc or an unchanged text
/// leaves the drawing as it was.
pub fn edit_mtext_object(ed: &mut Editor) -> Result<(), Cancelled> {
    let id = pick_text(ed)?;
    let original = ed
        .read(move |doc| match doc.get(id) {
            Some(Entity::MText(m)) => Some((**m).clone()),
            _ => None,
        })?
        .ok_or(Cancelled)?;
    let Some(edited) = ed.edit_mtext_entity(id, original.clone())? else {
        return Ok(());
    };
    if edited == original {
        return Ok(());
    }
    ed.transaction("Edit multiline text", move |tx| {
        tx.modify(id, |_| Entity::MText(Box::new(edited.clone())));
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

    fn start(state: &mut AppState, command: fn(&mut Editor) -> Result<(), Cancelled>) {
        state.start_command("Test", command);
        state.settle_command();
    }

    fn submit(state: &mut AppState, inputs: &[&str]) {
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
    }

    /// The text the editor was opened on, which proves the command is
    /// waiting on it.
    fn opened_with(state: &mut AppState) -> MText {
        let ask = state.waiting().expect("the command is waiting");
        assert_eq!(ask.wants, Wants::MTextEditor);
        let text = *ask.mtext.clone().expect("with a text to edit");
        state.sync_mtext_editor();
        assert!(state.mtext_editing(), "the editor opens");
        text
    }

    /// Type into the open editor and close it with `outcome`.
    fn type_and_close(state: &mut AppState, typed: &str, outcome: Outcome) {
        let active = state.active;
        if let Some(editor) = state.docs[active].mtext_editor.as_mut() {
            if !typed.is_empty() {
                editor.insert(typed, 0.0);
            }
        }
        state.finish_mtext_editor(outcome);
        state.settle_command();
    }

    fn texts(state: &AppState) -> Vec<MText> {
        let objects = state.session().document().objects();
        objects
            .iter()
            .filter_map(|o| match &o.entity {
                Entity::MText(m) => Some((**m).clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn two_corners_then_the_editor_make_one_wrapping_text_in_one_undo_step() {
        let _turn = take_turn_for_tests();
        let mut state = AppState::new();
        start(&mut state, draw_mtext);
        submit(&mut state, &["10,50", "50,20"]);
        let opened = opened_with(&mut state);
        assert_eq!(opened.contents, "", "a new text starts empty");
        assert_eq!((opened.insertion.x, opened.insertion.y), (10.0, 50.0));
        assert_eq!(opened.reference_width, 40.0);
        assert_eq!(opened.reference_height, 30.0, "the dragged box is kept");
        type_and_close(&mut state, "Hello\nworld", Outcome::Accept);
        assert!(!state.command_running() && !state.mtext_editing());
        let found = texts(&state);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].contents, "Hello\\Pworld");
        assert_eq!(found[0].nominal_height, 2.5);
        assert_eq!(found[0].reference_height, 30.0);
        state.undo();
        assert!(texts(&state).is_empty(), "one undo step");
    }

    #[test]
    fn the_opposite_corner_is_a_corner_prompt_with_no_rubber_band() {
        let _turn = take_turn_for_tests();
        let mut state = AppState::new();
        start(&mut state, draw_mtext);
        submit(&mut state, &["10,50"]);
        let ask = state.waiting().expect("asks for the opposite corner");
        assert_eq!(ask.base, Some(Point::new(10.0, 50.0)));
        assert!(!ask.rubber_band, "no line to the pointer");
        assert_eq!(
            ask.dynamic_dimensions,
            od_commands::DynamicDimensions::Cartesian
        );
        assert!(ask.ghost.is_some(), "the box outline follows the pointer");
    }

    #[test]
    fn the_options_set_height_justification_rotation_and_width() {
        let _turn = take_turn_for_tests();
        let mut state = AppState::new();
        start(&mut state, draw_mtext);
        submit(
            &mut state,
            &["0,0", "h", "5", "j", "bl", "r", "90", "w", "30", "10,10"],
        );
        let opened = opened_with(&mut state);
        assert_eq!(opened.nominal_height, 5.0);
        assert_eq!(opened.attachment, MTextAttachment::BottomLeft);
        assert_eq!(
            opened.reference_width, 30.0,
            "the typed width wins over the drag"
        );
        assert!(opened.direction.x.abs() < 1e-9 && (opened.direction.y - 1.0).abs() < 1e-9);
        type_and_close(&mut state, "Tilted", Outcome::Accept);
        assert_eq!(texts(&state).len(), 1);
        // The settings are remembered for the next text.
        assert_eq!(memory().height, 5.0);
        assert_eq!(memory().attachment, MTextAttachment::BottomLeft);
    }

    #[test]
    fn an_empty_editor_adds_nothing_and_escape_cancels() {
        let _turn = take_turn_for_tests();
        let mut state = AppState::new();
        start(&mut state, draw_mtext);
        submit(&mut state, &["0,0", "10,10"]);
        opened_with(&mut state);
        type_and_close(&mut state, "", Outcome::Accept);
        assert!(texts(&state).is_empty(), "nothing typed, nothing added");
        assert!(!state.command_running() && !state.mtext_editing());

        start(&mut state, draw_mtext);
        submit(&mut state, &["0,0", "10,10"]);
        opened_with(&mut state);
        type_and_close(&mut state, "typed but cancelled", Outcome::Cancel);
        assert!(texts(&state).is_empty());
        assert!(!state.command_running() && !state.mtext_editing());
    }

    #[test]
    fn a_dragged_box_sets_the_attachment_corner() {
        let _turn = take_turn_for_tests();
        let mut state = AppState::new();
        start(&mut state, draw_mtext);
        submit(&mut state, &["0,0", "j", "mc", "20,10"]);
        let opened = opened_with(&mut state);
        assert_eq!((opened.insertion.x, opened.insertion.y), (10.0, 5.0));
        assert_eq!(opened.reference_width, 20.0);
        type_and_close(&mut state, "Centred", Outcome::Accept);
        assert_eq!(texts(&state).len(), 1);
    }

    #[test]
    fn cancelling_before_the_editor_leaves_nothing() {
        let _turn = take_turn_for_tests();
        let mut state = AppState::new();
        start(&mut state, draw_mtext);
        submit(&mut state, &["0,0", "10,10"]);
        state.cancel();
        state.settle_command();
        assert!(texts(&state).is_empty());
        assert!(!state.mtext_editing(), "the editor closes with the command");
    }

    fn state_with_text(contents: &str) -> AppState {
        let mut state = AppState::new();
        state.add_entity(Entity::MText(Box::new(MText::new(
            Point::new(5.0, 5.0),
            2.0,
            contents,
            od_core::TextStyleId::STANDARD,
        ))));
        state
    }

    #[test]
    fn mtedit_opens_the_selected_text_and_a_change_is_one_undo_step() {
        let mut state = state_with_text("Hello");
        state.select_all();
        start(&mut state, edit_mtext_object);
        let opened = opened_with(&mut state);
        assert_eq!(opened.contents, "Hello");
        type_and_close(&mut state, " there", Outcome::Accept);
        assert_eq!(texts(&state)[0].contents, "Hello there");
        state.undo();
        assert_eq!(texts(&state)[0].contents, "Hello");
    }

    #[test]
    fn mtedit_names_the_text_it_replaces_and_a_new_text_names_none() {
        let mut state = state_with_text("Hello");
        let id = state.session().document().objects()[0].id;
        state.select_all();
        start(&mut state, edit_mtext_object);
        opened_with(&mut state);
        let active = state.active;
        let editor = state.docs[active].mtext_editor.as_ref().expect("open");
        assert_eq!(editor.replaces, Some(id), "the drawing leaves it out");
        type_and_close(&mut state, "", Outcome::Cancel);

        start(&mut state, draw_mtext);
        submit(&mut state, &["0,0", "10,10"]);
        opened_with(&mut state);
        let editor = state.docs[active].mtext_editor.as_ref().expect("open");
        assert_eq!(editor.replaces, None, "a new text replaces nothing");
    }

    #[test]
    fn mtedit_unchanged_or_cancelled_changes_nothing() {
        let mut state = state_with_text("Hello");
        let saved = state.session().document().clone();
        state.select_all();
        start(&mut state, edit_mtext_object);
        opened_with(&mut state);
        type_and_close(&mut state, "", Outcome::Accept);
        assert_eq!(*state.session().document(), saved, "no change, no step");

        start(&mut state, edit_mtext_object);
        opened_with(&mut state);
        type_and_close(&mut state, "!!", Outcome::Cancel);
        assert_eq!(*state.session().document(), saved);
    }

    #[test]
    fn mtedit_refuses_other_objects_and_waits_for_text() {
        let mut state = AppState::new();
        state.add_entity(Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(5.0, 0.0),
        });
        state.select_all();
        start(&mut state, edit_mtext_object);
        let ask = state.waiting().expect("asks which object");
        assert_eq!(ask.wants, Wants::Object, "a selected line is not text");
        assert!(!state.mtext_editing());
    }
}
