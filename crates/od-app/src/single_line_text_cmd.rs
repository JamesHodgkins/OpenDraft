//! TEXT/DTEXT: one line of text placed at a point, with a height, rotation
//! and justification. The object remains a real single-line `TEXT` entity;
//! the shared text engine draws its projection.

use od_commands::{Answer, Cancelled, Editor, NumberOptions, Options, PointOptions};
use od_core::block::{TextHorizontal, TextVertical};
use od_core::{Entity, SingleLineText, TextAlignmentMode, TextStyleId};

use crate::defaults::EditorDefaults;
use crate::draw::{add, MIN_SIZE};

const JUSTIFY: [(&str, &str, TextHorizontal, TextVertical); 13] = [
    ("Left", "L", TextHorizontal::Left, TextVertical::Baseline),
    (
        "Center",
        "C",
        TextHorizontal::Center,
        TextVertical::Baseline,
    ),
    ("Middle", "M", TextHorizontal::Center, TextVertical::Middle),
    ("Right", "R", TextHorizontal::Right, TextVertical::Baseline),
    ("TL", "TL", TextHorizontal::Left, TextVertical::Top),
    ("TC", "TC", TextHorizontal::Center, TextVertical::Top),
    ("TR", "TR", TextHorizontal::Right, TextVertical::Top),
    ("ML", "ML", TextHorizontal::Left, TextVertical::Middle),
    ("MC", "MC", TextHorizontal::Center, TextVertical::Middle),
    ("MR", "MR", TextHorizontal::Right, TextVertical::Middle),
    ("BL", "BL", TextHorizontal::Left, TextVertical::Bottom),
    ("BC", "BC", TextHorizontal::Center, TextVertical::Bottom),
    ("BR", "BR", TextHorizontal::Right, TextVertical::Bottom),
];

#[derive(Clone, Copy)]
enum PlacementMode {
    Justified(TextHorizontal, TextVertical),
    Aligned(TextAlignmentMode),
}

fn choose_justification(ed: &Editor) -> Result<PlacementMode, Cancelled> {
    let mut options = Options::<String>::new(
        "Enter an option [Align/Fit/Left/Center/Middle/Right/TL/TC/TR/ML/MC/MR/BL/BC/BR]",
    )
    .keyword("Align", "A")
    .keyword("Fit", "F");
    for (name, key, _, _) in JUSTIFY {
        options = options.keyword(name, key);
    }
    let chosen = ed.get_keyword(options)?;
    if chosen == "Align" {
        return Ok(PlacementMode::Aligned(TextAlignmentMode::Align));
    }
    if chosen == "Fit" {
        return Ok(PlacementMode::Aligned(TextAlignmentMode::Fit));
    }
    JUSTIFY
        .iter()
        .find(|(name, _, _, _)| *name == chosen)
        .map(|(_, _, horizontal, vertical)| PlacementMode::Justified(*horizontal, *vertical))
        .ok_or(Cancelled)
}

fn choose_style(ed: &Editor, current: TextStyleId) -> Result<TextStyleId, Cancelled> {
    let current_name = ed.read(move |doc| {
        doc.text_style(current)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| "Standard".to_owned())
    })?;
    loop {
        let answer = ed.ask_string(Options::<String>::new(format!(
            "Enter style name <{current_name}>"
        )))?;
        let name = match answer {
            Answer::Value(name) => name,
            Answer::Enter => return Ok(current),
            Answer::Keyword(_) => continue,
        };
        let key = name.trim().to_owned();
        if let Some(id) = ed.read(move |doc| doc.text_style_named(&key).map(|s| s.id))? {
            return Ok(id);
        }
        ed.say(format!("Text style \"{}\" does not exist.", name.trim()));
    }
}

