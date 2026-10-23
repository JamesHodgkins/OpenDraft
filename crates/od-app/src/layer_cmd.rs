//! The layer tools of the Home tab's Layers panel, worded as AutoCAD's:
//! LAYFRZ, LAYTHW, LAYULK, LAYMCUR and LAYMCH. (LAYOFF, LAYON and LAYLCK
//! work on the selection: `layers.rs`.)

use od_commands::{
    Answer, Cancelled, Command, Editor, ModifyAttributes, ModifyLayers, Options, Picked,
    SetCurrentLayer, TextOptions,
};
use od_core::{EntityId, Layer, LayerId};

use crate::state::AppState;
use crate::text::plural;

/// The layer commands, for the command table.
pub fn commands() -> Vec<Command<AppState>> {
    // As `editor_command!` in `commands.rs`, for a drawing with objects.
    macro_rules! editor {
        ($id:literal, $label:literal, $name:literal, $run:path) => {
            Command::new($id, $label, |s: &mut AppState| s.start_command($name, $run))
                .checked(|s: &AppState| s.is_running($name))
                .transparent()
                .when(|s: &AppState| s.session().document().has_objects_here())
        };
    }
    vec![
        editor!("layer.freeze", "Freeze", "LAYFRZ", freeze_layers),
        editor!("layer.unlock", "Unlock", "LAYULK", unlock_layer),
        editor!(
            "layer.make_current",
            "Make Current",
            "LAYMCUR",
            make_current
        ),
        editor!("layer.match", "Match Layer", "LAYMCH", match_layer),
        Command::new(
            "layer.thaw_all",
            "Thaw All Layers",
            AppState::thaw_all_layers,
        )
        .when(|s: &AppState| s.session().document().layers().iter().any(|l| l.frozen)),
    ]
}

impl AppState {
    /// LAYTHW: thaw every frozen layer, as one undo step.
    pub fn thaw_all_layers(&mut self) {
        let thawed: Vec<Layer> = self
            .session()
            .document()
            .layers()
            .iter()
            .filter(|l| l.frozen)
            .map(|l| Layer {
                frozen: false,
                ..l.clone()
            })
            .collect();
        if thawed.is_empty() {
            return;
        }
        self.message = "All layers have been thawed.".to_owned();
        self.cmdline.say(self.message.clone());
        self.perform(ModifyLayers::new("Thaw all layers", thawed));
    }
}

/// The layer object `id` is on, as the drawing is now.
fn layer_of(ed: &Editor, id: EntityId) -> Result<Option<Layer>, Cancelled> {
    ed.read(move |doc| {
        let object = doc.object(id)?;
        doc.layer(object.attrs.layer).cloned()
    })
}

/// Replace one layer record as an undo step.
fn change_layer(ed: &Editor, name: &'static str, layer: Layer) -> Result<(), Cancelled> {
    ed.transaction(name, move |tx| {
        tx.perform(ModifyLayers::new(name, vec![layer]));
        Ok(())
    })
}

/// LAYFRZ: freeze the layer of each object picked until Enter. The current
/// layer cannot be frozen. Undo thaws the layer frozen last.
pub fn freeze_layers(ed: &mut Editor) -> Result<(), Cancelled> {
    let mut frozen = 0;
    loop {
        let options =
            Options::<Picked>::new("Select an object on the layer to be frozen or [Undo]")
                .keyword("Undo", "U")
                .allow_empty();
        match ed.ask_object(options)? {
            Answer::Value(Picked { id, .. }) => {
                let current = ed.read(|doc| doc.current_layer())?;
                let Some(layer) = layer_of(ed, id)? else {
                    continue;
                };
                if layer.id == current {
                    ed.say(format!(
                        "Cannot freeze layer \"{}\". It is the current layer.",
                        layer.name
                    ));
                    continue;
                }
                let name = layer.name.clone();
                change_layer(
                    ed,
                    "Freeze layer",
                    Layer {
                        frozen: true,
                        ..layer
                    },
                )?;
                frozen += 1;
                ed.say(format!("Layer \"{name}\" has been frozen."));
            }
            Answer::Keyword(_) if frozen > 0 => {
                ed.undo()?;
                frozen -= 1;
                ed.say("Layer thawed again.");
            }
            Answer::Keyword(_) => ed.say("Nothing to undo."),
            Answer::Enter => return Ok(()),
        }
    }
}

