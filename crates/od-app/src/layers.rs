//! Layer operations on the application state: what the Layers panel, the
//! Properties panel and the layer commands ask for. No UI code here.

use od_commands::{
    AddLayer, ModifyAttributes, ModifyLayers, RemoveLayer, SetCurrentLayer, SetNewObjectStyle,
};
use od_core::{Attr, Color, EntityKind, Layer, LayerId, LinetypeId, Lineweight};

use crate::state::AppState;
use crate::text::plural;

/// A switch on a layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerFlag {
    On,
    Frozen,
    Locked,
}

impl LayerFlag {
    fn get(self, layer: &Layer) -> bool {
        match self {
            LayerFlag::On => layer.on,
            LayerFlag::Frozen => layer.frozen,
            LayerFlag::Locked => layer.locked,
        }
    }

    fn set(self, layer: &mut Layer, value: bool) {
        match self {
            LayerFlag::On => layer.on = value,
            LayerFlag::Frozen => layer.frozen = value,
            LayerFlag::Locked => layer.locked = value,
        }
    }

    /// Undo name for turning the flag to `value`.
    fn action_name(self, value: bool) -> &'static str {
        match (self, value) {
            (LayerFlag::On, true) => "Turn layer on",
            (LayerFlag::On, false) => "Turn layer off",
            (LayerFlag::Frozen, true) => "Freeze layer",
            (LayerFlag::Frozen, false) => "Thaw layer",
            (LayerFlag::Locked, true) => "Lock layer",
            (LayerFlag::Locked, false) => "Unlock layer",
        }
    }
}

/// What the Layers panel asks the application to do.
#[derive(Clone, Debug, PartialEq)]
pub enum LayerEdit {
    New,
    Delete(LayerId),
    SetCurrent(LayerId),
    Rename(LayerId, String),
    Toggle(LayerId, LayerFlag),
    SetColor(LayerId, Color),
    SetLinetype(LayerId, LinetypeId),
    SetLineweight(LayerId, Lineweight),
    /// Freeze or thaw a layer in the viewport being worked on (VP Freeze).
    ToggleInViewport(LayerId),
}

/// A change to the selected objects' display attributes (Properties panel).
#[derive(Clone, Debug, PartialEq)]
pub enum AttrEdit {
    Layer(LayerId),
    Color(Attr<Color>),
    Linetype(Attr<LinetypeId>),
    Lineweight(Attr<Lineweight>),
    /// The objects' own linetype scale, above zero.
    LinetypeScale(f64),
    /// Polylines and rectangles only: one pattern along the whole outline.
    LinetypeGeneration(bool),
}

impl AttrEdit {
    fn name(&self) -> &'static str {
        match self {
            AttrEdit::Layer(_) => "Change layer",
            AttrEdit::Color(_) => "Change colour",
            AttrEdit::Linetype(_) => "Change linetype",
            AttrEdit::Lineweight(_) => "Change lineweight",
            AttrEdit::LinetypeScale(_) => "Change linetype scale",
            AttrEdit::LinetypeGeneration(_) => "Change linetype generation",
        }
    }
}

/// Whether Linetype Generation means something for this entity: it has
/// vertices a pattern could run through.
pub fn has_outline(entity: &od_core::Entity) -> bool {
    matches!(
        entity,
        od_core::Entity::Polyline { .. } | od_core::Entity::Rectangle { .. }
    )
}

impl AppState {
    pub fn apply_layer_edit(&mut self, edit: LayerEdit) {
        match edit {
            LayerEdit::New => {
                self.new_layer();
            }
            LayerEdit::Delete(id) => self.delete_layer(id),
            LayerEdit::SetCurrent(id) => self.set_current_layer(id),
            LayerEdit::Rename(id, name) => self.rename_layer(id, &name),
            LayerEdit::Toggle(id, flag) => self.toggle_layer(id, flag),
            LayerEdit::SetColor(id, color) => {
                self.edit_layers("Change layer colour", [id], |l| l.color = color)
            }
            LayerEdit::SetLinetype(id, linetype) => {
                self.edit_layers("Change layer linetype", [id], |l| l.linetype = linetype)
            }
            LayerEdit::SetLineweight(id, weight) => {
                self.edit_layers("Change layer lineweight", [id], |l| l.lineweight = weight)
            }
            LayerEdit::ToggleInViewport(id) => self.toggle_layer_in_viewport(id),
        }
    }

