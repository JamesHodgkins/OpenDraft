//! Editing commands written against the `Editor`: straight-line code that
//! asks and waits. See "Commands and the Editor" in ARCHITECTURE.md.

use od_commands::{Answer, Cancelled, Editor, Options, PointOptions};
use od_core::{Entity, EntityId, Object, Point};

use crate::text::plural;

/// MOVE: shift objects by a base point and a second point, or by a typed
/// displacement (the Displacement option, or Enter at the base point prompt).
pub fn move_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    let ids = ed.pickset_or_select("Select objects")?;
    let originals = entities_of(ed, &ids)?;
    let base_options = PointOptions::new("Specify base point").keyword("Displacement", "DI");
    let (dx, dy) = match ed.ask_point(base_options)? {
        Answer::Value(base) => {
            let ghost_of = originals;
            let options = PointOptions::new(
                "Specify second point of move or <use first point as displacement>",
            )
            .dimensions(od_commands::DynamicDimensions::Polar)
            .base(base)
            .ghost(move |at| {
                ghost_of
                    .iter()
                    .map(|e| e.translated(at.x - base.x, at.y - base.y))
                    .collect()
            });
            // Enter alone takes the base point as the displacement from the origin.
            match ed.ask_point(options)? {
                Answer::Value(to) => (to.x - base.x, to.y - base.y),
                _ => (base.x, base.y),
            }
        }
        Answer::Keyword(_) | Answer::Enter => {
            let ghost_of = originals;
            let options = PointOptions::new("Specify displacement")
                .default(Point::new(0.0, 0.0))
                .ghost(move |at| ghost_of.iter().map(|e| e.translated(at.x, at.y)).collect());
            let displacement = ed.get_point(options)?;
            (displacement.x, displacement.y)
        }
    };
    let count = ids.len();
    ed.transaction("Move", move |tx| {
        for id in ids {
            tx.modify(id, |e| e.translated(dx, dy));
        }
        Ok(())
    })?;
    ed.say(format!("Moved {}", plural(count, "object")));
    Ok(())
}

/// The objects as they are now.
pub(crate) fn objects_of(ed: &Editor, ids: &[EntityId]) -> Result<Vec<Object>, Cancelled> {
    let ids = ids.to_vec();
    ed.read(move |doc| {
        ids.iter()
            .filter_map(|&id| doc.object(id).cloned())
            .collect()
    })
}

pub(crate) fn entities_of(ed: &Editor, ids: &[EntityId]) -> Result<Vec<Entity>, Cancelled> {
    Ok(objects_of(ed, ids)?.into_iter().map(|o| o.entity).collect())
}

/// COPY: copy objects to as many places as the user picks, until Exit, Enter
/// or Esc. Each copy is its own undo step.
pub fn copy_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    let ids = ed.pickset_or_select("Select objects")?;
    let objects = objects_of(ed, &ids)?;
    let base = ed.get_point("Specify base point")?;
    loop {
        let ghosts: Vec<Entity> = objects.iter().map(|o| o.entity.clone()).collect();
        let options = PointOptions::new("Specify second point")
            .dimensions(od_commands::DynamicDimensions::Polar)
            .base(base)
            .keyword("Exit", "E")
            .ghost(move |at| {
                ghosts
                    .iter()
                    .map(|e| e.translated(at.x - base.x, at.y - base.y))
                    .collect()
            });
        let Answer::Value(to) = ed.ask_point(options)? else {
            return Ok(());
        };
        let (dx, dy) = (to.x - base.x, to.y - base.y);
        let copies: Vec<_> = objects
            .iter()
            .map(|o| (o.entity.translated(dx, dy), o.attrs.clone()))
            .collect();
        let count = copies.len();
        ed.transaction("Copy", move |tx| {
            tx.add_with(copies);
            Ok(())
        })?;
        ed.say(format!("Copied {}", plural(count, "object")));
    }
}

