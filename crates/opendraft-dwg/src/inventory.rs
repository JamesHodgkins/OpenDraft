//! A checked list of every object in a drawing: what it is, where it is, and
//! whom it belongs to. It is the raw inventory the semantic importer is
//! built on, and a way to check the reader against a drawing's known content.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;

use crate::classes::ClassRegistry;
use crate::database::Database;
use crate::error::DwgError;
use crate::handles::DwgHandle;
use crate::header::DwgHeader;
use crate::limits::DwgLimits;
use crate::types::ObjectKind;
use crate::version::DwgVersion;

/// One object of the drawing.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectSummary {
    pub handle: DwgHandle,
    pub kind: ObjectKind,
    pub type_code: u32,
    pub is_entity: bool,
    /// Where the object starts in `AcDb:AcDbObjects`, and how many bytes it
    /// takes up including its size fields and CRC.
    pub offset: u64,
    pub extent: u64,
    pub owner: Option<DwgHandle>,
    /// The layer, for an entity.
    pub layer: Option<DwgHandle>,
    pub reactors: Vec<DwgHandle>,
    pub extended_data_blocks: u32,
    /// An entity marked invisible.
    pub invisible: bool,
}

/// Every object of a drawing, in handle order.
#[derive(Debug)]
pub struct Inventory {
    pub version: DwgVersion,
    pub header: DwgHeader,
    pub classes: ClassRegistry,
    pub objects: Vec<ObjectSummary>,
    /// References (owner, layer, reactors) to handles that are not in the
    /// drawing. Zero in a healthy file.
    pub dangling_references: u64,
}

impl Inventory {
    /// Reads and checks the whole object database of `bytes`.
    pub fn read(
        bytes: &[u8],
        limits: &DwgLimits,
        cancelled: &AtomicBool,
    ) -> Result<Inventory, DwgError> {
        let db = Database::load(bytes, limits, cancelled)?;
        let known: BTreeSet<DwgHandle> = db.map.iter().map(|e| e.handle).collect();
        let mut objects = Vec::with_capacity(db.map.len());
        let mut dangling = 0u64;
        db.visit(limits, cancelled, |v| {
            let mut summary = ObjectSummary {
                handle: v.handle,
                kind: v.kind.clone(),
                type_code: v.type_code,
                is_entity: v.is_entity,
                offset: v.offset,
                extent: v.extent,
                owner: None,
                layer: None,
                reactors: Vec::new(),
                extended_data_blocks: 0,
                invisible: false,
            };
            if let (Some(common), Some(refs)) = (&v.common, &v.refs) {
                summary.owner = refs.owner;
                summary.layer = refs.layer;
                summary.reactors = refs.reactors.clone();
                summary.extended_data_blocks = common.eed_apps.len() as u32;
                summary.invisible = common.entity.is_some_and(|e| e.invisible & 1 != 0);
                let all = [refs.owner, refs.layer, refs.xdictionary]
                    .into_iter()
                    .flatten();
                let all = all.chain(refs.reactors.iter().copied());
                dangling += all.filter(|h| !known.contains(h)).count() as u64;
            }
            objects.push(summary);
            Ok(())
        })?;
        Ok(Inventory {
            version: db.version,
            header: db.header,
            classes: db.classes,
            objects,
            dangling_references: dangling,
        })
    }

    /// How many objects there are of each type, by DXF name.
    pub fn type_counts(&self) -> BTreeMap<String, u64> {
        let mut counts = BTreeMap::new();
        for object in &self.objects {
            *counts.entry(object.kind.name()).or_insert(0) += 1;
        }
        counts
    }

    pub fn get(&self, handle: DwgHandle) -> Option<&ObjectSummary> {
        let i = self
            .objects
            .binary_search_by_key(&handle, |o| o.handle)
            .ok()?;
        self.objects.get(i)
    }
}
