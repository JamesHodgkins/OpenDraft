//! One object in `AcDb:AcDbObjects`: its envelope and its three bit streams
//! (Open Design Specification §20.1 and §20.2).
//!
//! Layout, from the object's offset: a modular short size (bytes of the body),
//! a modular char giving the handle stream's length in bits, the body, and a
//! 16-bit CRC. The CRC covers everything before it, starting from 0xC0C1. The
//! size counts the body only, not the two size fields; the supplied drawings
//! confirm it by the offsets of consecutive objects.

use crate::bit::BitReader;
use crate::checksum::{crc16, CRC16_SEED};
use crate::cursor::Cursor;
use crate::error::{DwgError, DwgErrorKind, DwgStage};
use crate::limits::{check, DwgLimits};

/// The three streams an object body is split into. Data is read first, strings
/// are read only through it (each `TV` field takes its text from here), and
/// handles are read last.
pub(crate) struct ObjectStreams<'a> {
    pub data: BitReader<'a>,
    pub strings: BitReader<'a>,
    pub handles: BitReader<'a>,
}

fn invalid(at: u64) -> DwgError {
    DwgError::new(DwgErrorKind::InvalidObject, DwgStage::Object).at(at)
}

/// Splits `body` into streams. The handle stream is its last `handle_bits`
/// bits. The bit before it says whether a string stream exists; if so, the
/// 16 bits before that hold its length in bits (with a second 16 bits holding
/// the high part when the first has its 0x8000 flag), and the string stream
/// ends just before them. Data is everything before the string stream.
pub(crate) fn split_streams(
    body: &[u8],
    handle_bits: u64,
    stage: DwgStage,
) -> Result<ObjectStreams<'_>, DwgError> {
    let whole = BitReader::new(body, stage);
    let total = body.len() as u64 * 8;
    let handles_start = total
        .checked_sub(handle_bits)
        .filter(|&s| s > 0)
        .ok_or_else(|| invalid(0))?;
    let handles = whole.window(handles_start, total)?;
    let flag_at = handles_start - 1;
    let has_strings = whole.window(flag_at, handles_start)?.bit()?;
    if !has_strings {
        let empty = whole.window(flag_at, flag_at)?;
        return Ok(ObjectStreams {
            data: whole.window(0, flag_at)?,
            strings: empty,
            handles,
        });
    }
    let mut size_at = flag_at.checked_sub(16).ok_or_else(|| invalid(flag_at))?;
    let low = whole.window(size_at, size_at + 16)?.rs()?;
    let mut size = u64::from(low);
    if low & 0x8000 != 0 {
        size_at = size_at.checked_sub(16).ok_or_else(|| invalid(size_at))?;
        let high = whole.window(size_at, size_at + 16)?.rs()?;
        size = u64::from(low & 0x7FFF) | (u64::from(high) << 15);
    }
    let strings_start = size_at.checked_sub(size).ok_or_else(|| invalid(size_at))?;
    Ok(ObjectStreams {
        data: whole.window(0, strings_start)?,
        strings: whole.window(strings_start, size_at)?,
        handles,
    })
}

/// An object whose envelope has been checked and whose type has been read.
pub(crate) struct RawObject<'a> {
    /// Bytes from the object's offset to the end of its CRC.
    pub extent: u64,
    pub type_code: u32,
    /// `data` is positioned just after the type.
    pub streams: ObjectStreams<'a>,
}

