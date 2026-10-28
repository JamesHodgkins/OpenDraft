//! The LTYPE table: the dash patterns a drawing defines, read into the
//! drawing's linetype table (see `linetype_import`).

use od_core::StrokeElement;

use super::{decode, Importer, Record};

/// What the table reading has to report.
pub(super) type Reading = crate::linetype_import::Imported;

impl Importer {
    /// Read one LTYPE record. The three special records are ours already.
    pub(super) fn linetype_record(&mut self, record: &Record) {
        let name = decode(record.text(2).unwrap_or(""));
        let mut elements = Vec::new();
        let mut complex = false;
        for group in &record.groups {
            match group.code {
                49 => match group.value.parse().ok().and_then(StrokeElement::from_dxf) {
                    Some(element) => elements.push(element),
                    None => {
                        self.damaged += 1;
                        return;
                    }
                },
                74 => complex |= group.value.parse::<i64>().is_ok_and(|v| v != 0),
                _ => {}
            }
        }
        let description = decode(record.text(3).unwrap_or(""));
        let done = self
            .ltypes
            .define(&mut self.document, &name, &description, elements, complex);
        self.damaged += usize::from(!done);
    }

    /// After everything is read: give each linetype name in use a record.
    pub(super) fn resolve_linetypes(&mut self) {
        self.ltypes.finish(&mut self.document);
    }
}