    /// Add a layer with a fresh name and return its id.
    pub fn new_layer(&mut self) -> LayerId {
        let doc = &mut self.docs[self.active];
        let id = doc.session.allocate_layer_id();
        let name = doc.session.document().unused_layer_name();
        self.message = format!("Created layer {name}");
        self.perform(AddLayer::new(Layer::new(id, name)));
        id
    }

    pub fn delete_layer(&mut self, id: LayerId) {
        if let Some(reason) = self.session().document().layer_delete_blocker(id) {
            self.message = reason.to_owned();
            return;
        }
        self.message = "Deleted layer".to_owned();
        self.perform(RemoveLayer::new(id));
    }

    pub fn set_current_layer(&mut self, id: LayerId) {
        let document = &self.session().document();
        if document.current_layer() == id {
            return;
        }
        let Some(layer) = document.layer(id) else {
            return;
        };
        self.message = format!("Current layer: {}", layer.name);
        self.perform(SetCurrentLayer::new(id));
    }

    pub fn rename_layer(&mut self, id: LayerId, name: &str) {
        let name = name.trim();
        let document = &self.session().document();
        if document.layer(id).is_some_and(|l| l.name == name) {
            return;
        }
        if id == LayerId::DEFAULT {
            self.message = "Layer 0 cannot be renamed".to_owned();
        } else if name.is_empty() {
            self.message = "A layer needs a name".to_owned();
        } else if document.layer_name_taken(name, Some(id)) {
            self.message = format!("A layer called {name} already exists");
        } else {
            self.edit_layers("Rename layer", [id], |l| l.name = name.to_owned());
        }
    }

    pub fn toggle_layer(&mut self, id: LayerId, flag: LayerFlag) {
        let document = &self.session().document();
        let Some(layer) = document.layer(id) else {
            return;
        };
        let value = !flag.get(layer);
        if flag == LayerFlag::Frozen && value && id == document.current_layer() {
            self.message = "The current layer cannot be frozen".to_owned();
            return;
        }
        self.edit_layers(flag.action_name(value), [id], |l| flag.set(l, value));
    }

    /// Apply `change` to the given layers as one undo step. Does nothing when
    /// no layer would change.
    fn edit_layers(
        &mut self,
        name: &str,
        ids: impl IntoIterator<Item = LayerId>,
        change: impl Fn(&mut Layer),
    ) {
        let document = &self.session().document();
        let changed: Vec<Layer> = ids
            .into_iter()
            .filter_map(|id| {
                let old = document.layer(id)?;
                let mut new = old.clone();
                change(&mut new);
                (new != *old).then_some(new)
            })
            .collect();
        if changed.is_empty() {
            return;
        }
        self.perform(ModifyLayers::new(name, changed));
    }

    /// The layers the selected objects are on, in display order.
    fn selected_layers(&self) -> Vec<LayerId> {
        let doc = &self.docs[self.active];
        doc.session
            .document()
            .layers()
            .iter()
            .map(|l| l.id)
            .filter(|&id| {
                doc.selection
                    .iter()
                    .filter_map(|&o| doc.session.document().object(o))
                    .any(|o| o.attrs.layer == id)
            })
            .collect()
    }

    /// Turn off the layers of the selected objects (hides them).
    pub fn hide_selected_layers(&mut self) {
        let layers = self.selected_layers();
        self.message = format!("Turned off {}", plural(layers.len(), "layer"));
        self.edit_layers("Turn layers off", layers, |l| l.on = false);
    }

    /// Lock the layers of the selected objects.
    pub fn lock_selected_layers(&mut self) {
        let layers = self.selected_layers();
        self.message = format!("Locked {}", plural(layers.len(), "layer"));
        self.edit_layers("Lock layers", layers, |l| l.locked = true);
    }

