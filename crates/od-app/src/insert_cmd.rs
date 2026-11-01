//! INSERT: place a block reference, worded as AutoCAD's -INSERT. The block
//! is previewed at the cursor; the geometry is `od_core::Insert`.

use std::sync::Arc;

use od_commands::{Answer, Cancelled, Editor, NumberOptions, Options, PointOptions};
use od_core::block::AttributeReference;
use od_core::{BlockDef, Entity, Insert, InsertUnits, Point};

use crate::attribute_prompt::ask_values;
use crate::block_cmd::{ask_block_name, list_blocks, NameAnswer};

fn placed(
    block: &Arc<BlockDef>,
    at: Point,
    scale: (f64, f64),
    rotation: f64,
    attributes: Vec<AttributeReference>,
) -> Entity {
    Entity::Insert(Box::new(Insert {
        block: block.clone(),
        insertion: at,
        scale_x: scale.0,
        scale_y: scale.1,
        rotation,
        attributes,
    }))
}

/// A scale factor: any finite number but zero (negative mirrors).
fn get_factor(ed: &Editor, message: &str, default: f64) -> Result<f64, Cancelled> {
    loop {
        let factor = ed.get_number(NumberOptions::new(message).default(default))?;
        if factor.is_finite() && factor != 0.0 {
            return Ok(factor);
        }
        ed.say("The scale factor cannot be zero.");
    }
}

/// Most block names offered as options at the name prompt; `?` lists them
/// all.
const MAX_OFFERED: usize = 20;

/// The block names to pick from at the name prompt, in alphabetical order
/// ignoring case.
fn offered_names(doc: &od_core::Document) -> Vec<String> {
    let mut names: Vec<String> = doc.blocks().iter().map(|b| b.name.clone()).collect();
    names.sort_by_key(|n| n.to_lowercase());
    names.truncate(MAX_OFFERED);
    names
}

/// The block to insert, asked by name or picked from the options; `None` when the user ends the
/// command.
pub(crate) fn choose_block(ed: &Editor) -> Result<Option<Arc<BlockDef>>, Cancelled> {
    let (newest, names) = ed.read(|doc| {
        let newest = doc.blocks().last().map(|b| b.name.clone());
        (newest, offered_names(doc))
    })?;
    let Some(newest) = newest else {
        ed.say("This drawing has no blocks. Make one with BLOCK first.");
        return Ok(None);
    };
    loop {
        let mut options = Options::new("Enter block name").default(newest.clone());
        for name in &names {
            options = options.keyword(name.clone(), name.clone());
        }
        let name = match ask_block_name(ed, options)? {
            NameAnswer::Done => return Ok(None),
            NameAnswer::List => {
                for line in ed.read(list_blocks)? {
                    ed.say(line);
                }
                continue;
            }
            NameAnswer::Name(name) => name,
        };
        let lookup = name.clone();
        match ed.read(move |doc| doc.block_named(&lookup).cloned())? {
            Some(block) => return Ok(Some(block)),
            None => ed.say(format!("Block \"{name}\" not found.")),
        }
    }
}

