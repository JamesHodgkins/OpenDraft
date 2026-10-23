//! Undoable pasting of another drawing's objects (see `Document::absorb`).

use od_core::{Absorbed, Document, LayoutId, Placement};

use crate::{Action, Changes};

/// Bring the objects of a clipboard drawing in, with the layers, styles,
/// linetypes and blocks they need that the drawing lacks.
pub struct PasteDrawing {
    name: String,
    source: Document,
    placement: Placement,
    /// Decided the first time the action runs: the space being edited then.
    layout: Option<Option<LayoutId>>,
    done: Option<Absorbed>,
}

impl PasteDrawing {
    pub fn new(name: impl Into<String>, source: Document, placement: Placement) -> Self {
        Self {
            name: name.into(),
            source,
            placement,
            layout: None,
            done: None,
        }
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
                self.done = Some(doc.absorb(&self.source, &self.placement, layout));
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
