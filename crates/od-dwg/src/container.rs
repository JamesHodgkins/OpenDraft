//! The file header and the container as a whole: probe the version, decode the
//! header block, follow it to the page map and section map, then hand out
//! logical sections by name.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::checksum::crc32;
use crate::cursor::Cursor;
use crate::error::{DwgError, DwgErrorKind, DwgStage};
use crate::limits::{check, DwgLimits};
use crate::mask::{unmask_file_header, FILE_HEADER_BLOCK};
use crate::section_map::{
    parse_page_map, parse_section_map, read_system_page, PageMap, SectionInfo, FIRST_PAGE,
    PAGE_MAP_TYPE, SECTION_MAP_TYPE,
};
use crate::sections::assemble;
use crate::version::DwgVersion;

const HEADER_BLOCK_AT: usize = 0x80;
const FILE_ID: &[u8; 12] = b"AcFssFcAJMB\0";

/// The names of the logical sections the importer needs.
pub mod section_names {
    pub const HEADER: &str = "AcDb:Header";
    pub const CLASSES: &str = "AcDb:Classes";
    pub const OBJECTS: &str = "AcDb:AcDbObjects";
    pub const HANDLES: &str = "AcDb:Handles";
    pub const AUX_HEADER: &str = "AcDb:AuxHeader";
}

/// What the decoded file header block says about where the maps are.
struct HeaderBlock {
    page_map_id: u32,
    page_map_address: u64,
    section_map_id: u32,
}

fn parse_header_block(bytes: &[u8]) -> Result<HeaderBlock, DwgError> {
    let stage = DwgStage::FileHeader;
    let mut block: [u8; FILE_HEADER_BLOCK] = bytes
        .get(HEADER_BLOCK_AT..HEADER_BLOCK_AT + FILE_HEADER_BLOCK)
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| DwgError::new(DwgErrorKind::Truncated, stage))?;
    unmask_file_header(&mut block);

    let stored_crc = Cursor::new(block.get(0x68..).unwrap_or(&[]), stage).u32()?;
    block[0x68..].fill(0);
    if crc32(0, &block) != stored_crc {
        return Err(DwgError::new(DwgErrorKind::ChecksumMismatch, stage));
    }

    let mut c = Cursor::new(&block, stage);
    let id_ok = c.take(12)? == FILE_ID;
    c.skip(4)?;
    let size_ok = c.u32()? == FILE_HEADER_BLOCK as u32;
    c.skip(0x50 - 0x14)?;
    if !id_ok || !size_ok {
        return Err(DwgError::new(DwgErrorKind::InvalidHeader, stage));
    }
    let page_map_id = c.u32()?;
    let page_map_address = c.u64()?;
    let section_map_id = c.u32()?;
    Ok(HeaderBlock {
        page_map_id,
        page_map_address,
        section_map_id,
    })
}

/// Fails with `Cancelled` when the flag is set. Called between pages and
/// between other units of work that can take noticeable time.
pub(crate) fn check_cancelled(cancelled: &AtomicBool, stage: DwgStage) -> Result<(), DwgError> {
    if cancelled.load(Ordering::Relaxed) {
        Err(DwgError::new(DwgErrorKind::Cancelled, stage))
    } else {
        Ok(())
    }
}

/// An opened DWG container: its version and the table of contents. Sections
/// are decompressed one at a time on request.
pub struct Container<'a> {
    bytes: &'a [u8],
    version: DwgVersion,
    maintenance: u8,
    pages: PageMap,
    sections: Vec<SectionInfo>,
    limits: DwgLimits,
    logical_total: u64,
}

