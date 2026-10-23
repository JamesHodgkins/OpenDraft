//! Undoable edits to a drawing's layers.

use od_core::{Attr, Color, Document, Layer, LayerId, Lineweight};

use crate::{Action, Changes};

/// Add a layer. Get its id from `Document::allocate_layer_id` first so the
/// caller can refer to the new layer.
pub struct AddLayer {
    layer: Layer,
}

impl AddLayer {
    pub fn new(layer: Layer) -> Self {
        Self { layer }
    }
}

impl Action for AddLayer {
    fn name(&self) -> String {
        format!("New layer {}", self.layer.name)
    }

    fn apply(&mut self, doc: &mut Document) {
        doc.push_layer(self.layer.clone());
    }

    fn revert(&mut self, doc: &mut Document) {
        doc.remove_layer(self.layer.id);
    }

    fn changes(&self) -> Changes {
        Changes::layers()
    }
}

/// Replace layer records in one step: rename, recolour, turn on or off,
/// freeze, lock.
pub struct ModifyLayers {
    name: String,
    /// Applying and reverting both swap, so each leaves the other state ready.
    layers: Vec<Layer>,
}

impl ModifyLayers {
    pub fn new(name: impl Into<String>, layers: Vec<Layer>) -> Self {
        Self {
            name: name.into(),
            layers,
        }
    }

    fn swap(&mut self, doc: &mut Document) {
        for layer in &mut self.layers {
            if let Some(old) = doc.replace_layer(layer.clone()) {
                *layer = old;
            }
        }
    }
}

impl Action for ModifyLayers {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn apply(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn revert(&mut self, doc: &mut Document) {
        self.swap(doc);
    }

    fn changes(&self) -> Changes {
        Changes::layers()
    }
}

/// Delete a layer. Does nothing when the drawing says it cannot be deleted
/// (layer 0, the current layer, a layer with objects on it); callers check
/// `Document::layer_delete_blocker` first to tell the user why.
pub struct RemoveLayer {
    id: LayerId,
    removed: Option<(usize, Layer)>,
}

impl RemoveLayer {
    pub fn new(id: LayerId) -> Self {
        Self { id, removed: None }
    }
}

impl Action for RemoveLayer {
    fn name(&self) -> String {
        "Delete layer".to_owned()
    }

    fn apply(&mut self, doc: &mut Document) {
        if doc.layer_delete_blocker(self.id).is_none() {
            self.removed = doc.remove_layer(self.id);
        }
    }

    fn revert(&mut self, doc: &mut Document) {
        if let Some((position, layer)) = self.removed.take() {
            doc.insert_layer(position, layer);
        }
    }

    fn changes(&self) -> Changes {
        Changes::layers()
    }
}

/// Change the layer new objects are drawn on.
pub struct SetCurrentLayer {
    id: LayerId,
    previous: Option<LayerId>,
}

impl SetCurrentLayer {
    pub fn new(id: LayerId) -> Self {
        Self { id, previous: None }
    }
}

impl Action for SetCurrentLayer {
    fn name(&self) -> String {
        "Set current layer".to_owned()
    }

    fn apply(&mut self, doc: &mut Document) {
        self.previous = Some(doc.current_layer());
        doc.set_current_layer(self.id);
    }

    fn revert(&mut self, doc: &mut Document) {
        if let Some(previous) = self.previous.take() {
            doc.set_current_layer(previous);
        }
    }

    fn changes(&self) -> Changes {
        Changes::layers()
    }
}

/// Change the colour, linetype and lineweight new objects are drawn with.
pub struct SetNewObjectStyle {
    style: (Attr<Color>, Attr<String>, Attr<Lineweight>),
    previous: Option<(Attr<Color>, Attr<String>, Attr<Lineweight>)>,
}

impl SetNewObjectStyle {
    pub fn new(color: Attr<Color>, linetype: Attr<String>, lineweight: Attr<Lineweight>) -> Self {
        Self {
            style: (color, linetype, lineweight),
            previous: None,
        }
    }
}

impl Action for SetNewObjectStyle {
    fn name(&self) -> String {
        "Set current properties".to_owned()
    }

    fn apply(&mut self, doc: &mut Document) {
        let (c, t, w) = doc.new_object_style();
        self.previous = Some((c.clone(), t.clone(), w.clone()));
        let (color, linetype, lineweight) = self.style.clone();
        doc.set_new_object_style(color, linetype, lineweight);
    }

    fn revert(&mut self, doc: &mut Document) {
        if let Some((color, linetype, lineweight)) = self.previous.take() {
            doc.set_new_object_style(color, linetype, lineweight);
        }
    }

    fn changes(&self) -> Changes {
        Changes::layers()
    }
}

#[cfg(test)]
mod tests {
    use od_core::{Attributes, Color, Entity, Point};

    use super::*;
    use crate::{AddEntity, ModifyAttributes};

    fn circle() -> Entity {
        Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: 1.0,
        }
    }

