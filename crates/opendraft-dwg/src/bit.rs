//! The DWG bit codes (Open Design Specification chapter 2). Bits are read
//! most significant first; multi-byte values are little-endian but need not
//! start on a byte boundary.

// Some codes (2RD, 3BD, thickness, ...) are only used by the entity decoders,
// which come with the semantic importer; they are tested here first.
#![allow(dead_code)]

use crate::error::{DwgError, DwgErrorKind, DwgStage};

/// A reader over a window of bits in a byte slice. Positions are absolute bit
/// indexes into the slice; a read past the window's end is `Truncated`.
#[derive(Clone)]
pub(crate) struct BitReader<'a> {
    data: &'a [u8],
    pos: u64,
    end: u64,
    stage: DwgStage,
}

impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8], stage: DwgStage) -> Self {
        BitReader {
            data,
            pos: 0,
            end: data.len() as u64 * 8,
            stage,
        }
    }

    /// A reader over bits `start..end` of the same data, which must lie inside
    /// this reader's window.
    pub fn window(&self, start: u64, end: u64) -> Result<BitReader<'a>, DwgError> {
        if start > end || end > self.end {
            return Err(self.error(DwgErrorKind::InvalidObject));
        }
        Ok(BitReader {
            data: self.data,
            pos: start,
            end,
            stage: self.stage,
        })
    }

    pub fn stage(&self) -> DwgStage {
        self.stage
    }

    pub fn position(&self) -> u64 {
        self.pos
    }

    pub fn remaining(&self) -> u64 {
        self.end - self.pos
    }

    pub fn error(&self, kind: DwgErrorKind) -> DwgError {
        DwgError::new(kind, self.stage).at(self.pos)
    }

    /// Reads `n` (at most 64) bits as an unsigned number.
    pub fn bits(&mut self, n: u32) -> Result<u64, DwgError> {
        if u64::from(n) > self.remaining() {
            return Err(self.error(DwgErrorKind::Truncated));
        }
        let mut value = 0u64;
        let mut left = n;
        while left > 0 {
            let offset = (self.pos & 7) as u32;
            let take = left.min(8 - offset);
            let byte = self.data.get((self.pos >> 3) as usize).copied();
            let byte = byte.ok_or_else(|| self.error(DwgErrorKind::Truncated))?;
            let chunk = (u32::from(byte) >> (8 - offset - take)) & ((1 << take) - 1);
            value = (value << take) | u64::from(chunk);
            self.pos += u64::from(take);
            left -= take;
        }
        Ok(value)
    }

    pub fn skip(&mut self, n: u64) -> Result<(), DwgError> {
        if n > self.remaining() {
            return Err(self.error(DwgErrorKind::Truncated));
        }
        self.pos += n;
        Ok(())
    }

    /// B: one bit.
    pub fn bit(&mut self) -> Result<bool, DwgError> {
        self.bits(1).map(|b| b == 1)
    }

    /// BB: a two-bit code.
    pub fn bb(&mut self) -> Result<u8, DwgError> {
        self.bits(2).map(|b| b as u8)
    }

    /// 3B: one to three bits, stopping at the first zero (§2.1).
    pub fn three_b(&mut self) -> Result<u8, DwgError> {
        let mut value = 0;
        for _ in 0..3 {
            value = (value << 1) | self.bits(1)? as u8;
            if value & 1 == 0 {
                break;
            }
        }
        Ok(value)
    }

    pub fn rc(&mut self) -> Result<u8, DwgError> {
        self.bits(8).map(|b| b as u8)
    }

    pub fn rs(&mut self) -> Result<u16, DwgError> {
        Ok(u16::from_le_bytes([self.rc()?, self.rc()?]))
    }

    pub fn rl(&mut self) -> Result<u32, DwgError> {
        Ok(u32::from_le_bytes([
            self.rc()?,
            self.rc()?,
            self.rc()?,
            self.rc()?,
        ]))
    }

    pub fn rd(&mut self) -> Result<f64, DwgError> {
        let mut raw = [0u8; 8];
        for byte in &mut raw {
            *byte = self.rc()?;
        }
        Ok(f64::from_le_bytes(raw))
    }

    pub fn rd2(&mut self) -> Result<[f64; 2], DwgError> {
        Ok([self.rd()?, self.rd()?])
    }

    pub fn rd3(&mut self) -> Result<[f64; 3], DwgError> {
        Ok([self.rd()?, self.rd()?, self.rd()?])
    }

    /// BS: a bitshort (§2.2).
    pub fn bs(&mut self) -> Result<u16, DwgError> {
        match self.bb()? {
            0 => self.rs(),
            1 => self.rc().map(u16::from),
            2 => Ok(0),
            _ => Ok(256),
        }
    }

    /// BL: a bitlong (§2.3). Code 3 is unused.
    pub fn bl(&mut self) -> Result<u32, DwgError> {
        match self.bb()? {
            0 => self.rl(),
            1 => self.rc().map(u32::from),
            2 => Ok(0),
            _ => Err(self.error(DwgErrorKind::InvalidObject)),
        }
    }

    /// BLL: a bitlonglong, a byte count then that many bytes (§2.4). The
    /// specification says the count is a `3B` code (one to three bits), but the
    /// R2013 and R2018 headers, whose first field is a zero BLL, have three bits
    /// there, so the count is a plain 3-bit number.
    pub fn bll(&mut self) -> Result<u64, DwgError> {
        let count = self.bits(3)?;
        let mut value = 0u64;
        for i in 0..count {
            value |= u64::from(self.rc()?) << (8 * i);
        }
        Ok(value)
    }

    /// BD: a bitdouble (§2.5). Code 3 is unused.
    pub fn bd(&mut self) -> Result<f64, DwgError> {
        match self.bb()? {
            0 => self.rd(),
            1 => Ok(1.0),
            2 => Ok(0.0),
            _ => Err(self.error(DwgErrorKind::InvalidObject)),
        }
    }

    pub fn bd2(&mut self) -> Result<[f64; 2], DwgError> {
        Ok([self.bd()?, self.bd()?])
    }

    pub fn bd3(&mut self) -> Result<[f64; 3], DwgError> {
        Ok([self.bd()?, self.bd()?, self.bd()?])
    }

    /// DD: a bitdouble with default (§2.9): zero, four or six bytes patched
    /// into `default`, or a full double.
    pub fn dd(&mut self, default: f64) -> Result<f64, DwgError> {
        let mut raw = default.to_le_bytes();
        match self.bb()? {
            0 => return Ok(default),
            1 => {
                for byte in raw.iter_mut().take(4) {
                    *byte = self.rc()?;
                }
            }
            2 => {
                raw[4] = self.rc()?;
                raw[5] = self.rc()?;
                for byte in raw.iter_mut().take(4) {
                    *byte = self.rc()?;
                }
            }
            _ => return self.rd(),
        }
        Ok(f64::from_le_bytes(raw))
    }

    /// BE: a bit extrusion (§2.8, R2000+): one bit for (0, 0, 1), else 3BD.
    pub fn extrusion(&mut self) -> Result<[f64; 3], DwgError> {
        if self.bit()? {
            Ok([0.0, 0.0, 1.0])
        } else {
            self.bd3()
        }
    }

    /// BT: a bit thickness (§2.10, R2000+): one bit for zero, else a BD.
    pub fn thickness(&mut self) -> Result<f64, DwgError> {
        if self.bit()? {
            Ok(0.0)
        } else {
            self.bd()
        }
    }

    /// OT: an object type (§2.12, R2010+).
    pub fn object_type(&mut self) -> Result<u32, DwgError> {
        match self.bb()? {
            0 => self.rc().map(u32::from),
            1 => self.rc().map(|b| u32::from(b) + 0x1F0),
            _ => self.rs().map(u32::from),
        }
    }

    /// TU: UTF-16 text, a BS count of code units then two bytes each. The
    /// count includes the terminating zero, which is dropped. An unpaired
    /// surrogate is an error, not a replacement character.
    pub fn text(&mut self, max_units: u64) -> Result<String, DwgError> {
        let count = u64::from(self.bs()?);
        if count > max_units {
            let kind = DwgErrorKind::LimitExceeded {
                limit: "string length",
                value: count,
            };
            return Err(self.error(kind));
        }
        if count * 16 > self.remaining() {
            return Err(self.error(DwgErrorKind::Truncated));
        }
        let mut units = Vec::with_capacity(count as usize);
        for _ in 0..count {
            units.push(self.rs()?);
        }
        if units.last() == Some(&0) {
            units.pop();
        }
        String::from_utf16(&units).map_err(|_| self.error(DwgErrorKind::InvalidText))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reader(data: &[u8]) -> BitReader<'_> {
        BitReader::new(data, DwgStage::Object)
    }

    /// Packs a string of '0' and '1' (spaces ignored) into bytes, zero padded.
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

    #[test]
    fn bits_cross_byte_boundaries() {
        let data = [0b1010_1100, 0b0101_0011];
        let mut r = reader(&data);
        assert_eq!(r.bits(3), Ok(0b101));
        assert_eq!(r.bits(7), Ok(0b011_0001));
        assert_eq!(r.bits(6), Ok(0b01_0011));
        assert_eq!(r.remaining(), 0);
        assert_eq!(r.bits(1).unwrap_err().kind, DwgErrorKind::Truncated);
        assert_eq!(reader(&data).bits(16), Ok(0xAC53));
        assert_eq!(reader(&[0xFF; 9]).bits(64), Ok(u64::MAX));
    }

    #[test]
    fn three_b_reads_one_to_three_bits() {
        let data = pack("0 10 110 111");
        let mut r = reader(&data);
        assert_eq!(r.three_b(), Ok(0));
        assert_eq!(r.three_b(), Ok(0b10));
        assert_eq!(r.three_b(), Ok(0b110));
        assert_eq!(r.three_b(), Ok(0b111));
    }

    #[test]
    fn bitshorts_follow_the_specification_example() {
        // §2.2: five shorts from one stream.
        let data = pack("0000000001000000011011010000111110");
        let mut r = reader(&data);
        assert_eq!(r.bs(), Ok(257));
        assert_eq!(r.bs(), Ok(0));
        assert_eq!(r.bs(), Ok(256));
        assert_eq!(r.bs(), Ok(15));
        assert_eq!(r.bs(), Ok(0));
    }

    #[test]
    fn negative_bitshorts_use_the_long_form() {
        let data = pack("00 11111111 11111111");
        assert_eq!(reader(&data).bs(), Ok(0xFFFF));
    }

    #[test]
    fn bitlongs_follow_the_specification_example() {
        // §2.3: 257, 0, 15, 0.
        let data = pack("000000000100000001000000000000000010010000111110");
        let mut r = reader(&data);
        assert_eq!(r.bl(), Ok(257));
        assert_eq!(r.bl(), Ok(0));
        assert_eq!(r.bl(), Ok(15));
        assert_eq!(r.bl(), Ok(0));
        assert_eq!(
            reader(&pack("11")).bl().unwrap_err().kind,
            DwgErrorKind::InvalidObject
        );
    }

    #[test]
    fn bitlonglong_has_a_counted_length() {
        // Length 2 (three bits, 010), then bytes 0x34 0x12.
        let data = pack("010 00110100 00010010");
        assert_eq!(reader(&data).bll(), Ok(0x1234));
        assert_eq!(reader(&pack("000")).bll(), Ok(0));
        assert_eq!(
            reader(&pack("111")).bll().unwrap_err().kind,
            DwgErrorKind::Truncated
        );
    }

    #[test]
    fn bitdoubles() {
        let one = pack("01");
        assert_eq!(reader(&one).bd(), Ok(1.0));
        assert_eq!(reader(&pack("10")).bd(), Ok(0.0));
        let mut bits = String::from("00");
        for byte in 2.5f64.to_le_bytes() {
            bits.push_str(&format!("{byte:08b}"));
        }
        assert_eq!(reader(&pack(&bits)).bd(), Ok(2.5));
        assert!(reader(&pack("11")).bd().is_err());
    }

    #[test]
    fn doubles_with_default_patch_the_documented_bytes() {
        let default = 1.0f64;
        assert_eq!(reader(&pack("00")).dd(default), Ok(1.0));
        // Opcode 01: four bytes replace the first four of the default.
        let bytes = default.to_le_bytes();
        let mut want = bytes;
        want[..4].copy_from_slice(&[1, 2, 3, 4]);
        let bits = format!("01 {}", [1u8, 2, 3, 4].map(|b| format!("{b:08b}")).join(""));
        assert_eq!(
            reader(&pack(&bits)).dd(default),
            Ok(f64::from_le_bytes(want))
        );
        // Opcode 10: two bytes go to positions 4 and 5, then four to the front.
        let mut want = bytes;
        want[4] = 9;
        want[5] = 8;
        want[..4].copy_from_slice(&[1, 2, 3, 4]);
        let bits = format!(
            "10 {}",
            [9u8, 8, 1, 2, 3, 4].map(|b| format!("{b:08b}")).join("")
        );
        assert_eq!(
            reader(&pack(&bits)).dd(default),
            Ok(f64::from_le_bytes(want))
        );
    }

    #[test]
    fn extrusion_and_thickness() {
        assert_eq!(reader(&pack("1")).extrusion(), Ok([0.0, 0.0, 1.0]));
        assert_eq!(reader(&pack("0 01 01 10")).extrusion(), Ok([1.0, 1.0, 0.0]));
        assert_eq!(reader(&pack("1")).thickness(), Ok(0.0));
        assert_eq!(reader(&pack("0 01")).thickness(), Ok(1.0));
    }

    #[test]
    fn object_types_use_the_r2010_encoding() {
        assert_eq!(reader(&pack("00 00010011")).object_type(), Ok(0x13));
        assert_eq!(reader(&pack("01 00000010")).object_type(), Ok(0x1F2));
        assert_eq!(
            reader(&pack("10 00000101 00000001")).object_type(),
            Ok(0x105)
        );
        assert_eq!(
            reader(&pack("11 00000101 00000001")).object_type(),
            Ok(0x105)
        );
    }

    fn utf16_bits(units: &[u16]) -> String {
        let count = format!(
            "00 {:08b}{:08b}",
            units.len() as u8,
            (units.len() >> 8) as u8
        );
        let body: String = units
            .iter()
            .map(|u| format!("{:08b}{:08b}", *u as u8, (*u >> 8) as u8))
            .collect();
        format!("{count}{body}")
    }

    #[test]
    fn text_is_utf16_with_a_terminator_and_surrogate_pairs() {
        let mut units: Vec<u16> = "Ab€😀".encode_utf16().collect();
        units.push(0);
        assert_eq!(
            reader(&pack(&utf16_bits(&units))).text(100),
            Ok("Ab€😀".to_owned())
        );
        assert_eq!(reader(&pack("10")).text(100), Ok(String::new()));
    }

    #[test]
    fn bad_text_is_an_error() {
        let lone = pack(&utf16_bits(&[0x41, 0xD800, 0]));
        assert_eq!(
            reader(&lone).text(100).unwrap_err().kind,
            DwgErrorKind::InvalidText
        );
        let long = pack(&utf16_bits(&[0x41; 10]));
        assert!(matches!(
            reader(&long).text(5).unwrap_err().kind,
            DwgErrorKind::LimitExceeded { .. }
        ));
        // Declares four units but holds one.
        let short = pack(&format!("00 {:08b}00000000 {:016b}", 4, 0x41));
        assert_eq!(
            reader(&short).text(100).unwrap_err().kind,
            DwgErrorKind::Truncated
        );
    }

    #[test]
    fn windows_stay_inside_their_parent() {
        let data = [0xAB, 0xCD, 0xEF];
        let r = reader(&data);
        let mut w = r.window(4, 12).unwrap();
        assert_eq!(w.bits(8), Ok(0xBC));
        assert!(w.bits(1).is_err());
        assert!(r.window(8, 25).is_err());
        assert!(r.window(9, 8).is_err());
        assert!(w.window(0, 20).is_err());
    }
}
