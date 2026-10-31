//! -OVERKILL: the command-line form of OVERKILL. After the objects it loops
//! on `Enter an option [Ignore/Tolerance/...] <accept>`, shares its settings
//! with the dialog, and tidies with the same `run`.

use od_commands::{Cancelled, Editor, Options};
use od_core::overkill::{Ignore, Settings};

use crate::defaults::EditorDefaults;
use crate::overkill_cmd::run;

fn yes_no(on: bool) -> &'static str {
    if on {
        "Yes"
    } else {
        "No"
    }
}

/// The settings in one line, for the command line.
fn describe(ed: &Editor, s: &Settings) -> Result<String, Cancelled> {
    let units = ed.read(|doc| *doc.units())?;
    let names = [
        (s.ignore.color, "Color"),
        (s.ignore.layer, "Layer"),
        (s.ignore.linetype, "Ltype"),
        (s.ignore.linetype_scale, "Ltscale"),
        (s.ignore.lineweight, "LWeight"),
    ];
    let ignored: Vec<&str> = names.iter().filter(|n| n.0).map(|n| n.1).collect();
    Ok(format!(
        "Tolerance = {}, Ignore = {}, Optimize polylines = {}, Combine partial overlap = {}, Combine end to end = {}, Associativity = {}",
        od_core::units::format_length(&units, s.tolerance),
        if ignored.is_empty() {
            "None".to_owned()
        } else {
            ignored.join(", ")
        },
        yes_no(s.optimize_polylines),
        yes_no(s.overlapping),
        yes_no(s.end_to_end),
        yes_no(s.maintain_associative),
    ))
}

/// Yes or No, with `current` as the default.
fn ask_yes_no(ed: &Editor, message: &str, current: bool) -> Result<bool, Cancelled> {
    let options = Options::<String>::new(format!("{message} [Yes/No]"))
        .keyword("Yes", "Y")
        .keyword("No", "N")
        .default(yes_no(current).to_owned());
    Ok(match ed.ask_keyword(options)?.as_deref() {
        Some("Yes") => true,
        Some("No") => false,
        _ => current,
    })
}

/// Choose properties to ignore, until Enter. `None` clears them all and
/// `All` ignores every one.
fn ask_ignore(ed: &Editor, ignore: &mut Ignore) -> Result<(), Cancelled> {
    loop {
        let options = Options::<String>::new(
            "Enter property to ignore, Enter when done [None/All/Color/LAyer/Ltype/Ltscale/LWeight]",
        )
        .keyword("None", "N")
        .keyword("All", "A")
        .keyword("Color", "C")
        .keyword("LAyer", "LA")
        .keyword("Ltype", "LT")
        .keyword("Ltscale", "LTS")
        .keyword("LWeight", "LW");
        match ed.ask_keyword(options)?.as_deref() {
            Some("None") => *ignore = Ignore::default(),
            Some("All") => {
                *ignore = Ignore {
                    layer: true,
                    color: true,
                    linetype: true,
                    linetype_scale: true,
                    lineweight: true,
                }
            }
            Some("Color") => ignore.color = !ignore.color,
            Some("LAyer") => ignore.layer = !ignore.layer,
            Some("Ltype") => ignore.linetype = !ignore.linetype,
            Some("Ltscale") => ignore.linetype_scale = !ignore.linetype_scale,
            Some("LWeight") => ignore.lineweight = !ignore.lineweight,
            _ => return Ok(()),
        }
    }
}

/// -OVERKILL.
pub fn overkill_prompt(ed: &mut Editor) -> Result<(), Cancelled> {
    let chosen = ed.pickset_or_select("Select objects")?;
    let mut settings = ed.defaults().overkill.get();
    loop {
        let line = describe(ed, &settings)?;
        ed.say(line);
        let options = Options::<String>::new(
            "Enter an option [Ignore/Tolerance/Optimize polylines/Combine partial overlap/combine End to end/Associativity] <accept>",
        )
        .keyword("Ignore", "I")
        .keyword("Tolerance", "T")
        .keyword("Optimize polylines", "O")
        .keyword("Combine partial overlap", "C")
        .keyword("combine End to end", "E")
        .keyword("Associativity", "A");
        match ed.ask_keyword(options)?.as_deref() {
            Some("Ignore") => ask_ignore(ed, &mut settings.ignore)?,
            Some("Tolerance") => {
                let ask = Options::<f64>::new("Specify tolerance").default(settings.tolerance);
                let value = ed.get_distance(ask)?;
                if value.is_finite() && value >= 0.0 {
                    settings.tolerance = value;
                } else {
                    ed.say("The tolerance must be zero or more.");
                }
            }
            Some("Optimize polylines") => {
                settings.optimize_polylines =
                    ask_yes_no(ed, "Optimize polylines", settings.optimize_polylines)?;
            }
            Some("Combine partial overlap") => {
                settings.overlapping =
                    ask_yes_no(ed, "Combine partial overlap", settings.overlapping)?;
            }
            Some("combine End to end") => {
                settings.end_to_end = ask_yes_no(ed, "Combine end to end", settings.end_to_end)?;
            }
            Some("Associativity") => {
                settings.maintain_associative = ask_yes_no(
                    ed,
                    "Maintain associative objects",
                    settings.maintain_associative,
                )?;
            }
            _ => break,
        }
    }
    ed.defaults().overkill.change(|m| *m = settings);
    run(ed, chosen, settings)
}
