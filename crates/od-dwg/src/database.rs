//! The loaded object database: header, classes, object map and the objects
//! section, and a checked pass over every object in handle order.

use std::sync::atomic::AtomicBool;

use crate::classes::{parse_classes, ClassRegistry};
use crate::common::{read_common_data, read_common_handles, CommonData, CommonHandles};
use crate::container::{check_cancelled, section_names, Container};
use crate::error::{DwgError, DwgErrorKind, DwgStage};
use crate::handles::{parse_object_map, DwgHandle, MapEntry};
use crate::header::{parse_header, DwgHeader};
use crate::limits::DwgLimits;
use crate::object::{read_object, ObjectStreams};
use crate::types::{classify, ObjectKind};
use crate::version::DwgVersion;

pub(crate) struct Database {
    pub version: DwgVersion,
    pub header: DwgHeader,
    pub classes: ClassRegistry,
    pub map: Vec<MapEntry>,
    objects: Vec<u8>,
}

/// One object, handed to the callback of [`Database::visit`] with its common
/// fields already read; `streams` is positioned at the object's own fields.
pub(crate) struct Visit<'a, 'b> {
    pub handle: DwgHandle,
    pub offset: u64,
    pub extent: u64,
    pub type_code: u32,
    pub kind: ObjectKind,
    pub is_entity: bool,
    /// `None` for a type this reader has no name for, whose contents are not read.
    pub common: Option<CommonData>,
    pub refs: Option<CommonHandles>,
    pub streams: &'b mut ObjectStreams<'a>,
    /// The file's version, for fields that depend on it.
    pub version: DwgVersion,
}

impl Database {
    pub fn load(
        bytes: &[u8],
        limits: &DwgLimits,
        cancelled: &AtomicBool,
    ) -> Result<Database, DwgError> {
        let mut container = Container::open(bytes, limits, cancelled)?;
        let version = container.version();
        let header_data = container.read_section(section_names::HEADER, cancelled)?;
        let header = parse_header(&header_data, version, limits)?;
        let classes_data = container.read_section(section_names::CLASSES, cancelled)?;
        let classes = parse_classes(&classes_data, version, limits)?;
        let handles_data = container.read_section(section_names::HANDLES, cancelled)?;
        let objects = container.read_section(section_names::OBJECTS, cancelled)?;
        let map = parse_object_map(&handles_data, objects.len() as u64, limits, cancelled)?;
        check_cancelled(cancelled, DwgStage::Object)?;
        Ok(Database {
            version,
            header,
            classes,
            map,
            objects,
        })
    }

    /// Calls `f` for every object in handle order, after checking its
    /// envelope, CRC, type and common fields (its own handle must be the one
    /// the map gave it), and at the end that no two objects share bytes. An
    /// error from `f` is reported against the object.
    pub fn visit<F>(
        &self,
        limits: &DwgLimits,
        cancelled: &AtomicBool,
        mut f: F,
    ) -> Result<(), DwgError>
    where
        F: FnMut(&mut Visit<'_, '_>) -> Result<(), DwgError>,
    {
        let mut spans = Vec::with_capacity(self.map.len());
        for (n, entry) in self.map.iter().enumerate() {
            if n % 1024 == 0 {
                check_cancelled(cancelled, DwgStage::Object)?;
            }
            let of = |e: DwgError| e.of_object(entry.handle);
            let bad = |kind| {
                DwgError::new(kind, DwgStage::Object)
                    .at(entry.offset)
                    .of_object(entry.handle)
            };
            let mut raw = read_object(&self.objects, entry.offset, limits).map_err(of)?;
            let (kind, is_entity) = classify(raw.type_code, &self.classes).map_err(bad)?;
            spans.push((entry.offset, raw.extent));
            let (mut common, mut refs) = (None, None);
            if !matches!(kind, ObjectKind::Unknown(_)) {
                let streams = &mut raw.streams;
                let c = read_common_data(&mut streams.data, is_entity, self.version, limits)
                    .map_err(of)?;
                if c.handle != entry.handle {
                    return Err(bad(DwgErrorKind::InvalidHandle));
                }
                refs = Some(read_common_handles(&mut streams.handles, &c, limits).map_err(of)?);
                common = Some(c);
            }
            let mut visit = Visit {
                handle: entry.handle,
                offset: entry.offset,
                extent: raw.extent,
                type_code: raw.type_code,
                kind,
                is_entity,
                common,
                refs,
                streams: &mut raw.streams,
                version: self.version,
            };
            f(&mut visit).map_err(of)?;
        }
        no_overlaps(&mut spans)
    }
}

/// Objects must not share bytes: each starts after the previous one ends.
fn no_overlaps(spans: &mut [(u64, u64)]) -> Result<(), DwgError> {
    spans.sort_unstable();
    for pair in spans.windows(2) {
        if let [(offset, extent), (next, _)] = pair {
            if offset.saturating_add(*extent) > *next {
                let err = DwgError::new(DwgErrorKind::InvalidObject, DwgStage::Object);
                return Err(err.at(*next));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objects_that_share_bytes_are_refused() {
        assert!(no_overlaps(&mut [(0, 10), (10, 5), (20, 4)]).is_ok());
        assert!(no_overlaps(&mut [(0, 11), (10, 5)]).is_err());
        assert!(no_overlaps(&mut [(4, 5), (4, 5)]).is_err());
        assert!(
            no_overlaps(&mut [(30, 5), (0, 31)]).is_err(),
            "order does not matter"
        );
    }
}
