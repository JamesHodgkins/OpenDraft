//! Test support: a writer for the DWG bit codes, so the entity decoders can be
//! exercised on cases the supplied drawings do not contain. It is written from
//! the same specification as the reader, so a round trip proves they agree with
//! each other, not that either is right; the supplied drawings are the check
//! on the codes they happen to use.

use crate::bit::BitReader;
use crate::error::DwgStage;
use crate::object::ObjectStreams;

#[derive(Default)]
pub(crate) struct BitWriter {
    bits: Vec<bool>,
}

impl BitWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bits(&mut self, value: u64, n: u32) -> &mut Self {
        for i in (0..n).rev() {
            self.bits.push((value >> i) & 1 == 1);
        }
        self
    }

    pub fn bit(&mut self, v: bool) -> &mut Self {
        self.bits(u64::from(v), 1)
    }

    pub fn rc(&mut self, v: u8) -> &mut Self {
        self.bits(u64::from(v), 8)
    }

    pub fn rs(&mut self, v: u16) -> &mut Self {
        self.rc(v as u8).rc((v >> 8) as u8)
    }

    pub fn rl(&mut self, v: u32) -> &mut Self {
        self.rs(v as u16).rs((v >> 16) as u16)
    }

    pub fn rd(&mut self, v: f64) -> &mut Self {
        for b in v.to_le_bytes() {
            self.rc(b);
        }
        self
    }

    pub fn bs(&mut self, v: u16) -> &mut Self {
        match v {
            0 => self.bits(2, 2),
            256 => self.bits(3, 2),
            1..=255 => self.bits(1, 2).rc(v as u8),
            _ => self.bits(0, 2).rs(v),
        }
    }

    pub fn bl(&mut self, v: u32) -> &mut Self {
        match v {
            0 => self.bits(2, 2),
            1..=255 => self.bits(1, 2).rc(v as u8),
            _ => self.bits(0, 2).rl(v),
        }
    }

    pub fn bd(&mut self, v: f64) -> &mut Self {
        if v == 0.0 {
            self.bits(2, 2)
        } else if v == 1.0 {
            self.bits(1, 2)
        } else {
            self.bits(0, 2).rd(v)
        }
    }

    /// A DD whose value is the default (no bytes follow).
    pub fn dd_default(&mut self) -> &mut Self {
        self.bits(0, 2)
    }

    /// A DD written in full.
    pub fn dd_full(&mut self, v: f64) -> &mut Self {
        self.bits(3, 2).rd(v)
    }

    pub fn bd3(&mut self, v: [f64; 3]) -> &mut Self {
        self.bd(v[0]).bd(v[1]).bd(v[2])
    }

    pub fn rd2(&mut self, v: [f64; 2]) -> &mut Self {
        self.rd(v[0]).rd(v[1])
    }

    /// A bit extrusion.
    pub fn extrusion(&mut self, v: [f64; 3]) -> &mut Self {
        if v == [0.0, 0.0, 1.0] {
            self.bit(true)
        } else {
            self.bit(false).bd3(v)
        }
    }

    /// A bit thickness.
    pub fn thickness(&mut self, v: f64) -> &mut Self {
        if v == 0.0 {
            self.bit(true)
        } else {
            self.bit(false).bd(v)
        }
    }

    /// A handle reference with the given code and value.
    pub fn handle(&mut self, code: u8, value: u64) -> &mut Self {
        let bytes: Vec<u8> = value
            .to_be_bytes()
            .into_iter()
            .skip_while(|&b| b == 0)
            .collect();
        self.bits(u64::from(code), 4).bits(bytes.len() as u64, 4);
        for b in bytes {
            self.rc(b);
        }
        self
    }

    /// A TU string without a terminator.
    pub fn text(&mut self, s: &str) -> &mut Self {
        let units: Vec<u16> = s.encode_utf16().collect();
        self.bs(units.len() as u16);
        for u in units {
            self.rs(u);
        }
        self
    }

    pub fn finish(&self) -> Vec<u8> {
        self.bits
            .chunks(8)
            .map(|c| {
                c.iter()
                    .enumerate()
                    .fold(0u8, |acc, (i, b)| acc | (u8::from(*b) << (7 - i)))
            })
            .collect()
    }
}

/// Three separate buffers made into the streams of an object.
pub(crate) struct Buffers {
    pub data: Vec<u8>,
    pub strings: Vec<u8>,
    pub handles: Vec<u8>,
}

impl Buffers {
    pub fn new(data: &BitWriter, strings: &BitWriter, handles: &BitWriter) -> Self {
        Buffers {
            data: data.finish(),
            strings: strings.finish(),
            handles: handles.finish(),
        }
    }

    pub fn streams(&self) -> ObjectStreams<'_> {
        ObjectStreams {
            data: BitReader::new(&self.data, DwgStage::Object),
            strings: BitReader::new(&self.strings, DwgStage::Object),
            handles: BitReader::new(&self.handles, DwgStage::Object),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_written_is_read_back() {
        let mut w = BitWriter::new();
        w.bs(0)
            .bs(256)
            .bs(77)
            .bs(1000)
            .bl(0)
            .bl(9)
            .bl(70000)
            .bd(0.0)
            .bd(1.0)
            .bd(-2.5);
        w.rd(3.25)
            .bit(true)
            .extrusion([0.0, 0.0, 1.0])
            .extrusion([0.0, 0.6, 0.8]);
        w.thickness(0.0)
            .thickness(2.0)
            .handle(5, 0x5E7)
            .handle(4, 0)
            .text("Ab€");
        let bytes = w.finish();
        let mut r = BitReader::new(&bytes, DwgStage::Object);
        assert_eq!(
            [r.bs(), r.bs(), r.bs(), r.bs()],
            [Ok(0), Ok(256), Ok(77), Ok(1000)]
        );
        assert_eq!([r.bl(), r.bl(), r.bl()], [Ok(0), Ok(9), Ok(70000)]);
        assert_eq!([r.bd(), r.bd(), r.bd()], [Ok(0.0), Ok(1.0), Ok(-2.5)]);
        assert_eq!(r.rd(), Ok(3.25));
        assert_eq!(r.bit(), Ok(true));
        assert_eq!(r.extrusion(), Ok([0.0, 0.0, 1.0]));
        assert_eq!(r.extrusion(), Ok([0.0, 0.6, 0.8]));
        assert_eq!([r.thickness(), r.thickness()], [Ok(0.0), Ok(2.0)]);
        assert_eq!(r.handle(8).map(|h| (h.code, h.value)), Ok((5, 0x5E7)));
        assert_eq!(r.handle(8).map(|h| (h.code, h.value)), Ok((4, 0)));
        assert_eq!(r.text(100), Ok("Ab€".to_owned()));
    }

    #[test]
    fn defaulted_doubles() {
        let mut w = BitWriter::new();
        w.dd_default().dd_full(7.5);
        let bytes = w.finish();
        let mut r = BitReader::new(&bytes, DwgStage::Object);
        assert_eq!(r.dd(2.0), Ok(2.0));
        assert_eq!(r.dd(2.0), Ok(7.5));
    }
}
