//! Undoable pasting of another drawing's objects (see `Document::absorb`).

use std::sync::{Arc, Mutex};

use od_core::{Absorbed, Document, LayoutId, Placement, TransferWarning};

use crate::{Action, Changes};

/// Where a paste leaves what did not carry over exactly, for the caller to
/// say (the action itself is gone into the undo stack by then).
pub type WarningSink = Arc<Mutex<Vec<TransferWarning>>>;

/// Bring the objects of a clipboard drawing in, with the layers, styles,
/// linetypes and blocks they need that the drawing lacks.
pub struct PasteDrawing {
    name: String,
    source: Document,
    placement: Placement,
    /// Decided the first time the action runs: the space being edited then.
    layout: Option<Option<LayoutId>>,
    done: Option<Absorbed>,
    sink: Option<WarningSink>,
}

impl PasteDrawing {
    pub fn new(name: impl Into<String>, source: Document, placement: Placement) -> Self {
        Self {
            name: name.into(),
            source,
            placement,
            layout: None,
            done: None,
            sink: None,
        }
    }

    /// Put the warnings of the first run in `sink`.
    pub fn reporting(mut self, sink: WarningSink) -> Self {
        self.sink = Some(sink);
        self
    }
}

impl Action for PasteDrawing {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn apply(&mut self, doc: &mut Document) {
        match &self.done {
            Some(done) => doc.reabsorb(done),
            None => {
                let layout = *self.layout.get_or_insert(doc.active_layout());
                let absorbed = doc.absorb(&self.source, &self.placement, layout);
                if let Some(Ok(mut sink)) = self.sink.as_ref().map(|s| s.lock()) {
                    sink.extend(absorbed.warnings.iter().cloned());
                }
                self.done = Some(absorbed);
            }
        }
    }

    fn revert(&mut self, doc: &mut Document) {
        if let Some(done) = &self.done {
            doc.unabsorb(done);
        }
    }

    fn changes(&self) -> Changes {
        Changes {
            added: self.done.as_ref().map(Absorbed::ids).unwrap_or_default(),
            layers: true,
            dim_style: true,
            blocks: true,
            linetypes: true,
            text_styles: true,
            mleader_styles: true,
            ..Changes::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use od_core::StrokeElement::{Dash, Gap};
    use od_core::{Attr, Attributes, Entity, LayerId, Point};

    use super::*;

    /// The first run reports a same-name linetype with another pattern once,
    /// and a redo does not report it again.
    #[test]
    fn a_conflicting_linetype_is_reported_to_the_sink_on_the_first_run_only() {
        let mut source = Document::default();
        let mut table = source.linetypes().clone();
        let mine = table.add("Mine", "", vec![Dash(5.0), Gap(5.0)]).unwrap();
        source.set_linetypes(table);
        let attrs = Attributes {
            linetype: Attr::Set(mine),
            ..Attributes::on_layer(LayerId::DEFAULT)
        };
        source.add_with(
            Entity::Circle {
                center: Point::new(0.0, 0.0),
                radius: 1.0,
            },
            attrs,
        );
        let mut doc = Document::default();
        let mut table = doc.linetypes().clone();
        table.add("MINE", "", vec![Dash(1.0), Gap(1.0)]).unwrap();
        doc.set_linetypes(table);

        let sink = WarningSink::default();
        let mut paste = PasteDrawing::new("Paste", source, Placement::Loose { dx: 0.0, dy: 0.0 })
            .reporting(sink.clone());
        paste.apply(&mut doc);
        paste.revert(&mut doc);
        paste.apply(&mut doc);
        assert_eq!(
            *sink.lock().unwrap(),
            [TransferWarning::LinetypeDiffers("Mine".to_owned())]
        );
        assert_eq!(doc.validate(), Ok(()));
    }
}
