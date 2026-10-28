//! The linetype table as the drawing holds it.

use super::Document;
use crate::linetype::LinetypeTable;

impl Document {
    pub fn linetypes(&self) -> &LinetypeTable {
        &self.linetypes
    }

    /// The drawing's `LTSCALE`.
    pub fn linetype_scale(&self) -> f64 {
        self.linetype_scale
    }

    /// Set `LTSCALE` and return the old value. A scale that is not a finite
    /// number above zero is ignored.
    pub fn set_linetype_scale(&mut self, scale: f64) -> f64 {
        let old = self.linetype_scale;
        if scale.is_finite() && scale > 0.0 {
            self.linetype_scale = scale;
        }
        old
    }

    /// Whether a layer, an object or a block's contents use the linetype
    /// called `name` (ignoring case). A linetype in use is not for
    /// deleting.
    pub fn linetype_in_use(&self, name: &str) -> bool {
        let same = |used: &str| used.eq_ignore_ascii_case(name);
        let set = |attr: &crate::Attr<String>| match attr {
            crate::Attr::Set(used) => same(used),
            crate::Attr::ByLayer => false,
        };
        self.layers().iter().any(|l| same(&l.linetype))
            || self.objects().iter().any(|o| set(&o.attrs.linetype))
            || self
                .blocks()
                .iter()
                .any(|b| b.entities.iter().any(|e| set(&e.attrs.linetype)))
    }

    /// Swap in a whole table, returning the old one (what undo and loading
    /// use). The caller has built it through [`LinetypeTable`], which keeps
    /// names and ids unique.
    pub fn set_linetypes(&mut self, table: LinetypeTable) -> LinetypeTable {
        std::mem::replace(&mut self.linetypes, table)
    }
}
