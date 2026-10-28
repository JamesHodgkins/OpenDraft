//! How the linetype table is written to the file. A pattern is DXF's own
//! list of lengths (group 49): above zero a dash, below zero a gap, zero a
//! dot.

use od_core::{Attributes, Document, Linetype, LinetypeId, LinetypeTable, StrokeElement};
use serde::{Deserialize, Serialize};

use super::entity::invalid;
use crate::FileError;

#[derive(Serialize, Deserialize)]
pub(super) struct LinetypeDto {
    id: u64,
    name: String,
    description: String,
    pattern: Vec<f64>,
    /// DXF symbol-table flags, kept as read.
    flags: u16,
}

/// How an object's pattern is laid, as one flattened group of optional fields:
/// a scale of 1 and a generation of false are left out, and files from before
/// linetype scales read as those.
#[derive(Serialize, Deserialize)]
#[serde(default)]
pub(super) struct ScaleDto {
    #[serde(skip_serializing_if = "is_one")]
    linetype_scale: f64,
    #[serde(skip_serializing_if = "is_false")]
    continuous_generation: bool,
}

impl Default for ScaleDto {
    fn default() -> Self {
        Self {
            linetype_scale: 1.0,
            continuous_generation: false,
        }
    }
}

impl ScaleDto {
    pub(super) fn of(attrs: &Attributes) -> Self {
        Self {
            linetype_scale: attrs.linetype_scale,
            continuous_generation: attrs.continuous_generation,
        }
    }

    /// Set the two fields on `attrs`; a scale that is not above zero is 1.
    pub(super) fn apply(&self, attrs: &mut Attributes) {
        let usable = self.linetype_scale.is_finite() && self.linetype_scale > 0.0;
        attrs.linetype_scale = if usable { self.linetype_scale } else { 1.0 };
        attrs.continuous_generation = self.continuous_generation;
    }
}

fn is_one(v: &f64) -> bool {
    *v == 1.0
}

fn is_false(v: &bool) -> bool {
    !*v
}

impl LinetypeDto {
    fn from_record(record: &Linetype) -> Self {
        Self {
            id: record.id.0,
            name: record.name.clone(),
            description: record.description.clone(),
            pattern: record.elements.iter().map(|e| e.to_dxf()).collect(),
            flags: record.flags,
        }
    }

    fn to_record(&self) -> Result<Linetype, FileError> {
        let mut elements = Vec::with_capacity(self.pattern.len());
        for value in &self.pattern {
            match StrokeElement::from_dxf(*value) {
                Some(e) => elements.push(e),
                None => {
                    return invalid(format!(
                        "linetype \"{}\" has a length that is not a number",
                        self.name
                    ))
                }
            }
        }
        Ok(Linetype {
            id: LinetypeId(self.id),
            name: self.name.clone(),
            description: self.description.clone(),
            elements,
            flags: self.flags,
        })
    }
}

/// The drawing's linetypes as saved, flattened into the drawing: every
/// record in full (the file does not depend on the stock patterns of the
/// build that reads it), their id counter and `LTSCALE`.
#[derive(Serialize, Deserialize)]
pub(super) struct TableDto {
    linetypes: Vec<LinetypeDto>,
    next_linetype_id: u64,
    linetype_scale: f64,
}

impl TableDto {
    pub(super) fn of(document: &Document) -> Self {
        let table = document.linetypes();
        Self {
            linetypes: table
                .records()
                .iter()
                .map(LinetypeDto::from_record)
                .collect(),
            next_linetype_id: table.next_id(),
            linetype_scale: document.linetype_scale(),
        }
    }

    /// Load the table into `document`.
    pub(super) fn load(&self, document: &mut Document) -> Result<(), FileError> {
        document.set_linetype_scale(self.linetype_scale);
        let records = self
            .linetypes
            .iter()
            .map(LinetypeDto::to_record)
            .collect::<Result<Vec<_>, _>>()?;
        match LinetypeTable::from_records(records, self.next_linetype_id) {
            Ok(table) => {
                document.set_linetypes(table);
                Ok(())
            }
            Err(e) => invalid(e.to_string()),
        }
    }
}