    /// Turn every layer back on.
    pub fn show_all_layers(&mut self) {
        let layers: Vec<LayerId> = self
            .session()
            .document()
            .layers()
            .iter()
            .map(|l| l.id)
            .collect();
        self.message = "All layers turned on".to_owned();
        self.edit_layers("Turn layers on", layers, |l| {
            l.on = true;
            l.frozen = false;
        });
    }

    /// Whether any layer is off or frozen.
    pub fn any_layer_hidden(&self) -> bool {
        self.session()
            .document()
            .layers()
            .iter()
            .any(|l| !l.is_visible())
    }

    /// Change the colour, linetype or lineweight new objects are drawn with
    /// (what the ribbon lists set when nothing is selected).
    pub fn set_new_object_style(&mut self, edit: &AttrEdit) {
        let (color, linetype, lineweight) = self.session().document().new_object_style();
        let (mut color, mut linetype, mut lineweight) =
            (color.clone(), linetype.clone(), lineweight.clone());
        match edit {
            AttrEdit::Color(c) => color = c.clone(),
            AttrEdit::Linetype(t) => linetype = t.clone(),
            AttrEdit::Lineweight(w) => lineweight = w.clone(),
            _ => return,
        }
        self.message = format!("{} for new objects", edit.name());
        self.perform(SetNewObjectStyle::new(color, linetype, lineweight));
    }

    /// Change the layer, colour, linetype or lineweight of the selected
    /// objects (one undo step). `kind` limits it to one kind of entity (the
    /// Properties type filter).
    pub fn set_attrs(&mut self, edit: &AttrEdit, kind: Option<EntityKind>) {
        let doc = &self.docs[self.active];
        let changes: Vec<_> = doc
            .selection
            .iter()
            .filter_map(|&id| doc.session.document().object(id))
            .filter(|o| kind.is_none_or(|k| o.entity.kind() == k))
            .filter_map(|o| {
                let mut attrs = o.attrs.clone();
                match edit {
                    AttrEdit::Layer(layer) => attrs.layer = *layer,
                    AttrEdit::Color(c) => attrs.color = c.clone(),
                    AttrEdit::Linetype(t) => attrs.linetype = t.clone(),
                    AttrEdit::Lineweight(w) => attrs.lineweight = w.clone(),
                    AttrEdit::LinetypeScale(s) => attrs.linetype_scale = *s,
                    AttrEdit::LinetypeGeneration(on) if has_outline(&o.entity) => {
                        attrs.continuous_generation = *on;
                    }
                    AttrEdit::LinetypeGeneration(_) => {}
                }
                (attrs != o.attrs).then_some((o.id, attrs))
            })
            .collect();
        if changes.is_empty() {
            return;
        }
        self.message = format!("{} ({})", edit.name(), plural(changes.len(), "object"));
        self.perform(ModifyAttributes::new(edit.name(), changes));
    }
}

#[cfg(test)]
mod tests {
    use od_core::{Entity, Point};

    use super::*;

