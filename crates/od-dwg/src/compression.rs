//! The R2004-family page decompressor, a variant of LZ77 (Open Design
//! Specification §4.7). It reads from one input slice and writes into a
//! caller-sized output slice, so a hostile stream cannot make it allocate.

use crate::error::{DwgError, DwgErrorKind, DwgStage};

fn invalid(at: usize) -> DwgError {
    DwgError::new(DwgErrorKind::InvalidCompression, DwgStage::Decompress).at(at as u64)
}

struct Input<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Input<'a> {
    fn byte(&mut self) -> Result<u8, DwgError> {
        let b = self
            .data
            .get(self.pos)
            .copied()
            .ok_or_else(|| invalid(self.pos))?;
        self.pos += 1;
        Ok(b)
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], DwgError> {
        let bytes = self
            .pos
            .checked_add(n)
            .and_then(|end| self.data.get(self.pos..end))
            .ok_or_else(|| invalid(self.pos))?;
        self.pos += n;
        Ok(bytes)
    }

    /// A literal run length. A byte with a high nibble is not a length: the
    /// run is empty and the byte is the next opcode, which is returned too.
    fn literal_length(&mut self) -> Result<(usize, Option<u8>), DwgError> {
        let first = self.byte()?;
        match first {
            0 => {
                let mut total = 0x0F_usize;
                loop {
                    match self.byte()? {
                        0 => total = total.checked_add(0xFF).ok_or_else(|| invalid(self.pos))?,
                        b => {
                            total += usize::from(b);
                            return Ok((total + 3, None));
                        }
                    }
                }
            }
            1..=0x0F => Ok((usize::from(first) + 3, None)),
            _ => Ok((0, Some(first))),
        }
    }

    /// A match length that did not fit in the opcode.
    fn long_length(&mut self) -> Result<usize, DwgError> {
        let first = self.byte()?;
        if first != 0 {
            return Ok(usize::from(first));
        }
        let mut total = 0xFF_usize;
        loop {
            match self.byte()? {
                0 => total = total.checked_add(0xFF).ok_or_else(|| invalid(self.pos))?,
                b => return Ok(total + usize::from(b)),
            }
        }
    }

    /// A back-reference offset in 14 bits plus 0 to 3 trailing literals.
    fn two_byte_offset(&mut self) -> Result<(usize, usize), DwgError> {
        let first = usize::from(self.byte()?);
        let second = usize::from(self.byte()?);
        Ok(((first >> 2) | (second << 6), first & 3))
    }
}

/// Decompresses `input` into `out` and returns how many bytes it wrote. The
/// stream must end with its terminator opcode, followed by nothing but zero
/// bytes, and must not write past `out` or refer to data before the start.
pub(crate) fn decompress(input: &[u8], out: &mut [u8]) -> Result<usize, DwgError> {
    let mut r = Input {
        data: input,
        pos: 0,
    };
    let mut w = 0usize;
    let (literals, mut pending) = r.literal_length()?;
    copy_literals(&mut r, out, &mut w, literals)?;

    loop {
        let opcode = match pending.take() {
            Some(op) => op,
            None => r.byte()?,
        };
        let (length, distance, mut literals) = match opcode {
            0x00..=0x0F => return Err(invalid(r.pos)),
            0x10 => {
                let length = r.long_length()? + 9;
                let (offset, lit) = r.two_byte_offset()?;
                (length, offset + 0x3FFF + 1, lit)
            }
            0x11 => break,
            0x12..=0x1F => {
                let (offset, lit) = r.two_byte_offset()?;
                (usize::from(opcode & 0x0F) + 2, offset + 0x3FFF + 1, lit)
            }
            0x20 => {
                let length = r.long_length()? + 0x21;
                let (offset, lit) = r.two_byte_offset()?;
                (length, offset + 1, lit)
            }
            0x21..=0x3F => {
                let (offset, lit) = r.two_byte_offset()?;
                (usize::from(opcode) - 0x1E, offset + 1, lit)
            }
            0x40..=0xFF => {
                let second = usize::from(r.byte()?);
                let offset = (second << 2) | usize::from((opcode & 0x0C) >> 2);
                (
                    usize::from((opcode & 0xF0) >> 4) - 1,
                    offset + 1,
                    usize::from(opcode & 3),
                )
            }
        };
        copy_match(out, &mut w, distance, length, r.pos)?;
        if literals == 0 {
            (literals, pending) = r.literal_length()?;
        }
        copy_literals(&mut r, out, &mut w, literals)?;
    }

    // Real streams end the terminator with two zero bytes the specification
    // does not mention; anything else after it is not part of the stream.
    if input
        .get(r.pos..)
        .is_none_or(|rest| rest.iter().any(|&b| b != 0))
    {
        return Err(invalid(r.pos));
    }
    Ok(w)
}

fn copy_literals(r: &mut Input, out: &mut [u8], w: &mut usize, n: usize) -> Result<(), DwgError> {
    let src = r.take(n)?;
    let end = w.checked_add(n).ok_or_else(|| invalid(r.pos))?;
    out.get_mut(*w..end)
        .ok_or_else(|| invalid(r.pos))?
        .copy_from_slice(src);
    *w = end;
    Ok(())
}

