//! Handles and the object map (Open Design Specification §2.13 and chapter 23).

use std::fmt;
use std::sync::atomic::AtomicBool;

use crate::bit::BitReader;
use crate::checksum::{crc16, CRC16_SEED};
use crate::container::check_cancelled;
use crate::cursor::Cursor;
use crate::error::{DwgError, DwgErrorKind, DwgStage};
use crate::limits::{check, DwgLimits};

/// An object's handle: its identity within one file. Handles are sparse and
/// file-local; they are never used as indexes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DwgHandle(pub u64);

impl fmt::Display for DwgHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:X}", self.0)
    }
}

/// A handle reference as stored: a code saying how to apply the value, and the
/// value. Use [`HandleRef::resolve`] for the handle it points at.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HandleRef {
    pub code: u8,
    pub value: u64,
}

impl HandleRef {
    /// The handle referred to, given the handle of the object holding the
    /// reference. `None` is the null reference. Codes 0 to 5 are absolute
    /// (§2.13); 6, 8, 10 and 12 are relative to `base`.
    pub fn resolve(self, base: DwgHandle) -> Result<Option<DwgHandle>, DwgError> {
        let invalid = || DwgError::new(DwgErrorKind::InvalidHandle, DwgStage::Object);
        let value = match self.code {
            0..=5 => Some(self.value),
            6 => base.0.checked_add(1),
            8 => base.0.checked_sub(1),
            10 => base.0.checked_add(self.value),
            12 => base.0.checked_sub(self.value),
            _ => return Err(invalid()),
        };
        match value {
            Some(0) if self.code <= 5 => Ok(None),
            Some(v) => Ok(Some(DwgHandle(v))),
            None => Err(invalid()),
        }
    }
}

impl BitReader<'_> {
    /// H: a handle reference, a 4-bit code, a 4-bit byte count, then that many
    /// bytes, most significant first.
    pub fn handle(&mut self, max_bytes: u8) -> Result<HandleRef, DwgError> {
        let code = self.bits(4)? as u8;
        let count = self.bits(4)? as u8;
        if count > max_bytes.min(8) {
            return Err(self.error(DwgErrorKind::InvalidHandle));
        }
        let mut value = 0u64;
        for _ in 0..count {
            value = (value << 8) | u64::from(self.rc()?);
        }
        Ok(HandleRef { code, value })
    }
}

/// Where one object starts in `AcDb:AcDbObjects`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MapEntry {
    pub handle: DwgHandle,
    pub offset: u64,
}

/// The longest block of the object map, size field included. The
/// specification gives 2032, but AutoCAD stops adding pairs only once a block
/// passes that, so the last pair (two modular chars of at most 8 bytes each)
/// may run over.
const MAX_MAP_BLOCK: u16 = 2032 + 16;

