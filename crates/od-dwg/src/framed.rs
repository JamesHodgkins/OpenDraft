//! The frame around the header and class sections: a start sentinel, a size,
//! the data, a CRC and an end sentinel (Open Design Specification chapters 9
//! and 10).

use crate::checksum::{crc16, CRC16_SEED};
use crate::cursor::Cursor;
use crate::error::{DwgError, DwgErrorKind, DwgStage};
use crate::version::DwgVersion;

pub(crate) const CLASSES_START: [u8; 16] = [
    0x8D, 0xA1, 0xC4, 0xB8, 0xC4, 0xA9, 0xF8, 0xC5, 0xC0, 0xDC, 0xF4, 0x5F, 0xE7, 0xCF, 0xB6, 0x8A,
];
pub(crate) const CLASSES_END: [u8; 16] = [
    0x72, 0x5E, 0x3B, 0x47, 0x3B, 0x56, 0x07, 0x3A, 0x3F, 0x23, 0x0B, 0xA0, 0x18, 0x30, 0x49, 0x75,
];
pub(crate) const HEADER_START: [u8; 16] = [
    0xCF, 0x7B, 0x1F, 0x23, 0xFD, 0xDE, 0x38, 0xA9, 0x5F, 0x7C, 0x68, 0xB8, 0x4E, 0x6D, 0x33, 0x5F,
];
pub(crate) const HEADER_END: [u8; 16] = [
    0x30, 0x84, 0xE0, 0xDC, 0x02, 0x21, 0xC7, 0x56, 0xA0, 0x83, 0x97, 0x47, 0xB1, 0x92, 0xCC, 0xA0,
];

/// Returns the data between the size words and the CRC, after checking both
/// sentinels, and the CRC (which covers the size words).
fn locate<'a>(
    data: &'a [u8],
    with_high_word: bool,
    sentinels: (&[u8; 16], &[u8; 16]),
    stage: DwgStage,
    name: &str,
) -> Result<&'a [u8], DwgError> {
    let mut c = Cursor::new(data, stage);
    if c.take(16)? != sentinels.0 {
        return Err(DwgError::new(DwgErrorKind::InvalidHeader, stage));
    }
    let size = c.u32()?;
    if with_high_word && c.u32()? != 0 {
        return Err(DwgError::new(DwgErrorKind::InvalidHeader, stage));
    }
    let region = c.take(size as usize)?;
    let covered = c.position();
    let stored = c.u16()?;
    if crc16(CRC16_SEED, data.get(16..covered).unwrap_or(&[])) != stored {
        return Err(DwgError::new(DwgErrorKind::ChecksumMismatch, stage).in_section(name));
    }
    if c.take(16)? != sentinels.1 {
        return Err(DwgError::new(DwgErrorKind::InvalidHeader, stage));
    }
    // Whatever follows the end sentinel is padding to the section's stored
    // size. Writers leave it zero-filled or holding stale bytes, and of any
    // length, so it is not checked.
    Ok(region)
}

/// Finds the data region of a framed section. R2010 and later may put a
/// second size word (the high half of a 64-bit size) after the first; rather
/// than trust a version rule, accept the layout whose CRC and end sentinel
/// both check out.
pub(crate) fn frame<'a>(
    data: &'a [u8],
    version: DwgVersion,
    sentinels: (&[u8; 16], &[u8; 16]),
    stage: DwgStage,
    name: &str,
) -> Result<&'a [u8], DwgError> {
    match locate(data, true, sentinels, stage, name) {
        Err(_) if version < DwgVersion::R2018 => locate(data, false, sentinels, stage, name),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn framed(region: &[u8], tail: &[u8]) -> Vec<u8> {
        let mut out = CLASSES_START.to_vec();
        out.extend((region.len() as u32).to_le_bytes());
        out.extend([0; 4]);
        out.extend(region);
        let crc = crc16(CRC16_SEED, &out[16..]);
        out.extend(crc.to_le_bytes());
        out.extend(CLASSES_END);
        out.extend(tail);
        out
    }

    fn open(data: &[u8]) -> Result<&[u8], DwgError> {
        let sentinels = (&CLASSES_START, &CLASSES_END);
        frame(
            data,
            DwgVersion::R2018,
            sentinels,
            DwgStage::Classes,
            "AcDb:Classes",
        )
    }

    #[test]
    fn padding_after_the_end_sentinel_is_ignored() {
        assert_eq!(open(&framed(b"abc", &[])).unwrap(), b"abc");
        assert_eq!(open(&framed(b"abc", &[0; 18])).unwrap(), b"abc");
        assert_eq!(open(&framed(b"abc", &[0x49, 0x75, 7])).unwrap(), b"abc");
    }

    #[test]
    fn a_damaged_frame_is_still_refused() {
        let mut data = framed(b"abc", &[]);
        data[20] ^= 1;
        assert!(open(&data).is_err(), "checksum");
        let mut data = framed(b"abc", &[]);
        let end = data.len() - 1;
        data[end] ^= 1;
        assert!(open(&data).is_err(), "end sentinel");
    }
}