    fn with_layer(name: &str) -> (Document, LayerId) {
        let mut doc = Document::default();
        let id = doc.allocate_layer_id();
        AddLayer::new(Layer::new(id, name)).apply(&mut doc);
        (doc, id)
    }

    #[test]
    fn add_layer_undoes_and_redoes() {
        let mut doc = Document::default();
        let id = doc.allocate_layer_id();
        let mut action = AddLayer::new(Layer::new(id, "Walls"));
        action.apply(&mut doc);
        assert_eq!(doc.layers().len(), 2);
        action.revert(&mut doc);
        assert_eq!(doc.layers().len(), 1);
        action.apply(&mut doc);
        assert_eq!(doc.layer(id).unwrap().name, "Walls");
    }

    #[test]
    fn modify_layer_swaps_and_reverts() {
        let (mut doc, id) = with_layer("Walls");
        let mut changed = doc.layer(id).unwrap().clone();
        changed.color = Color::Rgb(255, 0, 0);
        changed.locked = true;
        let mut action = ModifyLayers::new("Lock layer", vec![changed]);
        action.apply(&mut doc);
        assert!(doc.layer(id).unwrap().locked);
        action.revert(&mut doc);
        assert!(!doc.layer(id).unwrap().locked);
        assert_eq!(doc.layer(id).unwrap().color, Color::Foreground);
        action.apply(&mut doc);
        assert_eq!(doc.layer(id).unwrap().color, Color::Rgb(255, 0, 0));
    }

    #[test]
    fn remove_layer_restores_its_position() {
        let (mut doc, a) = with_layer("A");
        let b = doc.allocate_layer_id();
        doc.push_layer(Layer::new(b, "B"));
        let before = doc.clone();
        let mut action = RemoveLayer::new(a);
        action.apply(&mut doc);
        assert!(doc.layer(a).is_none());
        action.revert(&mut doc);
        assert_eq!(doc, before);
    }

    #[test]
    fn remove_layer_refuses_what_the_drawing_forbids() {
        let (mut doc, walls) = with_layer("Walls");
        let mut zero = RemoveLayer::new(LayerId::DEFAULT);
        zero.apply(&mut doc);
        assert!(doc.layer(LayerId::DEFAULT).is_some());

        SetCurrentLayer::new(walls).apply(&mut doc);
        let mut current = RemoveLayer::new(walls);
        current.apply(&mut doc);
        assert!(doc.layer(walls).is_some());
        current.revert(&mut doc); // nothing was removed, so nothing comes back
        assert_eq!(doc.layers().len(), 2);
    }

    #[test]
    fn new_objects_take_the_current_style_and_undo_restores_it() {
        let mut doc = Document::default();
        let red = Attr::Set(Color::Rgb(255, 0, 0));
        let mut action =
            SetNewObjectStyle::new(red.clone(), Attr::Set("DASHED".into()), Attr::ByLayer);
        action.apply(&mut doc);
        let id = doc.add(circle());
        let attrs = &doc.object(id).unwrap().attrs;
        assert_eq!(attrs.color, red);
        assert_eq!(attrs.linetype, Attr::Set("DASHED".to_owned()));
        assert_eq!(attrs.lineweight, Attr::<Lineweight>::ByLayer);
        action.revert(&mut doc);
        assert_eq!(
            doc.new_object_attributes(),
            Attributes::on_layer(LayerId::DEFAULT)
        );
    }

    #[test]
    fn set_current_layer_reverts_to_the_previous_one() {
        let (mut doc, walls) = with_layer("Walls");
        let mut action = SetCurrentLayer::new(walls);
        action.apply(&mut doc);
        assert_eq!(doc.current_layer(), walls);
        action.revert(&mut doc);
        assert_eq!(doc.current_layer(), LayerId::DEFAULT);
    }

    #[test]
    fn added_entities_keep_the_layer_they_were_drawn_on() {
        let (mut doc, walls) = with_layer("Walls");
        SetCurrentLayer::new(walls).apply(&mut doc);
        let mut add = AddEntity::new(circle());
        add.apply(&mut doc);
        // Redo after the current layer changed still uses the original layer.
        SetCurrentLayer::new(LayerId::DEFAULT).apply(&mut doc);
        add.revert(&mut doc);
        add.apply(&mut doc);
        assert_eq!(doc.objects()[0].attrs.layer, walls);
    }

    #[test]
    fn modify_attributes_moves_objects_between_layers_and_back() {
        let (mut doc, walls) = with_layer("Walls");
        let id = doc.add(circle());
        let mut action =
            ModifyAttributes::new("Change layer", vec![(id, Attributes::on_layer(walls))]);
        action.apply(&mut doc);
        assert_eq!(doc.object(id).unwrap().attrs.layer, walls);
        action.revert(&mut doc);
        assert_eq!(doc.object(id).unwrap().attrs.layer, LayerId::DEFAULT);
    }
}