/// Reads the object at `offset` in the objects section: checks its size
/// against the limits and its CRC, splits its streams and reads its type.
pub(crate) fn read_object<'a>(
    section: &'a [u8],
    offset: u64,
    limits: &DwgLimits,
) -> Result<RawObject<'a>, DwgError> {
    let stage = DwgStage::Object;
    let start = usize::try_from(offset).map_err(|_| invalid(offset))?;
    let rest = section.get(start..).ok_or_else(|| invalid(offset))?;
    let mut c = Cursor::new(rest, stage);
    let size = u64::from(c.modular_short().map_err(|e| e.at(offset))?);
    check(stage, "object size", size, limits.max_single_object_bytes)?;
    let handle_bits = c.modular_char(false).map_err(|e| e.at(offset))?;
    let handle_bits = u64::try_from(handle_bits).map_err(|_| invalid(offset))?;
    let body = c.take(size as usize).map_err(|e| e.at(offset))?;
    let before_crc = c.position();
    let stored = c.u16().map_err(|e| e.at(offset))?;
    if crc16(CRC16_SEED, rest.get(..before_crc).unwrap_or(&[])) != stored {
        return Err(DwgError::new(DwgErrorKind::ChecksumMismatch, stage).at(offset));
    }
    let mut streams = split_streams(body, handle_bits, stage).map_err(|e| e.at(offset))?;
    let type_code = streams.data.object_type().map_err(|e| e.at(offset))?;
    Ok(RawObject {
        extent: c.position() as u64,
        type_code,
        streams,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a body of `data_bits` + optional strings + `handle_bits` from bit strings.
    fn pack(bits: &str) -> Vec<u8> {
        let bits: Vec<u8> = bits
            .bytes()
            .filter(|b| *b != b' ')
            .map(|b| b - b'0')
            .collect();
        bits.chunks(8)
            .map(|c| {
                c.iter()
                    .enumerate()
                    .fold(0u8, |acc, (i, b)| acc | (b << (7 - i)))
            })
            .collect()
    }

    fn rs_bits(v: u16) -> String {
        format!("{:08b}{:08b}", v as u8, (v >> 8) as u8)
    }

    #[test]
    fn no_string_stream() {
        // 11 data bits, the flag (0), then 12 handle bits (8 and 4 of padding).
        let body = pack("10101010101 0 11110000 0000");
        let mut s = split_streams(&body, 12, DwgStage::Object).unwrap();
        assert_eq!(s.data.remaining(), 11);
        assert_eq!(s.strings.remaining(), 0);
        assert_eq!(s.handles.remaining(), 12);
        assert_eq!(s.data.bits(11), Ok(0b101_0101_0101));
        assert_eq!(s.handles.bits(8), Ok(0xF0));
    }

    #[test]
    fn a_string_stream_is_found_from_the_end() {
        // 10 data bits, 24 string bits, a 16-bit length (24), the flag, 8 handle bits.
        let strings = "1111000011110000 11001100";
        let bits = format!("1010101010 {strings} {} 1 10101010 00000", rs_bits(24));
        let body = pack(&bits);
        assert_eq!(body.len() * 8, 10 + 24 + 16 + 1 + 8 + 5);
        let mut s = split_streams(&body, 8 + 5, DwgStage::Object).unwrap();
        assert_eq!(s.data.remaining(), 10);
        assert_eq!(s.strings.remaining(), 24);
        assert_eq!(s.strings.bits(16), Ok(0xF0F0));
        assert_eq!(s.handles.remaining(), 13);
    }

    #[test]
    fn a_long_string_stream_uses_two_lengths() {
        // Length 0x8005 low part with a high part of 1: 5 | (1 << 15) = 32773 bits.
        let low = 0x8000 | 5;
        let n = 5 + (1usize << 15);
        let bits = format!(
            "11 {} {} {} 1 11111111",
            "0".repeat(n),
            rs_bits(1),
            rs_bits(low)
        );
        let body = pack(&bits);
        let s = split_streams(&body, 8, DwgStage::Object).unwrap();
        assert_eq!(s.strings.remaining(), n as u64);
        assert_eq!(s.data.remaining(), 2);
    }

    #[test]
    fn impossible_splits_are_errors() {
        let body = pack("11111111 11111111");
        assert!(
            split_streams(&body, 17, DwgStage::Object).is_err(),
            "more handle bits than bits"
        );
        assert!(
            split_streams(&body, 16, DwgStage::Object).is_err(),
            "no room for the flag"
        );
        let bits = format!("1 {} 1 1111 00", rs_bits(500));
        assert!(
            split_streams(&pack(&bits), 6, DwgStage::Object).is_err(),
            "string stream longer than the body"
        );
    }

    fn object_bytes(body: &[u8], handle_bits: u8) -> Vec<u8> {
        let mut out = (body.len() as u16).to_le_bytes().to_vec();
        out.push(handle_bits);
        out.extend(body);
        out.extend(crc16(CRC16_SEED, &out).to_le_bytes());
        out
    }

    #[test]
    fn an_object_envelope_is_checked_and_its_type_read() {
        // Type 0x13 (LINE) as BB 00 + RC, padding, the flag (0), 8 handle bits.
        let body = pack("00 00010011 00000 0 11111111");
        let bytes = object_bytes(&body, 8);
        let obj = read_object(&bytes, 0, &DwgLimits::default()).unwrap();
        assert_eq!(obj.type_code, 0x13);
        assert_eq!(obj.extent, bytes.len() as u64);

        let mut padded = vec![0xAA, 0xBB];
        padded.extend(&bytes);
        assert_eq!(
            read_object(&padded, 2, &DwgLimits::default())
                .unwrap()
                .type_code,
            0x13
        );
    }

    #[test]
    fn a_damaged_envelope_is_refused() {
        let body = pack("00 00010011 00000 0 11111111");
        let good = object_bytes(&body, 8);
        let limits = DwgLimits::default();
        let mut bad = good.clone();
        bad[4] ^= 1;
        assert_eq!(
            read_object(&bad, 0, &limits).err().unwrap().kind,
            DwgErrorKind::ChecksumMismatch
        );
        assert!(read_object(&good[..good.len() - 1], 0, &limits).is_err());
        assert!(read_object(&good, 99, &limits).is_err());
        let small = DwgLimits {
            max_single_object_bytes: 2,
            ..DwgLimits::default()
        };
        let err = read_object(&good, 0, &small).err().unwrap();
        assert!(matches!(err.kind, DwgErrorKind::LimitExceeded { .. }));
    }
}