/// Copies `length` bytes from `distance` bytes back. The ranges may overlap,
/// which repeats a short pattern, so an overlapping copy goes byte by byte.
fn copy_match(
    out: &mut [u8],
    w: &mut usize,
    distance: usize,
    length: usize,
    at: usize,
) -> Result<(), DwgError> {
    let end = w
        .checked_add(length)
        .filter(|&end| end <= out.len())
        .ok_or_else(|| invalid(at))?;
    let start = w.checked_sub(distance).ok_or_else(|| invalid(at))?;
    if distance >= length {
        out.copy_within(start..start + length, *w);
    } else {
        for i in 0..length {
            out[*w + i] = out[start + i];
        }
    }
    *w = end;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(input: &[u8], capacity: usize) -> Result<Vec<u8>, DwgError> {
        let mut out = vec![0; capacity];
        let n = decompress(input, &mut out)?;
        out.truncate(n);
        Ok(out)
    }

    #[test]
    fn literals_only() {
        // Length byte 0x02 is 2 + 3 = 5 literals.
        assert_eq!(
            run(&[0x02, b'h', b'e', b'l', b'l', b'o', 0x11], 16).unwrap(),
            b"hello"
        );
    }

    #[test]
    fn long_literal_run() {
        // 0x00 0x02 is 0x0F + 2 + 3 = 20.
        let mut stream = vec![0x00, 0x02];
        stream.extend(1..=20);
        stream.push(0x11);
        assert_eq!(run(&stream, 32).unwrap(), (1..=20).collect::<Vec<u8>>());
    }

    #[test]
    fn short_opcode_overlapping_copy_then_literal() {
        // "abcd", then opcode 0x51: copy 4 from distance 1 (repeats 'd'), 1 literal.
        let stream = [0x01, b'a', b'b', b'c', b'd', 0x51, 0x00, b'Z', 0x11];
        assert_eq!(run(&stream, 16).unwrap(), b"abcdddddZ");
    }

    #[test]
    fn short_opcode_with_a_literal_length_following() {
        // Opcode 0x50 has no inline literals, so a length byte follows (0x01 = 4).
        let stream = [
            0x01, b'a', b'b', b'c', b'd', 0x50, 0x03, 0x01, b'w', b'x', b'y', b'z', 0x11,
        ];
        // Offset (3 << 2) + 1 = 13 is further back than 4 bytes: invalid.
        assert!(run(&stream, 32).is_err());
        let stream = [
            0x01, b'a', b'b', b'c', b'd', 0x50, 0x00, 0x01, b'w', b'x', b'y', b'z', 0x11,
        ];
        assert_eq!(run(&stream, 32).unwrap(), b"abcdddddwxyz");
    }

    #[test]
    fn medium_opcode_copies_from_further_back() {
        // Opcode 0x25 = copy 7 (0x25 - 0x1E), two-byte offset 0x04 0x00 = 1 -> distance 2, 1 literal.
        let stream = [0x01, b'a', b'b', b'c', b'd', 0x25, 0x05, 0x00, b'!', 0x11];
        // first byte 0x05: offset 1, literals 1.
        assert_eq!(run(&stream, 32).unwrap(), b"abcdcdcdcdc!");
    }

    #[test]
    fn long_match_opcodes() {
        // 0x20: length from a long count + 0x21, then a two-byte offset (0 -> distance 1).
        let stream = [
            0x01, b'q', b'q', b'q', b'q', 0x20, 0x01, 0x00, 0x00, 0x01, b'a', b'b', b'c', b'd',
            0x11,
        ];
        // 0x01 + 0x21 = 34 copies of 'q', then (lit 0) a length byte 0x01 = 4 literals.
        let mut want = vec![b'q'; 4 + 34];
        want.extend(b"abcd");
        assert_eq!(run(&stream, 64).unwrap(), want);
        // 0x10: length from a long count + 9, distance offset + 0x4000.
        let mut stream = vec![0x00, 0xFF]; // 0x0F + 0xFF + 3 = 0x111 literals
        stream.extend((0..0x111).map(|i| i as u8));
        stream.extend([0x10, 0x01, 0x00, 0x00, 0x01, b'1', b'2', b'3', b'4', 0x11]);
        assert!(
            run(&stream, 1024).is_err(),
            "distance 0x4000 reaches before the start"
        );
    }

    #[test]
    fn zero_bytes_may_follow_the_terminator() {
        assert_eq!(
            run(&[0x01, 1, 2, 3, 4, 0x11, 0x00, 0x00], 8).unwrap(),
            [1, 2, 3, 4]
        );
    }

    #[test]
    fn rejects_bad_streams() {
        assert!(run(&[], 8).is_err(), "no length byte");
        assert!(
            run(&[0x02, b'h', b'i'], 8).is_err(),
            "literals run past the input"
        );
        assert!(run(&[0x01, 1, 2, 3, 4], 8).is_err(), "no terminator");
        assert!(
            run(&[0x01, 1, 2, 3, 4, 0x11, 0x00, 0x07], 8).is_err(),
            "trailing junk"
        );
        assert!(
            run(&[0x01, 1, 2, 3, 4, 0x05, 0x11], 8).is_err(),
            "opcode 0x05 is not used"
        );
        assert!(
            run(&[0x01, 1, 2, 3, 4, 0x51, 0x04, 9, 0x11], 16).is_err(),
            "before the start"
        );
        assert!(
            run(&[0x01, 1, 2, 3, 4, 0x11], 3).is_err(),
            "output too small"
        );
        assert!(
            run(&[0x01, 1, 2, 3, 4, 0x51, 0x00, 9, 0x11], 6).is_err(),
            "match overruns output"
        );
    }

    #[test]
    fn a_zero_length_run_defers_to_the_next_opcode() {
        // 0xF0 as the first byte means no literals, and is itself the opcode:
        // 0xF1 = copy 14 from distance 1 with 1 literal -- but nothing precedes it.
        assert!(run(&[0xF1, 0x00, b'x', 0x11], 32).is_err());
    }
}
