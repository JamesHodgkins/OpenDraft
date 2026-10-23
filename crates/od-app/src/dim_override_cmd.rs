//! DIMOVERRIDE and the Apply option of `-DIMSTYLE`: change the style
//! variables of dimensions that exist without touching the style they were
//! made in, or give them the current style.

use od_commands::{Answer, Cancelled, Editor, Options};
use od_core::{AngleFormat, Arrow, DimStyle, Entity, EntityId, LengthFormat};

use crate::text::plural;

/// Set the variable `name` (as `DIMTXT`, ignoring case) of `style` to the
/// typed `value`.
pub fn set_variable(style: &mut DimStyle, name: &str, value: &str) -> Result<(), String> {
    let name = name.trim().to_ascii_uppercase();
    let value = value.trim();
    let number = || {
        value
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .ok_or_else(|| "Requires a number.".to_owned())
    };
    let size = || {
        number().and_then(|n| {
            if n >= 0.0 {
                Ok(n)
            } else {
                Err("Requires a value of zero or more.".to_owned())
            }
        })
    };
    let choice = |max: u8| {
        number().and_then(|n| {
            if n.fract() == 0.0 && (0.0..=f64::from(max)).contains(&n) {
                Ok(n as u8)
            } else {
                Err(format!("Requires a whole number from 0 to {max}."))
            }
        })
    };
    let flag = || choice(1).map(|n| n == 1);
    match name.as_str() {
        "DIMSCALE" => {
            style.scale = number().and_then(|n| {
                if n > 0.0 {
                    Ok(n)
                } else {
                    Err("Requires a value above zero.".to_owned())
                }
            })?
        }
        "DIMASZ" => style.arrow_size = size()?,
        "DIMEXO" => style.ext_offset = size()?,
        "DIMEXE" => style.ext_extend = size()?,
        "DIMTXT" => {
            style.text_height = size().and_then(|n| {
                if n > 0.0 {
                    Ok(n)
                } else {
                    Err("Requires a value above zero.".to_owned())
                }
            })?
        }
        "DIMGAP" => style.text_gap = size()?,
        "DIMCEN" => style.centre_mark = size()?,
        "DIMRND" => style.round_to = size()?,
        "DIMLFAC" => {
            style.linear_factor = number().and_then(|n| {
                if n != 0.0 {
                    Ok(n)
                } else {
                    Err("Requires a value other than zero.".to_owned())
                }
            })?
        }
        "DIMTAD" => style.text_above = choice(4)?,
        "DIMJUST" => style.text_just = choice(4)?,
        "DIMTIH" => style.inside_horizontal = flag()?,
        "DIMTOH" => style.outside_horizontal = flag()?,
        "DIMTIX" => style.text_inside = flag()?,
        "DIMTOFL" => style.force_line = flag()?,
        "DIMATFIT" => style.fit = choice(3)?,
        "DIMTMOVE" => style.text_move = choice(2)?,
        "DIMSD1" => style.suppress_line[0] = flag()?,
        "DIMSD2" => style.suppress_line[1] = flag()?,
        "DIMSE1" => style.suppress_ext[0] = flag()?,
        "DIMSE2" => style.suppress_ext[1] = flag()?,
        "DIMDEC" => style.precision = choice(8)?,
        "DIMARCSYM" => style.arc_symbol = choice(2)?,
        "DIMADEC" => style.angle_precision = choice(8)?,
        "DIMAUNIT" => {
            style.angle_format = AngleFormat::from_code(i64::from(choice(4)?)).unwrap_or_default()
        }
        "DIMZIN" => style.trim_zeros = choice(8)? & 8 != 0,
        "DIMLUNIT" => {
            style.length_format = number()
                .ok()
                .and_then(|n| LengthFormat::from_code(n as i64))
                .ok_or_else(|| "Requires a whole number from 1 to 5.".to_owned())?
        }
        "DIMPOST" => {
            style.post = if value == "." {
                String::new()
            } else {
                value.to_owned()
            }
        }
        "DIMDSEP" => {
            let mut chars = value.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => style.decimal_separator = c,
                _ => return Err("Requires a single character.".to_owned()),
            }
        }
        "DIMBLK" => {
            style.arrow = match value.to_uppercase().as_str() {
                "." | "" | "_CLOSEDFILLED" => Arrow::Closed,
                "_OPEN" => Arrow::Open,
                "_ARCHTICK" => Arrow::Tick,
                "_NONE" => Arrow::None,
                _ => return Err("Use . _OPEN _ARCHTICK or _NONE.".to_owned()),
            }
        }
        _ => {
            return Err(format!(
                "{name} is not a dimension variable that can be overridden."
            ))
        }
    }
    Ok(())
}

fn dimensions_in(ed: &Editor, ids: Vec<EntityId>) -> Result<Vec<EntityId>, Cancelled> {
    let found = ed.read(move |doc| {
        ids.into_iter()
            .filter(|id| matches!(doc.get(*id), Some(Entity::Dimension(_))))
            .collect::<Vec<_>>()
    })?;
    if found.is_empty() {
        ed.say("No dimensions selected.");
        return Err(Cancelled);
    }
    Ok(found)
}

