//! HATCHEDIT: change the pattern, scale, angle, spacing, island style or
//! origin of a hatch with the options HATCH has, seeing the result as a
//! preview; Enter or a click applies it as one undo step. Its boundary and
//! its association with the objects it follows are left alone (the
//! boundary is edited through those objects).
//!
//! The options work on HATCH's remembered settings, which this command
//! fills from the hatch and puts back afterwards, so editing a hatch does
//! not change what the next HATCH starts with.

use od_commands::{Answer, Cancelled, Editor, Options, Picked, PointOptions};
use od_core::hatch::{HatchEntity, HatchFill, HatchLimits, PatternType};
use od_core::{Entity, EntityId};

use crate::hatch_cmd::{adjust, check_tolerance, summary, with_keywords};
use crate::hatch_memory::{memory, remember, FillChoice, Memory};

/// Puts HATCH's settings back when the command ends, however it ends.
struct Restore(Memory);

impl Drop for Restore {
    fn drop(&mut self) {
        let saved = self.0.clone();
        remember(|m| *m = saved);
    }
}

/// The settings that describe `h`, or why they cannot (a pattern that is not
/// in the catalog cannot be rebuilt from its name).
fn settings_of(h: &HatchEntity) -> Result<Memory, String> {
    let mut m = memory();
    m.style = h.style;
    match &h.fill {
        HatchFill::Solid => m.fill = FillChoice::Solid,
        HatchFill::Gradient(_) => {
            return Err("A gradient hatch cannot be edited here yet.".to_owned());
        }
        HatchFill::Pattern(p) => {
            m.angle = p.angle;
            m.origin = p.origin;
            if p.pattern_type == PatternType::UserDefined {
                m.fill = FillChoice::UserDefined;
                m.spacing = p.scale_or_spacing;
                m.double = p.double;
            } else {
                m.fill = Memory::choice_named(&p.name)
                    .filter(|c| matches!(c, FillChoice::Named(_)))
                    .ok_or_else(|| {
                        format!(
                            "The pattern {} is not in the catalog; change it in Properties.",
                            p.name
                        )
                    })?;
                m.scale = p.scale_or_spacing;
            }
        }
    }
    Ok(m)
}

/// `h` with the fill and style the settings now describe.
fn edited(h: &HatchEntity, settings: &Memory) -> Result<HatchEntity, String> {
    let fresh = settings.hatch(h.loops.clone())?;
    Ok(HatchEntity {
        elevation: h.elevation,
        associative: h.associative,
        seed_points: h.seed_points.clone(),
        source_pixel_size: h.source_pixel_size,
        ..fresh
    })
}

/// The hatch to edit: the one selected, or one the user picks.
fn choose(ed: &Editor) -> Result<(EntityId, HatchEntity), Cancelled> {
    let selected = ed.pickset().to_vec();
    let find = move |doc: &od_core::Document, ids: &[EntityId]| -> Vec<(EntityId, HatchEntity)> {
        ids.iter()
            .filter_map(|id| match doc.get(*id) {
                Some(Entity::Hatch(h)) => Some((*id, (**h).clone())),
                _ => None,
            })
            .collect()
    };
    let wanted = selected.clone();
    let mut hatches = ed.read(move |doc| find(doc, &wanted))?;
    if let (1, Some(only)) = (hatches.len(), hatches.pop()) {
        return Ok(only);
    }
    loop {
        let Picked { id, .. } =
            match ed.ask_object(Options::<Picked>::new("Select hatch object"))? {
                Answer::Value(p) => p,
                Answer::Enter | Answer::Keyword(_) => return Err(Cancelled),
            };
        let mut found = ed.read(move |doc| find(doc, &[id]))?;
        match found.pop() {
            Some(hatch) => return Ok(hatch),
            None => ed.say("That is not a hatch."),
        }
    }
}

/// HATCHEDIT. See the module notes.
pub fn hatch_edit(ed: &mut Editor) -> Result<(), Cancelled> {
    let (id, hatch) = choose(ed)?;
    let _restore = Restore(memory());
    match settings_of(&hatch) {
        Ok(m) => remember(|now| *now = m),
        Err(why) => {
            ed.say(why);
            return Ok(());
        }
    }
    loop {
        let settings = memory();
        let new = match edited(&hatch, &settings) {
            Ok(h) => h,
            Err(why) => {
                ed.say(format!("Unable to hatch: {why}"));
                adjust(ed, "Pattern")?;
                continue;
            }
        };
        let problem = new
            .evaluate(check_tolerance(&new), &HatchLimits::default())
            .err();
        if let Some(why) = &problem {
            ed.say(format!("The hatch cannot be drawn: {why}."));
        }
        ed.say(summary());
        let shown = new.clone();
        let options = with_keywords(
            PointOptions::new("Press Enter or click to apply the change")
                .ghost(move |_| vec![Entity::Hatch(Box::new(shown.clone()))]),
            &settings.fill,
            false,
        );
        match ed.ask_point(options)? {
            Answer::Keyword(k) => adjust(ed, &k)?,
            Answer::Value(_) | Answer::Enter if problem.is_some() => {
                ed.say("Change the scale or spacing first.");
            }
            Answer::Value(_) | Answer::Enter => {
                ed.transaction("Hatch edit", move |tx| {
                    tx.modify(id, |_| Entity::Hatch(Box::new(new)));
                    Ok(())
                })?;
                ed.say("Hatch changed.");
                return Ok(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use od_core::hatch::HatchStyle;
    use od_core::Point;

    use super::*;
    use crate::hatch_cmd::hatch_objects;
    use crate::hatch_memory::take_turn_for_tests;
    use crate::state::AppState;

    fn hatched() -> AppState {
        let mut state = AppState::new();
        state.add_entity(Entity::Rectangle {
            min: Point::new(0.0, 0.0),
            max: Point::new(10.0, 10.0),
        });
        state.select_all();
        state.start_command("Hatch", hatch_objects);
        state.settle_command();
        state.answer_enter();
        state.settle_command();
        state.click_select(None, false);
        state
    }

    fn the_hatch(state: &AppState) -> (EntityId, HatchEntity) {
        state
            .session()
            .document
            .objects()
            .iter()
            .find_map(|o| match &o.entity {
                Entity::Hatch(h) => Some((o.id, (**h).clone())),
                _ => None,
            })
            .expect("a hatch")
    }

    #[test]
    fn a_hatch_is_edited_in_place_keeping_its_boundary_and_association() {
        let _turn = take_turn_for_tests();
        let mut state = hatched();
        let (id, before) = the_hatch(&state);
        assert!(before.associative);
        state.select_ids([id], false);
        state.start_command("HatchEdit", hatch_edit);
        state.settle_command();
        state.answer_keyword("Style");
        state.settle_command();
        state.answer_keyword("Ignore");
        state.settle_command();
        state.answer_keyword("Scale");
        state.settle_command();
        assert!(state.answer_typed("3"));
        state.settle_command();
        state.answer_enter();
        state.settle_command();
        let (same, after) = the_hatch(&state);
        assert_eq!(same, id, "edited in place");
        assert_eq!(after.style, HatchStyle::Ignore);
        assert!(after.associative);
        assert_eq!(after.loops, before.loops);
        let HatchFill::Pattern(p) = &after.fill else {
            panic!("pattern");
        };
        assert_eq!(p.scale_or_spacing, 3.0);
        // HATCH's own settings are what they were.
        assert_eq!(memory().scale, 1.0);
        assert_eq!(memory().style, HatchStyle::Normal);
        state.undo();
        assert_eq!(the_hatch(&state).1, before);
    }
}