/// INSERT, as AutoCAD 2020's INSERT places a block: a block name, then the
/// insertion point, and the block is placed at full size and unturned.
/// Scale and Rotate at the point prompt change that first, so the preview
/// shows them. A block in other units than the drawing's (their `INSUNITS`)
/// is scaled by the ratio, on top of the scale chosen. One undo step.
pub fn insert_block(ed: &mut Editor) -> Result<(), Cancelled> {
    let Some(block) = choose_block(ed)? else {
        return Ok(());
    };
    let from = block.unit;
    let units = ed.read(move |doc| InsertUnits::ratio(from, doc.units().insert))?;
    let mut scale = (1.0, 1.0);
    let mut rotation = 0.0;
    let at = loop {
        let (shown, s, r) = (block.clone(), (scale.0 * units, scale.1 * units), rotation);
        let options = PointOptions::new("Specify insertion point")
            .keyword("Scale", "S")
            .keyword("Rotate", "R")
            .ghost(move |at| {
                let defaults = shown.new_references(|d| d.default_value.clone());
                vec![placed(&shown, at, s, r, defaults)]
            });
        match ed.ask_point(options)? {
            Answer::Value(at) => break at,
            Answer::Keyword(k) if k == "Scale" => {
                let f = get_factor(ed, "Specify scale factor for XY axes", scale.0)?;
                scale = (f, f);
            }
            Answer::Keyword(_) => {
                let options = NumberOptions::new("Specify rotation angle").default(rotation);
                rotation = ed.get_angle(options)?;
            }
            Answer::Enter => {}
        }
    };
    let attributes = ask_values(ed, &block)?;
    let scale = (scale.0 * units, scale.1 * units);
    let entity = placed(&block, at, scale, rotation, attributes);
    ed.transaction("Insert", move |tx| {
        tx.add(entity);
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_cmd::define_block;
    use crate::state::AppState;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    /// A drawing with block "Box": a 2 by 1 rectangle with its base point at
    /// its lower left corner, (10, 10).
    fn with_box() -> AppState {
        with_box_in(InsertUnits::UNITLESS)
    }

    /// As [`with_box`], made while the drawing's insert units are `units`.
    fn with_box_in(units: InsertUnits) -> AppState {
        let mut state = AppState::new();
        let mut settings = *state.session().document().units();
        settings.insert = units;
        state.set_units(settings);
        state.add_entity(Entity::Rectangle {
            min: p(10.0, 10.0),
            max: p(12.0, 11.0),
        });
        state.select_all();
        state.start_command("BLOCK", define_block);
        state.settle_command();
        for answer in ["Box", "10,10"] {
            state.cmd_submit(answer);
            state.settle_command();
        }
        assert!(!state.command_running());
        state
    }

    fn submit_all(state: &mut AppState, answers: &[&str]) {
        for answer in answers {
            state.cmd_submit(answer);
            state.settle_command();
        }
    }

    #[test]
    fn scale_and_rotate_options_come_before_the_point() {
        let mut state = with_box();
        state.start_command("INSERT", insert_block);
        state.settle_command();
        submit_all(&mut state, &["box", "S", "2", "R", "90", "100,0"]);
        assert!(!state.command_running());
        let doc = &state.session().document();
        let Some(Entity::Insert(i)) = doc.objects().last().map(|o| &o.entity) else {
            panic!("an insert was added")
        };
        assert_eq!((i.scale_x, i.scale_y), (2.0, 2.0));
        let b = i.bounds();
        assert!((b.min.x - 98.0).abs() < 1e-9 && (b.max.y - 4.0).abs() < 1e-9);
        assert_eq!(doc.validate(), Ok(()));
        state.undo();
        assert_eq!(state.session().document().len(), 1);
    }

    #[test]
    fn a_block_in_other_units_is_scaled_to_the_drawings() {
        let mm = InsertUnits::from_code(4).unwrap();
        let mut state = with_box_in(mm);
        let mut units = *state.session().document().units();
        units.insert = InsertUnits::from_code(6).unwrap();
        state.set_units(units);
        state.start_command("INSERT", insert_block);
        state.settle_command();
        submit_all(&mut state, &["box", "5,5"]);
        let doc = state.session().document();
        let Some(Entity::Insert(i)) = doc.objects().last().map(|o| &o.entity) else {
            panic!("an insert was added")
        };
        assert!((i.scale_x - 0.001).abs() < 1e-12, "{}", i.scale_x);
    }

    #[test]
    fn an_unknown_name_is_asked_again_and_the_scale_option_comes_first() {
        let mut state = with_box();
        state.start_command("INSERT", insert_block);
        state.settle_command();
        submit_all(&mut state, &["Nothing", "", "S", "3", "5,5"]);
        assert!(state
            .cmdline
            .log
            .iter()
            .any(|l| l.contains("\"Nothing\" not found")));
        let doc = &state.session().document();
        let Some(Entity::Insert(i)) = doc.objects().last().map(|o| &o.entity) else {
            panic!("the default block was inserted")
        };
        assert_eq!((i.scale_x, i.insertion), (3.0, p(5.0, 5.0)));
    }

    #[test]
    fn every_block_is_offered_as_an_option_and_picking_one_inserts_it() {
        let mut state = with_box();
        state.add_entity(Entity::Circle {
            center: p(0.0, 0.0),
            radius: 1.0,
        });
        state.docs[state.active].selection.clear();
        let circle = state.session().document().objects()[1].id;
        state.select_ids([circle], false);
        state.start_command("BLOCK", define_block);
        state.settle_command();
        submit_all(&mut state, &["anchor", "0,0"]);

        state.start_command("INSERT", insert_block);
        state.settle_command();
        let labels: Vec<_> = state
            .cmd_prompt()
            .options
            .iter()
            .map(|o| o.label.to_string())
            .collect();
        assert_eq!(labels, ["anchor", "Box", "?"]);
        state.answer_keyword("Box");
        state.settle_command();
        submit_all(&mut state, &["50,50"]);
        assert!(
            !state.command_running(),
            "placed at the point, nothing more asked"
        );
        let doc = &state.session().document();
        let Some(Entity::Insert(i)) = doc.objects().last().map(|o| &o.entity) else {
            panic!("an insert was added")
        };
        assert_eq!(i.block.name, "Box");
    }

    #[test]
    fn a_drawing_without_blocks_says_so() {
        let mut state = AppState::new();
        state.start_command("INSERT", insert_block);
        state.settle_command();
        assert!(!state.command_running());
        assert!(state.cmdline.log.iter().any(|l| l.contains("no blocks")));
    }
}