impl<'a> Container<'a> {
    /// Reads the file header and both maps, verifying every checksum on the way.
    pub fn open(
        bytes: &'a [u8],
        limits: &DwgLimits,
        cancelled: &AtomicBool,
    ) -> Result<Container<'a>, DwgError> {
        check(
            DwgStage::Probe,
            "file size",
            bytes.len() as u64,
            limits.max_file_bytes,
        )?;
        let version = DwgVersion::probe(bytes)?;
        let truncated = || DwgError::new(DwgErrorKind::Truncated, DwgStage::FileHeader);
        if bytes.len() < FIRST_PAGE as usize {
            return Err(truncated());
        }
        let maintenance = bytes.get(0x0B).copied().ok_or_else(truncated)?;
        // §4.1: bit 0x01 encrypts every data section; bit 0x02 only the
        // properties and preview, which are never read.
        let security = Cursor::new(bytes.get(0x18..).unwrap_or(&[]), DwgStage::FileHeader).u32()?;
        if security & 1 != 0 {
            return Err(DwgError::new(
                DwgErrorKind::UnsupportedProtection,
                DwgStage::FileHeader,
            ));
        }
        check_cancelled(cancelled, DwgStage::FileHeader)?;
        let header = parse_header_block(bytes)?;

        let map_address = FIRST_PAGE
            .checked_add(header.page_map_address)
            .ok_or_else(|| DwgError::new(DwgErrorKind::InvalidHeader, DwgStage::FileHeader))?;
        let stage = DwgStage::PageMap;
        let data = read_system_page(bytes, map_address, PAGE_MAP_TYPE, stage, limits, cancelled)?;
        let pages = parse_page_map(&data, limits)?;
        let own = pages.get(&header.page_map_id);
        if own.is_some_and(|entry| entry.address != map_address) {
            return Err(DwgError::new(DwgErrorKind::InvalidSectionMap, stage).at(map_address));
        }

        let entry = pages
            .get(&header.section_map_id)
            .ok_or_else(|| DwgError::new(DwgErrorKind::InvalidSectionMap, DwgStage::SectionMap))?;
        let stage = DwgStage::SectionMap;
        let data = read_system_page(
            bytes,
            entry.address,
            SECTION_MAP_TYPE,
            stage,
            limits,
            cancelled,
        )?;
        let sections = parse_section_map(&data, &pages, limits)?;

        Ok(Container {
            bytes,
            version,
            maintenance,
            pages,
            sections,
            limits: limits.clone(),
            logical_total: 0,
        })
    }

    pub fn version(&self) -> DwgVersion {
        self.version
    }

    /// The AutoCAD maintenance release recorded in the file header.
    pub fn maintenance_version(&self) -> u8 {
        self.maintenance
    }

    pub fn sections(&self) -> &[SectionInfo] {
        &self.sections
    }

    /// The section called `name`, if the file has exactly one.
    pub fn section_info(&self, name: &str) -> Result<Option<&SectionInfo>, DwgError> {
        let mut found = self.sections.iter().filter(|s| s.name == name);
        let first = found.next();
        if found.next().is_some() {
            let err = DwgError::new(DwgErrorKind::InvalidSectionMap, DwgStage::SectionMap);
            return Err(err.in_section(name));
        }
        Ok(first)
    }

    /// Decompresses and returns a section whose absence is an error. Each
    /// call counts its full size against the total budget.
    pub fn read_section(
        &mut self,
        name: &'static str,
        cancelled: &AtomicBool,
    ) -> Result<Vec<u8>, DwgError> {
        let stage = DwgStage::SectionPage;
        let info = self
            .section_info(name)?
            .ok_or_else(|| DwgError::new(DwgErrorKind::MissingSection(name), stage))?
            .clone();
        if info.encrypted == 1 {
            return Err(DwgError::new(DwgErrorKind::UnsupportedProtection, stage).in_section(name));
        }
        let total = self.logical_total.saturating_add(info.size);
        check(
            stage,
            "total decompressed size",
            total,
            self.limits.max_total_logical_bytes,
        )?;
        let data = assemble(self.bytes, &self.pages, &info, &self.limits, cancelled)?;
        self.logical_total = total;
        Ok(data)
    }
}
