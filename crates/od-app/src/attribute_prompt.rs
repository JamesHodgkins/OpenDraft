//! Asking for the values of a block's attributes when it is inserted, in the
//! order the definitions are in. `ATTREQ` off, a constant or a preset
//! attribute uses the default without asking; a verify attribute is
//! confirmed.

use std::sync::Arc;

use od_commands::{Answer, Cancelled, Editor, Options};
use od_core::block::{AttributeReference, FieldText};
use od_core::BlockDef;

use crate::defaults::EditorDefaults;

/// The references for a new insert of `block`.
pub(crate) fn ask_values(
    ed: &Editor,
    block: &Arc<BlockDef>,
) -> Result<Vec<AttributeReference>, Cancelled> {
    let asks = ed.defaults().attribute.get().prompt_on_insert;
    let mut out = Vec::new();
    for (_, d) in block
        .attribute_definitions()
        .filter(|(_, d)| !d.flags.constant())
    {
        let mut value = d.default_value.clone();
        if asks && !d.flags.preset() {
            loop {
                value = ask_value(ed, d, &value)?;
                if !d.flags.verify() || confirmed(ed, &value)? {
                    break;
                }
            }
        }
        out.push(d.reference(value));
    }
    Ok(out)
}

fn ask_value(
    ed: &Editor,
    d: &od_core::block::AttributeDefinition,
    current: &FieldText,
) -> Result<FieldText, Cancelled> {
    let name = if d.prompt.is_empty() {
        &d.tag
    } else {
        &d.prompt
    };
    let shown = current.display();
    let message = if shown.is_empty() {
        format!("Enter attribute value {name}")
    } else {
        format!("Enter attribute value {name} <{shown}>")
    };
    Ok(match ed.ask_string(Options::<String>::new(message))? {
        Answer::Value(text) => FieldText::Literal(text),
        Answer::Enter | Answer::Keyword(_) => current.clone(),
    })
}

fn confirmed(ed: &Editor, value: &FieldText) -> Result<bool, Cancelled> {
    let question = Options::<String>::new(format!(
        "Verify attribute value \"{}\": is it correct?",
        value.display()
    ))
    .keyword("Yes", "Y")
    .keyword("No", "N")
    .default("Yes".to_owned());
    Ok(ed.get_keyword(question)? == "Yes")
}
