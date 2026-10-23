//! The fields every object and entity starts with (Open Design Specification
//! §20.1, §20.2, §20.4.1 and §20.4.2). The "no links" bit the specification
//! lists after the data-store flag is not in the supplied drawings (R2010 and
//! later), whose fields then fill the data stream exactly.
//! Reading them is a check as well as a
//! step: an object's own handle must match the one the object map gave it.

use crate::bit::BitReader;
use crate::error::{DwgError, DwgErrorKind};
use crate::handles::DwgHandle;
use crate::limits::DwgLimits;
use crate::version::DwgVersion;

/// An entity's colour as stored (ENC, §2.11).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EntityColor {
    /// The colour index: 0 ByBlock, 1 to 255, 256 ByLayer, 257 ByBlock.
    pub index: u16,
    /// A true colour, as 0x00RRGGBB.
    pub rgb: Option<u32>,
    /// An `AcDbColor` (colour book) handle follows in the handle stream.
    pub has_book_handle: bool,
    pub transparency: Option<u32>,
}

/// What only entities carry, between the data and the entity's own fields.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EntityData {
    /// 0: the owner handle is stored; 1: paper space; 2: model space.
    pub entity_mode: u8,
    pub color: EntityColor,
    pub linetype_scale: f64,
    /// 0 ByLayer, 1 ByBlock, 2 Continuous, 3 a handle follows.
    pub linetype_flags: u8,
    pub plotstyle_flags: u8,
    pub material_flags: u8,
    pub shadow_flags: u8,
    /// Full, face and edge visual style handles follow.
    pub visual_styles: [bool; 3],
    /// Bit 0 set: the entity is invisible.
    pub invisible: u16,
    /// A lineweight code: 0 to 23 select standard values, 29 ByBlock, 30 ByLayer.
    pub lineweight: u8,
}

/// The common fields read from the data stream.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CommonData {
    pub handle: DwgHandle,
    /// The application handle of each block of extended entity data.
    pub eed_apps: Vec<DwgHandle>,
    pub graphic_bytes: u64,
    pub reactor_count: u32,
    pub xdictionary_missing: bool,
    pub has_data_store: bool,
    pub entity: Option<EntityData>,
}

/// The common handles read from the handle stream.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CommonHandles {
    pub owner: Option<DwgHandle>,
    pub reactors: Vec<DwgHandle>,
    pub xdictionary: Option<DwgHandle>,
    pub layer: Option<DwgHandle>,
    pub linetype: Option<DwgHandle>,
}

/// A colour as the tables store it (CMC, §2.11, R2004+): an index (zero in
/// the supplied drawings), a packed value whose top byte says what it is
/// (0xC2 true colour, 0xC3 an index in the low byte, 0xC0 ByLayer, 0xC1
/// ByBlock), and optional names.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TableColor {
    pub index: u16,
    pub packed: u32,
    pub name: Option<String>,
    pub book_name: Option<String>,
}

impl TableColor {
    /// The AutoCAD colour index, wherever this colour keeps it.
    pub fn aci(&self) -> Option<u8> {
        if self.packed >> 24 == 0xC3 {
            Some(self.packed as u8)
        } else {
            u8::try_from((self.index as i16).unsigned_abs())
                .ok()
                .filter(|&i| i != 0)
        }
    }

    /// The true colour as 0x00RRGGBB, when the colour is one.
    pub fn true_color(&self) -> Option<u32> {
        (self.packed >> 24 == 0xC2).then_some(self.packed & 0x00FF_FFFF)
    }
}

pub(crate) fn read_table_color(
    data: &mut BitReader<'_>,
    strings: &mut BitReader<'_>,
    limits: &DwgLimits,
) -> Result<TableColor, DwgError> {
    let index = data.bs()?;
    let packed = data.bl()?;
    let flags = data.rc()?;
    let max = limits.max_string_code_units;
    let name = if flags & 1 != 0 {
        Some(strings.text(max)?)
    } else {
        None
    };
    let book_name = if flags & 2 != 0 {
        Some(strings.text(max)?)
    } else {
        None
    };
    Ok(TableColor {
        index,
        packed,
        name,
        book_name,
    })
}

