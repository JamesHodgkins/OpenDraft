//! AutoCAD DXF: an exchange format for drawings, not one the app can keep a
//! drawing in without loss. Opening a DXF is an import (the result is a new
//! unsaved drawing) and saving one is an export; the notes and warnings
//! returned say what did not carry over.

mod blocks;
mod dimensions;
mod dimstyle_codec;
mod header;
#[cfg(test)]
mod linetype_tests;
mod linetypes;
mod point_style;
mod read;
mod units;
mod write;
mod write2000;

use od_core::Document;

use crate::{Encoded, FileError, FileFormat, Loaded};

pub struct DxfFormat;

/// The largest DXF read, as for DWG: the reader holds the whole file and
/// its groups in memory.
const MAX_FILE_BYTES: u64 = 1 << 30;

impl FileFormat for DxfFormat {
    fn name(&self) -> &'static str {
        "AutoCAD DXF"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["dxf"]
    }

    fn read(&self, bytes: &[u8]) -> Result<Loaded, FileError> {
        read::read(bytes)
    }

    fn check_size(&self, len: u64) -> Result<(), FileError> {
        if len > MAX_FILE_BYTES {
            return Err(FileError::Unsupported(
                "this DXF exceeds the safe file size limit".into(),
            ));
        }
        Ok(())
    }

    fn write(&self, document: &Document) -> Result<Encoded, FileError> {
        let (flat, block_count) = blocks::exploded(document);
        let (flat, count) = dimensions::exploded(&flat);
        let mut encoded = write::write(&flat)?;
        if block_count > 0 {
            encoded.notes.push(format!(
                "Block references were written as the objects they draw: DXF export of blocks is not built yet ({block_count} in this drawing)."
            ));
        }
        if count > 0 {
            encoded.notes.push(format!(
                "{count} arc length, jogged or jog line dimension{} written as plain lines, arcs and multiline text: they have no DIMENSION to map onto yet.",
                if count == 1 { " was" } else { "s were" }
            ));
        }
        let real = flat
            .objects()
            .iter()
            .filter(|o| matches!(&o.entity, od_core::Entity::Dimension(_)))
            .count();
        if real > 0 {
            encoded.notes.push(
                "Dimensions were written as DIMENSION entities with their pictures as anonymous blocks and their styles in the DIMSTYLE table; they have not been reopened in AutoCAD.".to_owned(),
            );
        }
        if flat
            .objects()
            .iter()
            .any(|o| matches!(o.entity, od_core::Entity::MLeader(_)))
        {
            encoded.notes.push(
                "Multileaders were written as DXF 2007 MULTILEADER objects following the layout of AutoCAD 2018; they have not been reopened in AutoCAD, arrowheads other than closed filled are written as closed filled, and override flags and preview graphics are not written.".to_owned(),
            );
        }
        Ok(encoded)
    }
}