/// Replace the style of each dimension with `next(its style, the drawing's
/// style of the same name)`, as one undo step.
fn restyle(
    ed: &Editor,
    ids: Vec<EntityId>,
    next: impl Fn(&DimStyle, Option<&DimStyle>) -> DimStyle + Send + 'static,
) -> Result<(), Cancelled> {
    let changed = ed.transaction("Dimension override", move |tx| {
        let mut changed = 0usize;
        for id in ids {
            let replacement = match tx.document().get(id) {
                Some(Entity::Dimension(d)) => {
                    let named = tx.document().dim_style_named(&d.style.name);
                    let style = next(&d.style, named).sanitized();
                    (style != d.style).then(|| {
                        let mut d = (**d).clone();
                        d.style = style;
                        d
                    })
                }
                _ => None,
            };
            if let Some(d) = replacement {
                tx.modify(id, |_| Entity::Dimension(Box::new(d.clone())));
                changed += 1;
            }
        }
        Ok(changed)
    })?;
    ed.say(format!("{} changed.", plural(changed, "dimension")));
    Ok(())
}

/// DIMOVERRIDE: variable names and values until Enter, then the dimensions
/// they apply to; or Clear, which puts dimensions back to their style's own
/// values.
pub fn dim_override(ed: &mut Editor) -> Result<(), Cancelled> {
    let mut changes: Vec<(String, String)> = Vec::new();
    let mut scratch = ed.read(|doc| doc.dim_style().clone())?;
    loop {
        let options = Options::<String>::new(
            "Enter dimension variable name to override or [Clear overrides]",
        )
        .keyword("Clear overrides", "C")
        .allow_empty();
        match ed.ask_string(options)? {
            Answer::Keyword(_) => {
                let ids = ed.pickset_or_select("Select objects")?;
                let ids = dimensions_in(ed, ids)?;
                return restyle(ed, ids, |own, named| {
                    named.cloned().unwrap_or_else(|| own.clone())
                });
            }
            Answer::Value(name) if name.trim().eq_ignore_ascii_case("C") => {
                let ids = ed.pickset_or_select("Select objects")?;
                let ids = dimensions_in(ed, ids)?;
                return restyle(ed, ids, |own, named| {
                    named.cloned().unwrap_or_else(|| own.clone())
                });
            }
            Answer::Value(name) => {
                let value = match ed.ask_string(Options::<String>::new(format!(
                    "Enter new value for dimension variable {}",
                    name.trim().to_ascii_uppercase()
                )))? {
                    Answer::Value(v) => v,
                    _ => continue,
                };
                match set_variable(&mut scratch, &name, &value) {
                    Ok(()) => changes.push((name, value)),
                    Err(why) => ed.say(why),
                }
            }
            Answer::Enter => break,
        }
    }
    if changes.is_empty() {
        return Ok(());
    }
    let ids = ed.pickset_or_select("Select objects")?;
    let ids = dimensions_in(ed, ids)?;
    restyle(ed, ids, move |own, _| {
        let mut style = own.clone();
        for (name, value) in &changes {
            let _ = set_variable(&mut style, name, value);
        }
        style
    })
}

/// `-DIMSTYLE` Apply: the selected dimensions take the current style.
pub fn dim_update(ed: &mut Editor) -> Result<(), Cancelled> {
    let current = ed.read(|doc| doc.dim_style().clone())?;
    let ids = ed.pickset_or_select("Select objects")?;
    let ids = dimensions_in(ed, ids)?;
    restyle(ed, ids, move |_, _| current.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use od_core::{Dimension, Point};

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn started(run: fn(&mut Editor) -> Result<(), Cancelled>) -> AppState {
        let mut state = AppState::new();
        let d = Dimension::aligned(p(0.0, 0.0), p(10.0, 0.0), p(0.0, 5.0), &DimStyle::default());
        state.add_entity(Entity::Dimension(Box::new(d)));
        state.select_all();
        state.start_command("Test", run);
        state.settle_command();
        state
    }

    fn submit(state: &mut AppState, text: &str) {
        state.cmd_submit(text);
        state.settle_command();
    }

    fn style_of(state: &AppState) -> DimStyle {
        match &state.session().document().objects()[0].entity {
            Entity::Dimension(d) => d.style.clone(),
            _ => unreachable!(),
        }
    }

    #[test]
    fn variables_are_checked() {
        let mut s = DimStyle::default();
        set_variable(&mut s, "dimtxt", "4").unwrap();
        set_variable(&mut s, "DIMPOST", "<> mm").unwrap();
        assert_eq!((s.text_height, s.post.as_str()), (4.0, "<> mm"));
        assert!(set_variable(&mut s, "DIMTXT", "0").is_err());
        assert!(set_variable(&mut s, "DIMTAD", "9").is_err());
        assert!(set_variable(&mut s, "DIMBLK", "x").is_err());
        assert!(set_variable(&mut s, "DIMCLRD", "1").is_err());
    }

    #[test]
    fn override_changes_the_dimension_and_clear_puts_it_back() {
        let mut state = started(dim_override);
        submit(&mut state, "dimtxt");
        submit(&mut state, "5");
        submit(&mut state, "");
        assert_eq!(style_of(&state).text_height, 5.0);
        assert_eq!(state.dim_style().text_height, 2.5, "the style is untouched");
        state.select_all();
        state.start_command("Test", dim_override);
        state.settle_command();
        submit(&mut state, "c");
        assert_eq!(style_of(&state).text_height, 2.5);
        state.undo();
        assert_eq!(style_of(&state).text_height, 5.0, "one undo step");
    }

    #[test]
    fn update_gives_the_current_style() {
        let mut state = started(dim_update);
        let mut current = state.dim_style();
        current.precision = 0;
        state.set_dim_style(current);
        state.select_all();
        state.start_command("Test", dim_update);
        state.settle_command();
        assert_eq!(style_of(&state).precision, 0);
    }
}
