//! ATTDEF: define an attribute, worded as AutoCAD's -ATTDEF. It asks the
//! modes, the tag, the prompt, the default value and where and how big the
//! text is, then adds the definition to the drawing. Put into a block (BLOCK)
//! it becomes the template each insert of that block asks a value for.

use od_commands::{Answer, Cancelled, Editor, NumberOptions, Options, PointOptions};
use od_core::block::attribute::{normalized_tag, tag_problem};
use od_core::block::{
    AttributeDefinition, AttributeDefinitionId, AttributeFlags, AttributeText, FieldText,
};
use od_core::{Entity, Point};

use crate::attribute_memory::Memory;
use crate::defaults::EditorDefaults;

/// Mode names in the order the prompt lists them, with their keys.
const MODES: [(&str, &str, u16); 4] = [
    ("Invisible", "I", AttributeFlags::INVISIBLE),
    ("Constant", "C", AttributeFlags::CONSTANT),
    ("Verify", "V", AttributeFlags::VERIFY),
    ("Preset", "P", AttributeFlags::PRESET),
];

fn yes(on: bool) -> &'static str {
    if on {
        "Y"
    } else {
        "N"
    }
}

/// "Invisible=N Constant=N ..." as AutoCAD reports the modes.
fn describe(m: &Memory) -> String {
    let mut parts: Vec<String> = MODES
        .iter()
        .map(|(name, _, bit)| format!("{name}={}", yes(m.flags & bit != 0)))
        .collect();
    parts.push(format!("Lock position={}", yes(m.lock_position)));
    parts.join(" ")
}

/// The modes to start with, changed by the user until they press Enter. What
/// they choose is remembered (`AFLAGS`).
fn ask_modes(ed: &Editor) -> Result<Memory, Cancelled> {
    let defaults = ed.defaults();
    let mut memory = defaults.attribute.get();
    loop {
        ed.say(format!("Current attribute modes: {}", describe(&memory)));
        let mut options = Options::<String>::new(
            "Enter an option to change [Invisible/Constant/Verify/Preset/Lock position] <done>",
        );
        for (name, key, _) in MODES {
            options = options.keyword(name, key);
        }
        options = options.keyword("Lock position", "L");
        let Some(chosen) = ed.ask_keyword(options)? else {
            break;
        };
        match MODES.iter().find(|(name, _, _)| *name == chosen) {
            Some((_, _, bit)) => memory.flags ^= bit,
            None => memory.lock_position = !memory.lock_position,
        }
        // A constant value is never asked for, so it cannot be verified or
        // preset; AutoCAD leaves those bits alone, and so do we.
    }
    defaults.attribute.change(|m| {
        m.flags = memory.flags;
        m.lock_position = memory.lock_position;
    });
    Ok(memory)
}

fn ask_tag(ed: &Editor) -> Result<String, Cancelled> {
    loop {
        let tag = ed.get_string(Options::<String>::new("Enter attribute tag"))?;
        match tag_problem(tag.trim()) {
            None => return Ok(normalized_tag(tag.trim())),
            Some(why) => ed.say(why),
        }
    }
}

/// A line of text where Enter is `default`.
fn ask_line(ed: &Editor, message: &str, default: &str) -> Result<String, Cancelled> {
    let shown = if default.is_empty() {
        format!("{message} <none>")
    } else {
        format!("{message} <{default}>")
    };
    match ed.ask_string(Options::<String>::new(shown))? {
        Answer::Value(text) => Ok(text),
        Answer::Enter | Answer::Keyword(_) => Ok(default.to_owned()),
    }
}

/// The definition `draft` describes at `at`, drawn as its tag.
fn draft(tag: &str, flags: AttributeFlags, text: &AttributeText, at: Point) -> Vec<Entity> {
    let d = AttributeDefinition {
        id: AttributeDefinitionId(1),
        tag: tag.to_owned(),
        prompt: String::new(),
        default_value: FieldText::default(),
        flags,
        lock_position: false,
        field_length: 0,
        text: AttributeText {
            position: at,
            ..text.clone()
        },
    };
    vec![Entity::AttributeDefinition(Box::new(d))]
}