/// TEXT: choose the point (or Justify/Style), height, rotation and contents.
/// Empty contents add nothing; accepting a line is one undo step.
pub fn draw_text(ed: &mut Editor) -> Result<(), Cancelled> {
    let mut style = ed.read(|doc| doc.current_text_style())?;
    let mut mode = PlacementMode::Justified(TextHorizontal::Left, TextVertical::Baseline);
    let at = loop {
        let options = PointOptions::new("Specify start point of text")
            .keyword("Justify", "J")
            .keyword("Style", "S");
        match ed.ask_point(options)? {
            Answer::Value(point) => break point,
            Answer::Keyword(word) if word == "Justify" => {
                mode = choose_justification(ed)?;
            }
            Answer::Keyword(_) => style = choose_style(ed, style)?,
            Answer::Enter => return Err(Cancelled),
        }
    };

    let second = if matches!(mode, PlacementMode::Aligned(_)) {
        Some(loop {
            let point =
                ed.get_point(PointOptions::new("Specify second endpoint of text").base(at))?;
            if (point.x - at.x).hypot(point.y - at.y) > MIN_SIZE {
                break point;
            }
            ed.say("Requires a point different from the first endpoint.");
        })
    } else {
        None
    };
    let fixed_height = ed.read(move |doc| doc.text_style(style).and_then(|s| s.fixed_height))?;
    let needs_height = !matches!(mode, PlacementMode::Aligned(TextAlignmentMode::Align));
    let height = match (needs_height, fixed_height) {
        (false, _) => 1.0,
        (true, Some(height)) => height,
        (true, None) => loop {
            let remembered = ed.defaults().text.get().height;
            let height = ed.get_distance(
                NumberOptions::new("Specify height")
                    .default(remembered)
                    .base(at),
            )?;
            if height > MIN_SIZE {
                ed.defaults().text.change(|m| m.height = height);
                break height;
            }
            ed.say("Requires a positive value.");
        },
    };
    let rotation = if second.is_some() {
        0.0
    } else {
        ed.get_angle(
            NumberOptions::new("Specify rotation angle of text")
                .default(0.0)
                .base(at),
        )?
    };
    let contents = match ed.ask_string(Options::<String>::new("Enter text"))? {
        Answer::Value(text) => text,
        Answer::Enter => return Ok(()),
        Answer::Keyword(_) => return Err(Cancelled),
    };
    if contents.is_empty() {
        return Ok(());
    }
    let mut text = SingleLineText::new(at, height, contents, style);
    match mode {
        PlacementMode::Justified(horizontal, vertical) => {
            text.placement.rotation = rotation;
            text.placement.horizontal = horizontal;
            text.placement.vertical = vertical;
        }
        PlacementMode::Aligned(alignment) => {
            let metrics = ed
                .read(move |doc| doc.text_style(style).cloned())?
                .ok_or(Cancelled)?;
            text.resolve_alignment(
                alignment,
                second.expect("aligned mode has endpoint"),
                &metrics,
            )
            .map_err(|why| {
                ed.say(why);
                Cancelled
            })?;
        }
    }
    add(ed, Entity::Text(Box::new(text)), "Single-line Text")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use od_core::Point;

    fn run(answers: &[&str]) -> AppState {
        let mut state = AppState::new();
        state.start_command("TEXT", draw_text);
        state.settle_command();
        for answer in answers {
            state.cmd_submit(answer);
            state.settle_command();
        }
        state
    }

    fn text(state: &AppState) -> &SingleLineText {
        match &state.session().document().objects()[0].entity {
            Entity::Text(text) => text,
            other => panic!("expected text, found {other:?}"),
        }
    }

    #[test]
    fn point_height_rotation_and_words_make_one_text_and_one_undo_step() {
        let mut state = run(&["10,20", "4", "30", "Hello world"]);
        let found = text(&state);
        assert_eq!(found.contents, "Hello world");
        assert_eq!(found.position(), Point::new(10.0, 20.0));
        assert_eq!(found.placement.height, 4.0);
        assert!((found.placement.rotation - 30f64.to_radians()).abs() < 1e-9);
        assert_eq!(state.session().document().validate(), Ok(()));
        state.undo();
        assert!(state.session().document().is_empty());
    }

    #[test]
    fn justification_is_kept_and_an_empty_line_adds_nothing() {
        let state = run(&["j", "tr", "1,2", "", "", "Top right"]);
        let found = text(&state);
        assert_eq!(found.placement.horizontal, TextHorizontal::Right);
        assert_eq!(found.placement.vertical, TextVertical::Top);

        let empty = run(&["0,0", "", "", ""]);
        assert!(empty.session().document().is_empty());
        assert!(!empty.command_running());
    }

    #[test]
    fn align_and_fit_land_exactly_between_their_endpoints() {
        for (word, mode) in [
            ("align", TextAlignmentMode::Align),
            ("fit", TextAlignmentMode::Fit),
        ] {
            let answers = if mode == TextAlignmentMode::Fit {
                vec!["j", word, "0,0", "12,0", "2", "Wide words"]
            } else {
                vec!["j", word, "0,0", "12,0", "Wide words"]
            };
            let state = run(&answers);
            let found = text(&state);
            assert_eq!(found.alignment.unwrap().mode, mode);
            assert_eq!(found.alignment.unwrap().second, Point::new(12.0, 0.0));
            let style = state
                .session()
                .document()
                .text_style(TextStyleId::STANDARD)
                .unwrap();
            let width = found.projection().layout(style).actual_width;
            assert!((width - 12.0).abs() < 1e-8, "{mode:?}: {width}");
        }
    }
}
