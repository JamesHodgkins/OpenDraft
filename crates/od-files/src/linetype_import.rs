//! Bringing a file's linetype definitions into the drawing's table, for the
//! readers that have them (DXF, DWG). A definition in the file replaces our
//! stock one of the same name, keeping its id, so the file's own numbers are
//! what is drawn.
//!
//! Layers and objects refer to a record by id, but a file names linetypes.
//! `id_for` turns each name a layer or object uses into a record id as it is
//! met: the record of that name, or (when the definition comes later in the
//! file, or never) a placeholder record with the file's spelling, so no
//! reference is left pointing at nothing. A later definition replaces the
//! placeholder keeping its id; `finish` gives each placeholder still without
//! one the stock pattern of that name, if there is one, and reports it, or
//! leaves it drawn solid, and reports that.

use std::collections::{BTreeMap, BTreeSet};

use od_core::linetype::stock;
use od_core::{Document, Linetype, LinetypeId, StrokeElement};

/// The three records every drawing has; a file's copies of them change nothing.
const SPECIAL: [&str; 3] = ["CONTINUOUS", "BYLAYER", "BYBLOCK"];

/// What reading the definitions has to report.
#[derive(Default)]
pub(crate) struct Imported {
    /// Definitions with text or shapes in them, read as their dashes alone.
    complex: Vec<String>,
    /// Names used but not defined, given the stock definition.
    standard: BTreeSet<String>,
    /// Names used but not defined and not stock: drawn solid.
    undefined: BTreeSet<String>,
    /// Records made for a name that was used before (or without) a
    /// definition.
    placeholders: BTreeMap<LinetypeId, String>,
}

fn list(names: impl Iterator<Item = String>) -> String {
    names.collect::<Vec<_>>().join(", ")
}

impl Imported {
    /// Add or replace the record called `name`. False when the table refuses
    /// it (an unusable name or length), leaving the table as it was. The
    /// three special records are left alone and count as taken.
    pub(crate) fn define(
        &mut self,
        document: &mut Document,
        name: &str,
        description: &str,
        elements: Vec<StrokeElement>,
        complex: bool,
    ) -> bool {
        if SPECIAL.iter().any(|s| s.eq_ignore_ascii_case(name)) {
            return true;
        }
        let mut table = document.linetypes().clone();
        let done = match table.named(name).map(|r| r.id) {
            Some(id) => table
                .replace(Linetype {
                    id,
                    name: name.to_owned(),
                    description: description.to_owned(),
                    elements,
                    flags: 0,
                })
                .is_ok(),
            None => table.add(name, description, elements).is_ok(),
        };
        if done {
            if let Some(id) = table.named(name).map(|r| r.id) {
                self.placeholders.remove(&id);
            }
            document.set_linetypes(table);
            if complex {
                self.complex.push(name.to_owned());
            }
        }
        done
    }

    /// The record a layer or object means by the linetype `name`: the
    /// table's record of that name (ignoring case), or a placeholder made now
    /// (see the module notes). A name the table refuses (empty or too long)
    /// is drawn solid with `CONTINUOUS`.
    pub(crate) fn id_for(&mut self, document: &mut Document, name: &str) -> LinetypeId {
        if let Some(record) = document.linetypes().named(name) {
            return record.id;
        }
        let mut table = document.linetypes().clone();
        match table.add(name, "", Vec::new()) {
            Ok(id) => {
                document.set_linetypes(table);
                self.placeholders.insert(id, name.to_owned());
                id
            }
            Err(_) => {
                self.undefined.insert(name.to_owned());
                LinetypeId::CONTINUOUS
            }
        }
    }

    /// After everything is read: each placeholder still without a definition
    /// gets the stock pattern of its name, if there is one.
    pub(crate) fn finish(&mut self, document: &mut Document) {
        let mut table = document.linetypes().clone();
        for (id, name) in std::mem::take(&mut self.placeholders) {
            let stocked = stock::find(&name).and_then(|s| {
                let elements = s
                    .pattern
                    .iter()
                    .filter_map(|v| StrokeElement::from_dxf(*v))
                    .collect();
                table
                    .replace(Linetype {
                        id,
                        name: name.clone(),
                        description: s.description.to_owned(),
                        elements,
                        flags: 0,
                    })
                    .ok()
            });
            if stocked.is_some() {
                self.standard.insert(name);
            } else {
                self.undefined.insert(name);
            }
        }
        document.set_linetypes(table);
    }

    pub(crate) fn warnings(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !self.complex.is_empty() {
            out.push(format!(
                "Linetypes with text or shapes were read as plain dashes: {}.",
                list(self.complex.iter().cloned())
            ));
        }
        if !self.standard.is_empty() {
            out.push(format!(
                "Linetypes the file does not define were given the standard pattern: {}.",
                list(self.standard.iter().cloned())
            ));
        }
        if !self.undefined.is_empty() {
            out.push(format!(
                "Linetypes the file does not define are drawn solid: {}.",
                list(self.undefined.iter().cloned())
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_used_before_its_definition_keeps_one_record() {
        let mut doc = Document::default();
        let mut imported = Imported::default();
        let id = imported.id_for(&mut doc, "Later");
        assert_eq!(imported.id_for(&mut doc, "LATER"), id, "ignoring case");
        let pattern = vec![StrokeElement::Dash(2.0), StrokeElement::Gap(1.0)];
        assert!(imported.define(&mut doc, "later", "defined after", pattern.clone(), false));
        imported.finish(&mut doc);
        let record = doc.linetypes().get(id).unwrap();
        assert_eq!(record.elements, pattern);
        assert_eq!(record.description, "defined after");
        assert_eq!(
            doc.linetypes()
                .records()
                .iter()
                .filter(|r| r.name.eq_ignore_ascii_case("later"))
                .count(),
            1
        );
        assert!(imported.warnings().is_empty());
    }

    #[test]
    fn a_name_never_defined_gets_the_stock_pattern_or_stays_solid_and_is_reported() {
        let mut doc = Document::default();
        let mut imported = Imported::default();
        let stock_id = imported.id_for(&mut doc, "HIDDEN2");
        let solid_id = imported.id_for(&mut doc, "Mystery");
        imported.finish(&mut doc);
        assert!(!doc.linetypes().get(stock_id).unwrap().elements.is_empty());
        assert!(doc.linetypes().get(solid_id).unwrap().elements.is_empty());
        let text = imported.warnings().join(
            "
",
        );
        assert!(text.contains("standard pattern: HIDDEN2"), "{text}");
        assert!(text.contains("drawn solid: Mystery"), "{text}");
    }

    #[test]
    fn a_name_the_table_refuses_is_drawn_continuous() {
        let mut doc = Document::default();
        let mut imported = Imported::default();
        assert_eq!(imported.id_for(&mut doc, ""), LinetypeId::CONTINUOUS);
    }

    #[test]
    fn the_names_of_the_stock_records_find_them_whatever_the_case() {
        let mut doc = Document::default();
        let mut imported = Imported::default();
        assert_eq!(imported.id_for(&mut doc, "dashed"), LinetypeId::DASHED);
        assert_eq!(
            imported.id_for(&mut doc, "Continuous"),
            LinetypeId::CONTINUOUS
        );
        assert!(doc.linetypes().is_stock(), "nothing was added");
    }
}