/// MIRROR: reflect objects in a line given by two points, keeping or erasing
/// the originals.
pub fn mirror_objects(ed: &mut Editor) -> Result<(), Cancelled> {
    let ids = ed.pickset_or_select("Select objects")?;
    let objects = objects_of(ed, &ids)?;
    if let Some(why) = objects.iter().find_map(|o| o.entity.mirror_refusal()) {
        ed.say(why);
        return Ok(());
    }
    let first = ed.get_point("Specify first point of mirror line")?;
    let ghosts: Vec<Entity> = objects.iter().map(|o| o.entity.clone()).collect();
    let second: Point = loop {
        let ghosts = ghosts.clone();
        let options = PointOptions::new("Specify second point of mirror line")
            .dimensions(od_commands::DynamicDimensions::Polar)
            .base(first)
            .ghost(move |at| {
                if at == first {
                    return Vec::new();
                }
                ghosts.iter().map(|e| e.mirrored(first, at)).collect()
            });
        let point = ed.get_point(options)?;
        // A point on the first one gives no line; ask again.
        if point != first {
            break point;
        }
    };
    let erase = ed.get_keyword(
        Options::new("Erase source objects?")
            .keyword("Yes", "Y")
            .keyword("No", "N")
            .default("No".to_owned()),
    )? == "Yes";
    let count = plural(objects.len(), "object");
    ed.transaction("Mirror", move |tx| {
        if erase {
            for o in &objects {
                tx.modify(o.id, |e| e.mirrored(first, second));
            }
        } else {
            let images = objects
                .iter()
                .map(|o| (o.entity.mirrored(first, second), o.attrs.clone()))
                .collect();
            tx.add_with(images);
        }
        Ok(())
    })?;
    ed.say(if erase {
        format!("Mirrored {count}")
    } else {
        format!("Mirrored {count}, originals kept")
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use od_core::Entity;

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

    /// Start `command`, then answer its prompts with typed `inputs`.
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

    #[test]
    fn move_takes_a_base_point_then_a_relative_point() {
        let mut state = state_with_selected_line();
        run(&mut state, move_objects, &["2,2", "@5,3"]);
        assert_eq!(entities(&state), [line((5.0, 3.0), (15.0, 3.0))]);
        assert!(!state.command_running());
        assert_eq!(state.selection().len(), 1, "the selection survives");
    }

    #[test]
    fn mirror_leaves_everything_alone_when_an_object_refuses() {
        let mut state = state_with_selected_line();
        let text = od_core::MText::new(p(1.0, 1.0), 2.0, "Hi", od_core::TextStyleId::STANDARD);
        state.add_entity(Entity::MText(Box::new(text)));
        state.select_all();
        let before = entities(&state);
        run(&mut state, mirror_objects, &[]);
        assert!(
            !state.command_running(),
            "it does not even ask for the line"
        );
        assert_eq!(entities(&state), before);
    }

    #[test]
    fn move_displacement_is_a_vector_from_the_origin() {
        let mut state = state_with_selected_line();
        run(&mut state, move_objects, &["di", "4,6"]);
        assert_eq!(entities(&state), [line((4.0, 6.0), (14.0, 6.0))]);
        assert!(!state.command_running());
    }

    #[test]
    fn move_enter_at_the_base_point_prompt_asks_for_a_displacement() {
        let mut state = state_with_selected_line();
        run(&mut state, move_objects, &[""]);
        assert!(state.cmd_prompt().text.starts_with("Specify displacement"));
        state.cmd_submit("-1,2");
        state.settle_command();
        assert_eq!(entities(&state), [line((-1.0, 2.0), (9.0, 2.0))]);
    }

    #[test]
    fn move_enter_at_the_second_point_uses_the_base_point_as_displacement() {
        let mut state = state_with_selected_line();
        run(&mut state, move_objects, &["3,1", ""]);
        assert_eq!(entities(&state), [line((3.0, 1.0), (13.0, 1.0))]);
    }

    #[test]
    fn copy_keeps_asking_until_exit_and_each_copy_is_one_undo_step() {
        let mut state = AppState::new();
        let walls = state.new_layer();
        state.set_current_layer(walls);
        state.add_entity(line((0.0, 0.0), (10.0, 0.0)));
        state.select_all();

        run(&mut state, copy_objects, &["0,0", "0,5", "0,10"]);
        assert_eq!(entities(&state).len(), 3);
        assert!(state.command_running(), "Copy stays armed");
        assert_eq!(state.cmd_prompt().text, "Specify second point or");
        let document = &state.session().document();
        assert!(document.objects().iter().all(|o| o.attrs.layer == walls));
        assert_eq!(state.selection().len(), 1, "the originals stay selected");

        state.cmd_submit("e");
        state.settle_command();
        assert!(!state.command_running());
        assert_eq!(entities(&state).len(), 3);
        state.undo();
        assert_eq!(entities(&state).len(), 2, "one copy per undo step");
    }

    #[test]
    fn mirror_asks_whether_to_erase_and_keeps_the_originals_by_default() {
        let mut state = state_with_selected_line();
        run(&mut state, mirror_objects, &["0,0", "1,0"]);
        assert!(state.cmd_prompt().text.starts_with("Erase source objects?"));
        assert!(state.cmd_prompt().text.contains("<No>"));
        state.cmd_submit("");
        state.settle_command();
        assert_eq!(entities(&state).len(), 2, "original and mirror image");
        assert_eq!(entities(&state)[0], line((0.0, 0.0), (10.0, 0.0)));
        assert!(!state.command_running());
    }

    #[test]
    fn mirror_with_erase_replaces_the_originals_and_undoes() {
        let mut state = AppState::new();
        state.add_entity(line((1.0, 1.0), (5.0, 3.0)));
        state.select_all();
        run(&mut state, mirror_objects, &["0,0", "0,1", "y"]);
        assert_eq!(entities(&state), [line((-1.0, 1.0), (-5.0, 3.0))]);
        state.undo();
        assert_eq!(entities(&state), [line((1.0, 1.0), (5.0, 3.0))]);
    }

    #[test]
    fn mirror_ignores_a_second_point_on_the_first() {
        let mut state = state_with_selected_line();
        run(&mut state, mirror_objects, &["0,0", "0,0"]);
        assert_eq!(
            state.cmd_prompt().text,
            "Specify second point of mirror line"
        );
        assert_eq!(entities(&state).len(), 1);
    }

    #[test]
    fn modify_commands_are_registered_and_light_up_while_running() {
        let registry = crate::commands::build_registry();
        let mut state = state_with_selected_line();
        for (id, name) in [
            ("modify.move", "Move"),
            ("modify.copy", "Copy"),
            ("modify.rotate", "Rotate"),
            ("modify.mirror", "Mirror"),
            ("modify.scale", "Scale"),
        ] {
            assert!(!registry.is_checked(id, &state));
            registry.execute(id, &mut state).unwrap();
            state.settle_command();
            assert!(registry.is_checked(id, &state), "{id}");
            assert!(state.is_running(name));
            state.cancel();
        }
    }
}