pub fn define_attribute(ed: &mut Editor) -> Result<(), Cancelled> {
    let modes = ask_modes(ed)?;
    let flags = AttributeFlags(modes.flags);
    let tag = ask_tag(ed)?;
    let prompt = if flags.constant() {
        String::new()
    } else {
        ask_line(ed, "Enter attribute prompt", &tag)?
    };
    let value = if flags.constant() {
        ask_line(ed, "Enter attribute value", "")?
    } else {
        ask_line(ed, "Enter default attribute value", "")?
    };
    let style = ed.read(|doc| doc.current_text_style())?;
    let height = ed.defaults().text.get().height;
    let sketch = AttributeText::new(Point::default(), height, style);
    let (shown, tag_shown) = (sketch.clone(), tag.clone());
    let at = ed.get_point(
        PointOptions::new("Specify start point of text")
            .ghost(move |at| draft(&tag_shown, flags, &shown, at)),
    )?;
    let height = ed.get_distance(
        NumberOptions::new("Specify height")
            .default(height)
            .base(at),
    )?;
    if !(height.is_finite() && height > 0.0) {
        ed.say("The text height must be above zero.");
        return Ok(());
    }
    ed.defaults().text.change(|m| m.height = height);
    let rotation = ed.get_angle(
        NumberOptions::new("Specify rotation angle of text")
            .default(0.0)
            .base(at),
    )?;
    let lock_position = modes.lock_position;
    ed.transaction("Define attribute", move |tx| {
        let id = AttributeDefinitionId(tx.document().next_attribute_definition_id());
        let mut text = AttributeText::new(at, height, style);
        text.rotation = rotation;
        tx.add(Entity::AttributeDefinition(Box::new(AttributeDefinition {
            id,
            tag,
            prompt,
            default_value: FieldText::Literal(value),
            flags,
            lock_position,
            field_length: 0,
            text,
        })));
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    fn run(answers: &[&str]) -> AppState {
        let mut state = AppState::new();
        state.start_command("ATTDEF", define_attribute);
        state.settle_command();
        for answer in answers {
            state.cmd_submit(answer);
            state.settle_command();
        }
        state
    }

    fn definition(state: &AppState) -> AttributeDefinition {
        match state
            .session()
            .document()
            .objects()
            .last()
            .map(|o| &o.entity)
        {
            Some(Entity::AttributeDefinition(d)) => (**d).clone(),
            other => panic!("no definition was added: {other:?}"),
        }
    }

    #[test]
    fn a_definition_takes_its_tag_upper_cased_with_the_prompt_defaulting_to_it() {
        let state = run(&["", "part_no", "", "none", "10,20", "4", "30"]);
        assert!(!state.command_running());
        let d = definition(&state);
        assert_eq!((d.tag.as_str(), d.prompt.as_str()), ("PART_NO", "PART_NO"));
        assert_eq!(d.default_value, FieldText::from("none"));
        assert_eq!(d.text.position, Point::new(10.0, 20.0));
        assert_eq!(d.text.height, 4.0);
        assert!((d.text.rotation - 30f64.to_radians()).abs() < 1e-9);
        assert_eq!(state.session().document().validate(), Ok(()));
        assert_eq!(state.session().document().next_attribute_definition_id(), 2);
    }

    #[test]
    fn a_bad_tag_is_asked_again_and_modes_toggle_until_enter() {
        let state = run(&[
            "C",
            "I",
            "",
            "two words",
            "a!",
            "price",
            "12.5",
            "0,0",
            "",
            "",
        ]);
        assert!(!state.command_running());
        let log = state.cmdline.log.join("\n");
        assert!(log.contains("cannot contain spaces"), "{log}");
        assert!(log.contains("cannot contain \"!\""), "{log}");
        let d = definition(&state);
        assert!(d.flags.constant() && d.flags.invisible());
        assert_eq!(d.prompt, "", "a constant has no prompt");
        assert_eq!(d.default_value, FieldText::from("12.5"));
        assert_eq!(
            state.defaults().attribute.get().flags,
            3,
            "AFLAGS is remembered"
        );
    }

    #[test]
    fn escape_adds_nothing_and_undo_removes_the_definition() {
        let mut state = run(&["", "tag", "", ""]);
        state.cancel_command();
        assert_eq!(state.session().document().len(), 0);
        let mut state = run(&["", "tag", "", "", "0,0", "", ""]);
        assert_eq!(state.session().document().len(), 1);
        state.undo();
        assert_eq!(state.session().document().len(), 0);
    }
}
