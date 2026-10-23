//! Reading data section pages and assembling them into logical sections
//! (Open Design Specification §4.5 and §4.6).

use std::sync::atomic::AtomicBool;

use crate::checksum::page_checksum;
use crate::compression::decompress;
use crate::cursor::Cursor;
use crate::error::{DwgError, DwgErrorKind, DwgStage};
use crate::limits::{check, DwgLimits};
use crate::mask::unmask_page_header;
use crate::section_map::{PageMap, PageRef, SectionInfo};

const DATA_PAGE_TYPE: u32 = 0x4163_043B;
const PAGE_HEADER: usize = 0x20;

fn corrupt(info: &SectionInfo, at: u64) -> DwgError {
    DwgError::new(DwgErrorKind::InvalidSectionMap, DwgStage::SectionPage)
        .at(at)
        .in_section(&info.name)
}

/// Decodes one data page and writes the first `dest.len()` bytes of its
/// decompressed content to `dest`. `scratch` holds a whole decompressed page.
fn read_page(
    bytes: &[u8],
    pages: &PageMap,
    info: &SectionInfo,
    page: &PageRef,
    scratch: &mut [u8],
    dest: &mut [u8],
) -> Result<(), DwgError> {
    let stage = DwgStage::SectionPage;
    let entry = pages.get(&page.number).ok_or_else(|| corrupt(info, 0))?;
    let at = entry.address;
    let start = usize::try_from(at).map_err(|_| corrupt(info, at))?;
    let mut header: [u8; PAGE_HEADER] = bytes
        .get(start..)
        .and_then(|rest| rest.get(..PAGE_HEADER))
        .and_then(|h| h.try_into().ok())
        .ok_or_else(|| {
            DwgError::new(DwgErrorKind::Truncated, stage)
                .at(at)
                .in_section(&info.name)
        })?;
    unmask_page_header(&mut header, at);

    let mut c = Cursor::new(&header, stage);
    let (page_type, section, data_size) = (c.u32()?, c.u32()?, c.u32()?);
    let (page_size, page_start) = (c.u32()?, c.u64()?);
    let (header_sum, data_sum) = (c.u32()?, c.u32()?);
    let sane = page_type == DATA_PAGE_TYPE
        && section == info.id
        && data_size == page.data_size
        && page_size == entry.size
        && page_start == page.start
        && u64::from(data_size) + PAGE_HEADER as u64 <= u64::from(page_size);
    if !sane {
        return Err(corrupt(info, at));
    }
    let data_start = start + PAGE_HEADER;
    let data = data_start
        .checked_add(data_size as usize)
        .and_then(|end| bytes.get(data_start..end))
        .ok_or_else(|| {
            DwgError::new(DwgErrorKind::Truncated, stage)
                .at(at)
                .in_section(&info.name)
        })?;

    // The data checksum seeds the header's, which is taken with its own field zeroed.
    header[0x18..0x1C].fill(0);
    let mismatch =
        page_checksum(0, data) != data_sum || page_checksum(data_sum, &header) != header_sum;
    if mismatch {
        return Err(DwgError::new(DwgErrorKind::ChecksumMismatch, stage)
            .at(at)
            .in_section(&info.name));
    }

    let content = if info.compressed {
        let written = decompress(data, scratch)?;
        scratch.get(..written)
    } else {
        Some(data)
    };
    let prefix = content.and_then(|c| c.get(..dest.len()));
    let prefix = prefix.ok_or_else(|| {
        DwgError::new(DwgErrorKind::InvalidCompression, stage)
            .at(at)
            .in_section(&info.name)
    })?;
    dest.copy_from_slice(prefix);
    Ok(())
}

/// Joins a section's pages into one buffer. Pages omitted from the file (all
/// zero) stay zero. Each page must begin at a multiple of the page size, in
/// order, inside the section.
pub(crate) fn assemble(
    bytes: &[u8],
    pages: &PageMap,
    info: &SectionInfo,
    limits: &DwgLimits,
    cancelled: &AtomicBool,
) -> Result<Vec<u8>, DwgError> {
    let stage = DwgStage::SectionPage;
    check(stage, "section size", info.size, limits.max_section_bytes)?;
    check(
        stage,
        "section pages",
        info.pages.len() as u64,
        limits.max_section_pages,
    )?;
    let page_size = u64::from(info.max_page_size);
    if !info.pages.is_empty() {
        check(stage, "page size", page_size, limits.max_page_bytes)?;
        if page_size == 0 {
            return Err(corrupt(info, 0));
        }
    }
    let mut out = vec![0u8; info.size as usize];
    let mut scratch = vec![0u8; page_size as usize];
    for page in &info.pages {
        crate::check_cancelled(cancelled, stage)?;
        if page.start >= info.size || page.start % page_size != 0 {
            return Err(corrupt(info, page.start));
        }
        let begin = page.start as usize;
        let logical = (page_size.min(info.size - page.start)) as usize;
        let dest = out
            .get_mut(begin..begin + logical)
            .ok_or_else(|| corrupt(info, page.start))?;
        read_page(bytes, pages, info, page, &mut scratch, dest)?;
    }
    crate::check_cancelled(cancelled, stage)?;
    Ok(out)
}