/// LAYULK: unlock the layer of the object picked; objects on locked layers
/// can be picked here.
pub fn unlock_layer(ed: &mut Editor) -> Result<(), Cancelled> {
    let options = Options::<Picked>::new("Select an object on the layer to be unlocked")
        .pick_locked()
        .allow_empty();
    let Answer::Value(Picked { id, .. }) = ed.ask_object(options)? else {
        return Ok(());
    };
    let Some(layer) = layer_of(ed, id)? else {
        return Ok(());
    };
    if !layer.locked {
        ed.say(format!("Layer \"{}\" is not locked.", layer.name));
        return Ok(());
    }
    let name = layer.name.clone();
    change_layer(
        ed,
        "Unlock layer",
        Layer {
            locked: false,
            ..layer
        },
    )?;
    ed.say(format!("Layer \"{name}\" has been unlocked."));
    Ok(())
}

/// The one layer all of `ids` are on, if they share one.
fn shared_layer(ed: &Editor, ids: Vec<EntityId>) -> Result<Option<LayerId>, Cancelled> {
    ed.read(move |doc| {
        let mut layers = ids
            .iter()
            .filter_map(|id| doc.object(*id))
            .map(|o| o.attrs.layer);
        let first = layers.next()?;
        layers.all(|l| l == first).then_some(first)
    })
}

/// LAYMCUR: make the layer of the object picked current. Objects already
/// selected on one layer answer the question.
pub fn make_current(ed: &mut Editor) -> Result<(), Cancelled> {
    let preselected = shared_layer(ed, ed.pickset().to_vec())?;
    let layer = match preselected {
        Some(layer) => layer,
        None => {
            let options = Options::<Picked>::new("Select object whose layer will become current")
                .allow_empty();
            let Answer::Value(Picked { id, .. }) = ed.ask_object(options)? else {
                return Ok(());
            };
            match layer_of(ed, id)? {
                Some(layer) => layer.id,
                None => return Ok(()),
            }
        }
    };
    let name = ed.transaction("Make object's layer current", move |tx| {
        let name = tx.document().layer(layer).map(|l| l.name.clone());
        if tx.document().current_layer() != layer {
            tx.perform(SetCurrentLayer::new(layer));
        }
        Ok(name)
    })?;
    if let Some(name) = name {
        ed.say(format!("{name} is now the current layer."));
    }
    Ok(())
}

/// LAYMCH: move the selected objects to the layer of an object picked, or
/// of a layer named. One undo step.
pub fn match_layer(ed: &mut Editor) -> Result<(), Cancelled> {
    let ids = ed.pickset_or_select("Select objects to be changed")?;
    let Some(layer) = destination(ed)? else {
        return Ok(());
    };
    let (changed, name) = ed.transaction("Match layer", move |tx| {
        let doc = tx.document();
        let name = doc.layer(layer).map(|l| l.name.clone()).unwrap_or_default();
        let changes: Vec<_> = ids
            .iter()
            .filter_map(|id| doc.object(*id))
            .filter(|o| o.attrs.layer != layer)
            .map(|o| {
                let mut attrs = o.attrs.clone();
                attrs.layer = layer;
                (o.id, attrs)
            })
            .collect();
        let changed = changes.len();
        if changed > 0 {
            tx.perform(ModifyAttributes::new("Match layer", changes));
        }
        Ok((changed, name))
    })?;
    ed.say(format!(
        "{} changed to layer \"{name}\".",
        plural(changed, "object")
    ));
    Ok(())
}