fn read_color(data: &mut BitReader<'_>) -> Result<EntityColor, DwgError> {
    let value = data.bs()?;
    let rgb = if value & 0x8000 != 0 {
        Some(data.bl()? & 0x00FF_FFFF)
    } else {
        None
    };
    let transparency = if value & 0x2000 != 0 {
        Some(data.bl()?)
    } else {
        None
    };
    Ok(EntityColor {
        index: value & 0x1FFF,
        rgb,
        has_book_handle: value & 0x4000 != 0,
        transparency,
    })
}

fn read_entity_data(
    data: &mut BitReader<'_>,
    version: DwgVersion,
    entity_mode: u8,
) -> Result<EntityData, DwgError> {
    let color = read_color(data)?;
    let linetype_scale = data.bd()?;
    let linetype_flags = data.bb()?;
    let plotstyle_flags = data.bb()?;
    let material_flags = data.bb()?;
    let shadow_flags = data.rc()?;
    let visual_styles = if version >= DwgVersion::R2010 {
        [data.bit()?, data.bit()?, data.bit()?]
    } else {
        [false; 3]
    };
    let invisible = data.bs()?;
    let lineweight = data.rc()?;
    Ok(EntityData {
        entity_mode,
        color,
        linetype_scale,
        linetype_flags,
        plotstyle_flags,
        material_flags,
        shadow_flags,
        visual_styles,
        invisible,
        lineweight,
    })
}

/// Reads the common fields at the start of an object's data stream, after its
/// type. `is_entity` selects the extra entity fields.
pub(crate) fn read_common_data(
    data: &mut BitReader<'_>,
    is_entity: bool,
    version: DwgVersion,
    limits: &DwgLimits,
) -> Result<CommonData, DwgError> {
    let handle = DwgHandle(data.handle(limits.max_handle_bytes)?.value);
    let mut eed_apps = Vec::new();
    loop {
        let size = u64::from(data.bs()?);
        if size == 0 {
            break;
        }
        eed_apps.push(DwgHandle(data.handle(limits.max_handle_bytes)?.value));
        data.skip(size * 8)?;
    }
    let mut graphic_bytes = 0;
    if is_entity && data.bit()? {
        graphic_bytes = data.bll()?;
        let bits = graphic_bytes.checked_mul(8);
        data.skip(bits.ok_or_else(|| data.error(DwgErrorKind::InvalidObject))?)?;
    }
    let entity_mode = if is_entity { data.bb()? } else { 0 };
    let reactor_count = data.bl()?;
    let xdictionary_missing = data.bit()?;
    let has_data_store = version.has_r2013_object_binary_flag() && data.bit()?;
    let entity = if is_entity {
        Some(read_entity_data(data, version, entity_mode)?)
    } else {
        None
    };
    Ok(CommonData {
        handle,
        eed_apps,
        graphic_bytes,
        reactor_count,
        xdictionary_missing,
        has_data_store,
        entity,
    })
}

/// Reads the common handles at the start of an object's handle stream.
pub(crate) fn read_common_handles(
    handles: &mut BitReader<'_>,
    common: &CommonData,
    limits: &DwgLimits,
) -> Result<CommonHandles, DwgError> {
    let base = common.handle;
    let next = |handles: &mut BitReader<'_>| -> Result<Option<DwgHandle>, DwgError> {
        let reference = handles.handle(limits.max_handle_bytes)?;
        reference
            .resolve(base)
            .map_err(|e| e.at(handles.position()))
    };
    let mut out = CommonHandles::default();
    if common.entity.is_none_or(|e| e.entity_mode == 0) {
        out.owner = next(handles)?;
    }
    for _ in 0..common.reactor_count {
        if let Some(reactor) = next(handles)? {
            out.reactors.push(reactor);
        }
    }
    if !common.xdictionary_missing {
        out.xdictionary = next(handles)?;
    }
    let Some(entity) = common.entity else {
        return Ok(out);
    };
    if entity.color.has_book_handle {
        next(handles)?;
    }
    out.layer = next(handles)?;
    if entity.linetype_flags == 3 {
        out.linetype = next(handles)?;
    }
    if entity.material_flags == 3 {
        next(handles)?;
    }
    if entity.plotstyle_flags == 3 {
        next(handles)?;
    }
    for present in entity.visual_styles {
        if present {
            next(handles)?;
        }
    }
    Ok(out)
}
