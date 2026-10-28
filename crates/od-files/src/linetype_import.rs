//! Bringing a file's linetype definitions into the drawing's table, for the
//! readers that have them (DXF, DWG). A definition in the file replaces our
//! stock one of the same name, keeping its id, so the file's own numbers are
//! what is drawn. A name that layers or objects use but no definition covers
//! gets the stock definition of that name, if there is one, and is reported.

use std::collections::BTreeSet;

use od_core::linetype::stock;
use od_core::{Attr, Document, Linetype, StrokeElement};

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
            document.set_linetypes(table);
            if complex {
                self.complex.push(name.to_owned());
            }
        }
        done
    }

    /// After everything is read: give each linetype name in use a record.
    pub(crate) fn resolve_used(&mut self, document: &mut Document) {
        let mut used: BTreeSet<String> = document
            .layers()
            .iter()
            .map(|l| l.linetype.clone())
            .collect();
        let set = |attr: &Attr<String>| match attr {
            Attr::Set(name) => Some(name.clone()),
            Attr::ByLayer => None,
        };
        used.extend(
            document
                .objects()
                .iter()
                .filter_map(|o| set(&o.attrs.linetype)),
        );
        for block in document.blocks() {
            used.extend(block.entities.iter().filter_map(|e| set(&e.attrs.linetype)));
        }
        let mut table = document.linetypes().clone();
        for name in used {
            if table.named(&name).is_some() {
                continue;
            }
            if stock::find(&name).is_some() && table.load_stock(&name).is_ok() {
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