/// The layer LAYMCH moves objects to: an object's, or one typed by name.
fn destination(ed: &mut Editor) -> Result<Option<LayerId>, Cancelled> {
    loop {
        let options = Options::<Picked>::new("Select object on destination layer or [Name]")
            .keyword("Name", "N")
            .allow_empty();
        match ed.ask_object(options)? {
            Answer::Value(Picked { id, .. }) => return Ok(layer_of(ed, id)?.map(|l| l.id)),
            Answer::Keyword(_) => {
                let Answer::Value(typed) = ed.ask_string(TextOptions::new("Enter layer name"))?
                else {
                    continue;
                };
                let wanted = typed.trim().to_owned();
                let found = ed.read(move |doc| {
                    let layers = doc.layers().iter();
                    layers
                        .filter(|l| l.name.eq_ignore_ascii_case(&wanted))
                        .map(|l| l.id)
                        .next()
                })?;
                match found {
                    Some(layer) => return Ok(Some(layer)),
                    None => ed.say(format!("Layer \"{}\" not found.", typed.trim())),
                }
            }
            Answer::Enter => return Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use od_commands::Reply;
    use od_core::{Attributes, Entity, Point};

    use super::*;

    fn line(y: f64) -> Entity {
        Entity::Line {
            start: Point::new(0.0, y),
            end: Point::new(10.0, y),
        }
    }

    /// A drawing with a line on layer 0 and one on a new layer "Layer1".
    fn drawing() -> (AppState, [EntityId; 2], LayerId) {
        let mut state = AppState::new();
        state.add_entity(line(0.0));
        state.add_entity(line(5.0));
        let walls = state.new_layer();
        let ids = {
            let objects = state.session().document().objects();
            [objects[0].id, objects[1].id]
        };
        state.perform(ModifyAttributes::new(
            "To walls",
            vec![(ids[1], Attributes::on_layer(walls))],
        ));
        (state, ids, walls)
    }

    fn layer(state: &AppState, id: LayerId) -> Layer {
        state.session().document().layer(id).cloned().unwrap()
    }

    fn pick(state: &mut AppState, id: EntityId) {
        state.answer_object(Point::new(1.0, 0.0), Some(id), false);
        state.settle_command();
    }

    fn said(state: &AppState) -> &str {
        state.cmdline.log.last().map_or("", String::as_str)
    }

    #[test]
    fn freeze_freezes_each_picked_layer_but_not_the_current_one() {
        let (mut state, ids, walls) = drawing();
        state.start_command("LAYFRZ", freeze_layers);
        state.settle_command();
        pick(&mut state, ids[0]);
        assert!(said(&state).contains("It is the current layer"));
        assert!(!layer(&state, LayerId::DEFAULT).frozen);
        pick(&mut state, ids[1]);
        assert!(layer(&state, walls).frozen);
        assert_eq!(said(&state), "Layer \"Layer1\" has been frozen.");
        // Undo thaws it again, inside the command.
        state.answer_keyword("Undo");
        state.settle_command();
        assert!(!layer(&state, walls).frozen);
        state.answer_enter();
        state.settle_command();
        assert!(!state.command_running());
    }

    #[test]
    fn thaw_all_thaws_every_layer_in_one_step() {
        let (mut state, _, walls) = drawing();
        let mut frozen = layer(&state, walls);
        frozen.frozen = true;
        state.perform(ModifyLayers::new("Freeze", vec![frozen]));
        state.thaw_all_layers();
        assert!(!layer(&state, walls).frozen);
        state.undo();
        assert!(layer(&state, walls).frozen);
    }

    #[test]
    fn unlock_unlocks_the_layer_of_the_object_picked() {
        let (mut state, ids, walls) = drawing();
        let mut locked = layer(&state, walls);
        locked.locked = true;
        state.perform(ModifyLayers::new("Lock", vec![locked]));
        state.start_command("LAYULK", unlock_layer);
        state.settle_command();
        assert!(state.waiting().is_some_and(|ask| ask.pick_locked));
        pick(&mut state, ids[1]);
        assert!(!layer(&state, walls).locked);
        assert_eq!(said(&state), "Layer \"Layer1\" has been unlocked.");
        assert!(!state.command_running());
    }

    #[test]
    fn make_current_uses_the_picked_or_preselected_objects_layer() {
        let (mut state, ids, walls) = drawing();
        state.start_command("LAYMCUR", make_current);
        state.settle_command();
        pick(&mut state, ids[1]);
        assert_eq!(state.session().document().current_layer(), walls);
        assert_eq!(said(&state), "Layer1 is now the current layer.");

        state.click_select(Some(ids[0]), false);
        state.start_command("LAYMCUR", make_current);
        state.settle_command();
        assert_eq!(state.session().document().current_layer(), LayerId::DEFAULT);
        state.undo();
        assert_eq!(state.session().document().current_layer(), walls);
    }

    #[test]
    fn match_layer_moves_the_selection_to_a_picked_or_named_layer() {
        let (mut state, ids, walls) = drawing();
        let on = |state: &AppState, id| state.session().document().object(id).unwrap().attrs.layer;
        state.click_select(Some(ids[0]), false);
        state.start_command("LAYMCH", match_layer);
        state.settle_command();
        pick(&mut state, ids[1]);
        assert_eq!(on(&state, ids[0]), walls);
        assert_eq!(said(&state), "1 object changed to layer \"Layer1\".");

        // With nothing selected it asks for the objects first.
        state.cancel();
        assert!(state.selection().is_empty());
        state.start_command("LAYMCH", match_layer);
        state.settle_command();
        state.reply(Reply::Selection(vec![ids[0], ids[1]]));
        state.settle_command();
        state.answer_keyword("Name");
        state.settle_command();
        state.reply(Reply::Text("nowhere".to_owned()));
        state.settle_command();
        assert_eq!(said(&state), "Layer \"nowhere\" not found.");
        state.answer_keyword("Name");
        state.settle_command();
        state.reply(Reply::Text("0".to_owned()));
        state.settle_command();
        assert_eq!(on(&state, ids[0]), LayerId::DEFAULT);
        assert_eq!(on(&state, ids[1]), LayerId::DEFAULT);
        state.undo();
        assert_eq!(on(&state, ids[1]), walls);
    }
}
