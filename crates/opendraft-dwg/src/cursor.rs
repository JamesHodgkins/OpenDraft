use crate::error::{DwgError, DwgErrorKind, DwgStage};

/// A bounded little-endian reader over a byte slice. It never reads past the
/// slice it was given; a short read is a `Truncated` error.
pub(crate) struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
    stage: DwgStage,
}

impl<'a> Cursor<'a> {
    pub fn new(data: &'a [u8], stage: DwgStage) -> Self {
        Cursor {
            data,
            pos: 0,
            stage,
        }
    }

    pub fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    /// How many bytes have been read.
    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8], DwgError> {
        let end = self.pos.checked_add(n);
        match end.and_then(|end| self.data.get(self.pos..end)) {
            Some(bytes) => {
                self.pos += n;
                Ok(bytes)
            }
            None => Err(DwgError::new(DwgErrorKind::Truncated, self.stage).at(self.pos as u64)),
        }
    }

    pub fn skip(&mut self, n: usize) -> Result<(), DwgError> {
        self.take(n).map(|_| ())
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], DwgError> {
        let at = self.pos as u64;
        self.take(N)?
            .try_into()
            .map_err(|_| DwgError::new(DwgErrorKind::Truncated, self.stage).at(at))
    }

    pub fn u16(&mut self) -> Result<u16, DwgError> {
        self.array().map(u16::from_le_bytes)
    }

    pub fn u16_be(&mut self) -> Result<u16, DwgError> {
        self.array().map(u16::from_be_bytes)
    }

    pub fn u8(&mut self) -> Result<u8, DwgError> {
        self.array::<1>().map(|[b]| b)
    }

    /// A modular char (§2.6): 7 bits per byte, least significant first, the
    /// high bit meaning "another byte follows". With `signed`, bit 0x40 of
    /// the last byte means negative and is not part of the value.
    pub fn modular_char(&mut self, signed: bool) -> Result<i64, DwgError> {
        let at = self.pos as u64;
        let mut value: i64 = 0;
        for shift in (0..8).map(|i| i * 7) {
            let byte = self.u8()?;
            if byte & 0x80 != 0 {
                value |= i64::from(byte & 0x7F) << shift;
                continue;
            }
            let negative = signed && byte & 0x40 != 0;
            let payload = if signed { byte & 0x3F } else { byte & 0x7F };
            value |= i64::from(payload) << shift;
            return Ok(if negative { -value } else { value });
        }
        Err(DwgError::new(DwgErrorKind::InvalidObject, self.stage).at(at))
    }

    /// A modular short (§2.7): 15 bits per short, high bit meaning "another
    /// short follows", at most two shorts.
    pub fn modular_short(&mut self) -> Result<u32, DwgError> {
        let low = self.u16()?;
        if low & 0x8000 == 0 {
            return Ok(u32::from(low));
        }
        let high = self.u16()?;
        if high & 0x8000 != 0 {
            return Err(DwgError::new(DwgErrorKind::InvalidObject, self.stage).at(self.pos as u64));
        }
        Ok(u32::from(low & 0x7FFF) | (u32::from(high) << 15))
    }

    pub fn u32(&mut self) -> Result<u32, DwgError> {
        self.array().map(u32::from_le_bytes)
    }

    pub fn i32(&mut self) -> Result<i32, DwgError> {
        self.array().map(i32::from_le_bytes)
    }

    pub fn u64(&mut self) -> Result<u64, DwgError> {
        self.array().map(u64::from_le_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_little_endian_and_tracks_remaining() {
        let data = [
            1, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF, 2, 0, 0, 0, 0, 0, 0, 0, 9,
        ];
        let mut c = Cursor::new(&data, DwgStage::FileHeader);
        assert_eq!(c.u32(), Ok(1));
        assert_eq!(c.i32(), Ok(-1));
        assert_eq!(c.u64(), Ok(2));
        assert_eq!(c.remaining(), 1);
    }

    #[test]
    fn modular_chars_follow_the_specification_examples() {
        // §2.6: 10000010 00100100 is 2 + 18 * 256.
        let mut c = Cursor::new(&[0b1000_0010, 0b0010_0100], DwgStage::ObjectMap);
        assert_eq!(c.modular_char(false), Ok(4610));
        let mut c = Cursor::new(&[0xE9, 0x97, 0xE6, 0x35], DwgStage::ObjectMap);
        assert_eq!(c.modular_char(false), Ok(112_823_273));
        // The negative example: 133 + 5 * 256 = 1413, negated.
        let mut c = Cursor::new(&[0b1000_0101, 0b0100_1011], DwgStage::ObjectMap);
        assert_eq!(c.modular_char(true), Ok(-1413));
        // Unsigned, the 0x40 bit is data.
        let mut c = Cursor::new(&[0x40], DwgStage::ObjectMap);
        assert_eq!(c.modular_char(false), Ok(0x40));
    }

    #[test]
    fn modular_chars_stop_after_eight_bytes() {
        let mut c = Cursor::new(&[0xFF; 9], DwgStage::ObjectMap);
        assert_eq!(
            c.modular_char(false).unwrap_err().kind,
            DwgErrorKind::InvalidObject
        );
        let mut c = Cursor::new(&[0x81], DwgStage::ObjectMap);
        assert_eq!(
            c.modular_char(false).unwrap_err().kind,
            DwgErrorKind::Truncated
        );
    }

    #[test]
    fn modular_shorts() {
        // §2.7: 00110001 11110100 10001101 00000000 (as bytes) is 4650033.
        let mut c = Cursor::new(&[0x31, 0xF4, 0x8D, 0x00], DwgStage::Object);
        assert_eq!(c.modular_short(), Ok(4_650_033));
        let mut c = Cursor::new(&[0x0D, 0x00], DwgStage::Object);
        assert_eq!(c.modular_short(), Ok(13));
        let mut c = Cursor::new(&[0x01, 0x80, 0x01, 0x80], DwgStage::Object);
        assert!(c.modular_short().is_err());
    }

    #[test]
    fn short_reads_fail_and_do_not_advance() {
        let mut c = Cursor::new(&[1, 2, 3], DwgStage::PageMap);
        let err = c.u32().unwrap_err();
        assert_eq!(err.kind, DwgErrorKind::Truncated);
        assert_eq!(c.remaining(), 3);
        assert!(c.take(usize::MAX).is_err());
        assert!(c.skip(3).is_ok() && c.is_empty());
    }
}
