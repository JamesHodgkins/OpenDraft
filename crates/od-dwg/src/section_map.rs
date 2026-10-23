//! The two system sections that index the file: the page map (page number to
//! file position) and the section map (section name to its pages), Open Design
//! Specification §4.3 to §4.5.

use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;

use crate::checksum::page_checksum;
use crate::compression::decompress;
use crate::cursor::Cursor;
use crate::error::{DwgError, DwgErrorKind, DwgStage};
use crate::limits::{check, DwgLimits};

pub(crate) const PAGE_MAP_TYPE: u32 = 0x4163_0E3B;
pub(crate) const SECTION_MAP_TYPE: u32 = 0x4163_003B;
/// Where the first page starts; every address in the page map is relative to it.
pub(crate) const FIRST_PAGE: u64 = 0x100;

/// Where one page lies in the file.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PageEntry {
    pub size: u32,
    pub address: u64,
}

pub(crate) type PageMap = BTreeMap<u32, PageEntry>;

/// One page of a section, as listed in the section map.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PageRef {
    pub number: u32,
    pub data_size: u32,
    /// Where this page's bytes begin in the decompressed section.
    pub start: u64,
}

/// A named logical section, assembled from pages.
#[derive(Debug, Clone)]
pub struct SectionInfo {
    pub name: String,
    pub id: u32,
    /// Decompressed size in bytes.
    pub size: u64,
    pub compressed: bool,
    /// 0 = no, 1 = yes, 2 = not known.
    pub encrypted: u32,
    pub(crate) max_page_size: u32,
    pub(crate) pages: Vec<PageRef>,
}

impl SectionInfo {
    /// How many pages are stored. Omitted all-zero pages are not counted.
    pub fn page_count(&self) -> usize {
        self.pages.len()
    }
}

fn corrupt(stage: DwgStage, at: u64) -> DwgError {
    DwgError::new(DwgErrorKind::InvalidSectionMap, stage).at(at)
}

/// Reads and decompresses a system page: a 0x14-byte header, then the
/// compressed data. Checksummed in two stages: the header (with its own
/// checksum zeroed) seeds the checksum of the data.
pub(crate) fn read_system_page(
    bytes: &[u8],
    address: u64,
    expected_type: u32,
    stage: DwgStage,
    limits: &DwgLimits,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>, DwgError> {
    crate::check_cancelled(cancelled, stage)?;
    let start = usize::try_from(address).map_err(|_| corrupt(stage, address))?;
    let mut header_bytes: [u8; 0x14] = bytes
        .get(start..)
        .and_then(|rest| rest.get(..0x14))
        .and_then(|h| h.try_into().ok())
        .ok_or_else(|| DwgError::new(DwgErrorKind::Truncated, stage).at(address))?;
    let mut c = Cursor::new(&header_bytes, stage);
    let (page_type, decompressed, compressed) = (c.u32()?, c.u32()?, c.u32()?);
    let (compression, stored) = (c.u32()?, c.u32()?);
    if page_type != expected_type || compression != 2 {
        return Err(corrupt(stage, address));
    }
    check(
        stage,
        "section size",
        u64::from(decompressed),
        limits.max_section_bytes,
    )?;
    let data_start = start + 0x14;
    let data = data_start
        .checked_add(compressed as usize)
        .and_then(|end| bytes.get(data_start..end))
        .ok_or_else(|| DwgError::new(DwgErrorKind::Truncated, stage).at(address))?;

    header_bytes[0x10..].fill(0);
    let seed = page_checksum(0, &header_bytes);
    if page_checksum(seed, data) != stored {
        return Err(DwgError::new(DwgErrorKind::ChecksumMismatch, stage).at(address));
    }
    let mut out = vec![0u8; decompressed as usize];
    let written = decompress(data, &mut out)?;
    if written != out.len() {
        return Err(DwgError::new(DwgErrorKind::InvalidCompression, stage).at(address));
    }
    Ok(out)
}

/// Parses the decompressed page map. Addresses are not stored: each page
/// follows the one before it, starting at [`FIRST_PAGE`]. A negative page
/// number marks a gap (unused space) and is followed by four more integers.
pub(crate) fn parse_page_map(data: &[u8], limits: &DwgLimits) -> Result<PageMap, DwgError> {
    let stage = DwgStage::PageMap;
    let mut c = Cursor::new(data, stage);
    let mut map = PageMap::new();
    let mut address = FIRST_PAGE;
    let mut count = 0u64;
    while !c.is_empty() {
        count += 1;
        check(stage, "page count", count, limits.max_section_pages)?;
        let (number, size) = (c.i32()?, c.i32()?);
        let size = u32::try_from(size).map_err(|_| corrupt(stage, address))?;
        if number < 0 {
            c.skip(16)?;
        } else if number == 0 {
            return Err(corrupt(stage, address));
        } else {
            let entry = PageEntry { size, address };
            if map.insert(number as u32, entry).is_some() {
                return Err(corrupt(stage, address));
            }
        }
        address = address
            .checked_add(u64::from(size))
            .ok_or_else(|| corrupt(stage, address))?;
    }
    Ok(map)
}

fn section_name(raw: &[u8]) -> Result<String, DwgError> {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    let name = raw.get(..end).unwrap_or(raw);
    if name.is_ascii() {
        Ok(String::from_utf8_lossy(name).into_owned())
    } else {
        Err(corrupt(DwgStage::SectionMap, 0))
    }
}

/// Parses the decompressed section map and checks each section's pages against
/// the page map: they exist, and their start offsets strictly increase.
pub(crate) fn parse_section_map(
    data: &[u8],
    pages: &PageMap,
    limits: &DwgLimits,
) -> Result<Vec<SectionInfo>, DwgError> {
    let stage = DwgStage::SectionMap;
    let mut c = Cursor::new(data, stage);
    let count = u64::from(c.u32()?);
    c.skip(16)?;
    check(stage, "section count", count, limits.max_collection_items)?;
    if count.saturating_mul(0x60) > c.remaining() as u64 {
        return Err(DwgError::new(DwgErrorKind::Truncated, stage));
    }
    let mut sections = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let size = c.u64()?;
        let page_count = u64::from(c.u32()?);
        let max_page_size = c.u32()?;
        c.skip(4)?;
        let (compression, id, encrypted) = (c.u32()?, c.u32()?, c.u32()?);
        let name = section_name(c.take(64)?)?;
        check(stage, "section pages", page_count, limits.max_section_pages)?;
        if page_count.saturating_mul(16) > c.remaining() as u64 || !(1..=2).contains(&compression) {
            return Err(corrupt(stage, 0));
        }
        let mut refs = Vec::with_capacity(page_count as usize);
        let mut previous: Option<u64> = None;
        for _ in 0..page_count {
            let number = c.u32()?;
            let page = PageRef {
                number,
                data_size: c.u32()?,
                start: c.u64()?,
            };
            let in_order = previous.is_none_or(|p| page.start > p);
            if !pages.contains_key(&number) || !in_order {
                return Err(corrupt(stage, 0).in_section(&name));
            }
            previous = Some(page.start);
            refs.push(page);
        }
        sections.push(SectionInfo {
            name,
            id,
            size,
            compressed: compression == 2,
            encrypted,
            max_page_size,
            pages: refs,
        });
    }
    Ok(sections)
}
