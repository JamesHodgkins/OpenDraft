//! Hatch sources in a DXF: each loop names the objects it was made from by
//! handle; once every object is read the handles become the objects.

use od_core::hatch::{follow, BoundarySource, Follow};
use od_core::Entity;

use super::Importer;

impl Importer {
    /// Turns each associative hatch's source handles into the objects they
    /// name. A hatch whose sources are missing, or that no longer matches
    /// them (AutoCAD files can hold a hatch that was copied away from its
    /// boundary), is kept as it is, without the association.
    pub(super) fn resolve_hatch_sources(&mut self) {
        let mut updates = Vec::new();
        for object in self.document.objects() {
            let Entity::Hatch(h) = &object.entity else {
                continue;
            };
            if !h.associative {
                continue;
            }
            let mut resolved = (**h).clone();
            let mut complete = true;
            for l in &mut resolved.loops {
                for s in &mut l.sources {
                    if let BoundarySource::External { value, .. } = s {
                        match self.handles.get(&value.to_uppercase()) {
                            Some(id) => *s = BoundarySource::Entity(*id),
                            None => complete = false,
                        }
                    }
                }
            }
            let doc = &self.document;
            let lookup = |id| doc.get(id).cloned();
            let kept = match (complete, follow(&resolved, &lookup, true)) {
                (true, Follow::Same) => resolved,
                _ => h.detached(),
            };
            updates.push((object.id, Entity::Hatch(Box::new(kept))));
        }
        for (id, entity) in updates {
            self.document.replace(id, entity);
        }
    }
}