    fn circle() -> Entity {
        Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: 1.0,
        }
    }

    fn layer_named(state: &AppState, name: &str) -> Layer {
        state
            .session()
            .document()
            .layers()
            .iter()
            .find(|l| l.name == name)
            .cloned()
            .unwrap()
    }

    #[test]
    fn new_layers_get_unused_names_and_undo() {
        let mut state = AppState::new();
        let a = state.new_layer();
        state.new_layer();
        let names: Vec<_> = state
            .session()
            .document()
            .layers()
            .iter()
            .map(|l| l.name.clone())
            .collect();
        assert_eq!(names, ["0", "Layer1", "Layer2"]);
        state.undo();
        assert!(state.session().document().layer(a).is_some());
        assert_eq!(state.session().document().layers().len(), 2);
    }

    #[test]
    fn rename_rejects_empty_duplicate_and_layer_zero() {
        let mut state = AppState::new();
        let a = state.new_layer();
        state.rename_layer(a, "  ");
        assert!(state.message.contains("needs a name"));
        state.rename_layer(a, "0");
        assert!(state.message.contains("already exists"));
        state.rename_layer(LayerId::DEFAULT, "Zero");
        assert!(state.message.contains("cannot be renamed"));
        state.rename_layer(a, " Walls ");
        assert_eq!(state.session().document().layer(a).unwrap().name, "Walls");
    }

    #[test]
    fn hiding_a_layer_drops_its_objects_from_the_selection() {
        let mut state = AppState::new();
        let walls = state.new_layer();
        state.set_current_layer(walls);
        state.add_entity(circle());
        state.select_all();
        assert_eq!(state.selection().len(), 1);

        state.toggle_layer(walls, LayerFlag::On);
        assert!(state.selection().is_empty());
        state.undo();
        assert!(state.session().document().layer(walls).unwrap().on);
    }

    #[test]
    fn locking_stops_selection_but_not_display() {
        let mut state = AppState::new();
        state.add_entity(circle());
        state.select_all();
        state.lock_selected_layers();
        assert!(state.selection().is_empty());
        assert!(layer_named(&state, "0").locked);
        state.select_all();
        assert!(
            state.selection().is_empty(),
            "locked objects are not selectable"
        );
    }

    #[test]
    fn the_current_layer_cannot_be_frozen_or_deleted() {
        let mut state = AppState::new();
        let walls = state.new_layer();
        state.set_current_layer(walls);
        state.toggle_layer(walls, LayerFlag::Frozen);
        assert!(!state.session().document().layer(walls).unwrap().frozen);
        state.delete_layer(walls);
        assert!(state.session().document().layer(walls).is_some());
        assert!(state.message.contains("current layer"));
    }

    #[test]
    fn hide_selected_layers_then_show_all() {
        let mut state = AppState::new();
        state.add_entity(circle());
        state.select_all();
        state.hide_selected_layers();
        assert!(state.any_layer_hidden());
        state.show_all_layers();
        assert!(!state.any_layer_hidden());
        state.undo();
        assert!(state.any_layer_hidden());
    }

    #[test]
    fn set_attrs_moves_selected_objects_and_skips_no_ops() {
        let mut state = AppState::new();
        let walls = state.new_layer();
        state.add_entity(circle());
        state.add_entity(circle());
        state.select_all();

        state.set_attrs(&AttrEdit::Layer(walls), None);
        let doc = &state.session().document();
        assert!(doc.objects().iter().all(|o| o.attrs.layer == walls));

        // Same layer again: nothing to do, so no extra undo step.
        state.set_attrs(&AttrEdit::Layer(walls), None);
        state.undo();
        let doc = &state.session().document();
        assert!(doc
            .objects()
            .iter()
            .all(|o| o.attrs.layer == LayerId::DEFAULT));
        assert_eq!(doc.layers().len(), 2, "the layer itself was not undone");
    }

    #[test]
    fn set_attrs_respects_the_kind_filter() {
        let mut state = AppState::new();
        state.add_entity(circle());
        state.add_entity(Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(1.0, 0.0),
        });
        state.select_all();
        let red = Attr::Set(Color::Rgb(255, 0, 0));
        state.set_attrs(&AttrEdit::Color(red.clone()), Some(EntityKind::Line));
        let doc = &state.session().document();
        assert_eq!(doc.objects()[0].attrs.color, Attr::ByLayer);
        assert_eq!(doc.objects()[1].attrs.color, red);
    }

    #[test]
    fn linetype_scale_applies_to_all_and_generation_only_to_outlines() {
        let mut state = AppState::new();
        state.add_entity(circle());
        state.add_entity(Entity::Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(1.0, 1.0),
            ],
            closed: false,
            bulges: Vec::new(),
            widths: Vec::new(),
        });
        state.select_all();
        state.set_attrs(&AttrEdit::LinetypeScale(2.5), None);
        state.set_attrs(&AttrEdit::LinetypeGeneration(true), None);
        let doc = &state.session().document();
        assert!(doc.objects().iter().all(|o| o.attrs.linetype_scale == 2.5));
        assert!(!doc.objects()[0].attrs.continuous_generation);
        assert!(doc.objects()[1].attrs.continuous_generation);
        state.undo();
        state.undo();
        let doc = &state.session().document();
        assert!(doc.objects().iter().all(|o| o.attrs.linetype_scale == 1.0));
    }
}