/// Decodes `AcDb:Handles` into entries sorted by handle. Each block is a
/// big-endian size (counting itself), pairs of modular chars (handle delta,
/// offset delta, which may be negative), and a big-endian CRC; an empty block
/// of size 2 ends the map, and anything after it is ignored. The deltas start again from zero in every block,
/// so a block's first pair is an absolute handle and offset. Handles must
/// increase across the whole map and every offset must lie inside the
/// objects section.
pub(crate) fn parse_object_map(
    data: &[u8],
    objects_len: u64,
    limits: &DwgLimits,
    cancelled: &AtomicBool,
) -> Result<Vec<MapEntry>, DwgError> {
    let stage = DwgStage::ObjectMap;
    let bad = |at: usize| DwgError::new(DwgErrorKind::InvalidHandle, stage).at(at as u64);
    let mut c = Cursor::new(data, stage);
    let mut entries: Vec<MapEntry> = Vec::new();
    loop {
        let (mut handle, mut offset) = (0u64, 0i64);
        let start = c.position();
        let size = c.u16_be()?;
        if !(2..=MAX_MAP_BLOCK).contains(&size) {
            return Err(bad(start));
        }
        let body_len = usize::from(size) - 2;
        let mut body = Cursor::new(c.take(body_len)?, stage);
        let stored = c.u16_be()?;
        let block = data
            .get(start..start + usize::from(size))
            .ok_or_else(|| bad(start))?;
        if crc16(CRC16_SEED, block) != stored {
            return Err(DwgError::new(DwgErrorKind::ChecksumMismatch, stage).at(start as u64));
        }
        while !body.is_empty() {
            let delta = u64::try_from(body.modular_char(false)?).map_err(|_| bad(start))?;
            let step = body.modular_char(true)?;
            handle = handle.checked_add(delta).ok_or_else(|| bad(start))?;
            let last = entries.last().map_or(0, |e| e.handle.0);
            if handle <= last {
                return Err(bad(start));
            }
            offset = offset.checked_add(step).ok_or_else(|| bad(start))?;
            let at = u64::try_from(offset).ok().filter(|&o| o < objects_len);
            let at = at.ok_or_else(|| bad(start))?;
            let count = entries.len() as u64 + 1;
            check(stage, "object count", count, limits.max_objects)?;
            entries.push(MapEntry {
                handle: DwgHandle(handle),
                offset: at,
            });
            if count.is_multiple_of(4096) {
                check_cancelled(cancelled, stage)?;
            }
        }
        if size == 2 {
            break;
        }
    }
    // Whatever follows the empty block is stale padding, not part of the map.
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(body: &[u8]) -> Vec<u8> {
        let mut out = ((body.len() + 2) as u16).to_be_bytes().to_vec();
        out.extend(body);
        out.extend(crc16(CRC16_SEED, &out).to_be_bytes());
        out
    }

    fn map(blocks: &[&[u8]]) -> Vec<u8> {
        let mut out: Vec<u8> = blocks.iter().flat_map(|b| block(b)).collect();
        out.extend(block(&[]));
        out
    }

    fn parse(data: &[u8], len: u64) -> Result<Vec<MapEntry>, DwgError> {
        parse_object_map(data, len, &DwgLimits::default(), &AtomicBool::new(false))
    }

    #[test]
    fn deltas_accumulate_and_offsets_may_go_back() {
        // Handle +1 at 4, handle +2 at 4 + 18, handle +1 at 22 - 10 (0x4A: negative 10).
        let data = map(&[&[0x01, 0x04, 0x02, 0x12, 0x01, 0x4A]]);
        let entries = parse(&data, 100).unwrap();
        let got: Vec<(u64, u64)> = entries.iter().map(|e| (e.handle.0, e.offset)).collect();
        assert_eq!(got, [(1, 4), (3, 22), (4, 12)]);
    }

    #[test]
    fn each_block_starts_its_totals_from_zero() {
        // As AutoCAD 2018 writes a map longer than one block: the second
        // block's first pair is the absolute handle and offset.
        let data = map(&[&[0x01, 0x04, 0x01, 0x04], &[0x05, 0x20, 0x01, 0x04]]);
        let entries = parse(&data, 100).unwrap();
        let got: Vec<(u64, u64)> = entries.iter().map(|e| (e.handle.0, e.offset)).collect();
        assert_eq!(got, [(1, 4), (2, 8), (5, 32), (6, 36)]);
        assert!(
            parse(&map(&[&[0x02, 0x04], &[0x02, 0x08]]), 100).is_err(),
            "a later block may not go back to an earlier handle"
        );
    }

    #[test]
    fn rejects_damage() {
        let good = map(&[&[0x01, 0x04]]);
        let mut bad_crc = good.clone();
        bad_crc[3] ^= 1;
        assert_eq!(
            parse(&bad_crc, 100).unwrap_err().kind,
            DwgErrorKind::ChecksumMismatch
        );
        assert!(parse(&good[..good.len() - 1], 100).is_err(), "truncated");
        let mut padded = good.clone();
        padded.extend([0x2F, 0x04, 0x31]);
        assert_eq!(parse(&padded, 100).unwrap().len(), 1, "padding is ignored");
        assert!(
            parse(&map(&[&[0x00, 0x04]]), 100).is_err(),
            "handles must increase"
        );
        assert!(
            parse(&map(&[&[0x01, 0x04]]), 4).is_err(),
            "offset outside the section"
        );
        assert!(
            parse(&map(&[&[0x01, 0x44]]), 100).is_err(),
            "negative offset"
        );
        assert!(
            parse(&[0x00, 0x01, 0x00, 0x00], 100).is_err(),
            "block size below 2"
        );
        let mut long = vec![0x01, 0x04];
        long.extend([0x01, 0x00].repeat(1020));
        assert!(parse(&map(&[&long]), 100).is_ok(), "a block of 2044 bytes");
        long.extend([0x01, 0x00].repeat(3));
        assert!(parse(&map(&[&long]), 100).is_err(), "a block of 2050 bytes");
        assert!(parse(&[], 100).is_err());
    }

    #[test]
    fn the_object_count_limit_applies() {
        let data = map(&[&[0x01, 0x04, 0x01, 0x04]]);
        let limits = DwgLimits {
            max_objects: 1,
            ..DwgLimits::default()
        };
        let err = parse_object_map(&data, 100, &limits, &AtomicBool::new(false)).unwrap_err();
        assert!(matches!(err.kind, DwgErrorKind::LimitExceeded { .. }));
    }

    #[test]
    fn handle_references_read_big_endian_bytes() {
        // §2.13: 5.2.05.E7 is handle 0x5E7, hard pointer.
        let data = [0x52, 0x05, 0xE7, 0x40];
        let mut r = BitReader::new(&data, DwgStage::Object);
        assert_eq!(
            r.handle(8),
            Ok(HandleRef {
                code: 5,
                value: 0x5E7
            })
        );
        assert_eq!(r.handle(8), Ok(HandleRef { code: 4, value: 0 }));
        let too_long = [0x59, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        assert!(BitReader::new(&too_long, DwgStage::Object)
            .handle(8)
            .is_err());
        assert!(BitReader::new(&[0x42, 0x01], DwgStage::Object)
            .handle(1)
            .is_err());
        assert!(BitReader::new(&[0x42, 0x01], DwgStage::Object)
            .handle(8)
            .is_err());
    }

    #[test]
    fn relative_references_resolve_against_the_holder() {
        let base = DwgHandle(0x20);
        let r = |code, value| HandleRef { code, value };
        assert_eq!(r(5, 0x10).resolve(base), Ok(Some(DwgHandle(0x10))));
        assert_eq!(r(4, 0).resolve(base), Ok(None), "null stays null");
        assert_eq!(r(6, 0).resolve(base), Ok(Some(DwgHandle(0x21))));
        assert_eq!(r(8, 0).resolve(base), Ok(Some(DwgHandle(0x1F))));
        assert_eq!(r(10, 5).resolve(base), Ok(Some(DwgHandle(0x25))));
        assert_eq!(r(12, 5).resolve(base), Ok(Some(DwgHandle(0x1B))));
        assert!(r(12, 0x30).resolve(base).is_err(), "below zero");
        assert!(r(10, u64::MAX).resolve(base).is_err(), "overflow");
        assert!(r(7, 1).resolve(base).is_err(), "unknown code");
    }
}
