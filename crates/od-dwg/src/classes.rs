//! `AcDb:Classes`: the classes that give non-fixed object types their meaning
//! (Open Design Specification chapter 10).

use std::collections::BTreeMap;

use crate::bit::BitReader;
use crate::cursor::Cursor;
use crate::error::{DwgError, DwgErrorKind, DwgStage};
use crate::framed::{frame, CLASSES_END, CLASSES_START};
use crate::limits::{check, DwgLimits};
use crate::object::split_streams;
use crate::types::FIRST_CLASS_CODE;
use crate::version::DwgVersion;

/// `itemclassid` of a class that produces entities; objects use 0x1F3.
const ENTITY_CLASS: u16 = 0x1F2;
const OBJECT_CLASS: u16 = 0x1F3;

/// One class definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassRecord {
    /// The object type code objects of this class carry (500 or more).
    pub number: u16,
    pub proxy_flags: u16,
    pub app_name: String,
    pub cpp_name: String,
    pub dxf_name: String,
    pub was_zombie: bool,
    /// Whether the class produces entities rather than other objects.
    pub is_entity: bool,
    pub instance_count: u32,
}

/// The class table, looked up by object type code.
#[derive(Clone, Debug, Default)]
pub struct ClassRegistry {
    classes: Vec<ClassRecord>,
    by_number: BTreeMap<u16, usize>,
}

impl ClassRegistry {
    pub fn classes(&self) -> &[ClassRecord] {
        &self.classes
    }

    /// The class for an object type code of 500 or more.
    pub fn get(&self, type_code: u32) -> Option<&ClassRecord> {
        let number = u16::try_from(type_code).ok()?;
        self.by_number
            .get(&number)
            .and_then(|&i| self.classes.get(i))
    }
}

fn read_class(
    data: &mut BitReader<'_>,
    strings: &mut BitReader<'_>,
    limits: &DwgLimits,
) -> Result<ClassRecord, DwgError> {
    let number = data.bs()?;
    let proxy_flags = data.bs()?;
    let text_limit = limits.max_string_code_units;
    let app_name = strings.text(text_limit)?;
    let cpp_name = strings.text(text_limit)?;
    let dxf_name = strings.text(text_limit)?;
    let was_zombie = data.bit()?;
    let item_class = data.bs()?;
    let instance_count = data.bl()?;
    // Version, maintenance version and two longs that are normally zero.
    data.bs()?;
    data.bs()?;
    data.bl()?;
    data.bl()?;
    if item_class != ENTITY_CLASS && item_class != OBJECT_CLASS {
        return Err(data.error(DwgErrorKind::InvalidObject));
    }
    Ok(ClassRecord {
        number,
        proxy_flags,
        app_name,
        cpp_name,
        dxf_name,
        was_zombie,
        is_entity: item_class == ENTITY_CLASS,
        instance_count,
    })
}

/// Decodes the decompressed `AcDb:Classes` section.
///
/// The region starts with a 32-bit count of the bits it really uses, which
/// includes that count itself; any bits after it are padding to a byte. That
/// leaves the data stream, the string stream and the string-stream length and
/// flag in the same arrangement as an object, with the padding in the place of
/// the handle stream.
pub(crate) fn parse_classes(
    data: &[u8],
    version: DwgVersion,
    limits: &DwgLimits,
) -> Result<ClassRegistry, DwgError> {
    let stage = DwgStage::Classes;
    let sentinels = (&CLASSES_START, &CLASSES_END);
    let region = frame(data, version, sentinels, stage, "AcDb:Classes")?;
    let invalid = || DwgError::new(DwgErrorKind::InvalidObject, stage);
    let used = u64::from(Cursor::new(region, stage).u32()?);
    let padding = (region.len() as u64 * 8)
        .checked_sub(used)
        .filter(|&p| p < 8);
    let mut streams = split_streams(region, padding.ok_or_else(invalid)?, stage)?;
    streams.data.skip(32)?;
    let max_number = streams.data.bs()?;
    streams.data.skip(16 + 1)?; // two zero bytes and a true bit
    let mut registry = ClassRegistry::default();
    while streams.data.remaining() >= 8 {
        let count = registry.classes.len() as u64 + 1;
        check(stage, "class count", count, limits.max_collection_items)?;
        let class = read_class(&mut streams.data, &mut streams.strings, limits)?;
        let fresh = registry
            .by_number
            .insert(class.number, registry.classes.len())
            .is_none();
        if !fresh || u32::from(class.number) < FIRST_CLASS_CODE || class.number > max_number {
            return Err(invalid());
        }
        registry.classes.push(class);
    }
    if streams.strings.remaining() != 0 {
        return Err(invalid());
    }
    Ok(registry)
}
