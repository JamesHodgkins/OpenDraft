//! The two checksums of the R2004-family container (Open Design Specification
//! §4.2 for the page checksum, §2.14.2 for the 32-bit CRC).

/// The page checksum (an Adler-32 variant with a different modulus and a
/// caller-chosen seed). Page headers chain it: the result for one block seeds
/// the next.
pub(crate) fn page_checksum(seed: u32, data: &[u8]) -> u32 {
    let mut sum1 = seed & 0xFFFF;
    let mut sum2 = seed >> 16;
    for chunk in data.chunks(0x15B0) {
        for &byte in chunk {
            sum1 += u32::from(byte);
            sum2 += sum1;
        }
        sum1 %= 0xFFF1;
        sum2 %= 0xFFF1;
    }
    (sum2 << 16) | (sum1 & 0xFFFF)
}

fn crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    for (i, slot) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        *slot = c;
    }
    table
}

/// The 32-bit CRC of the file header block: the standard reflected CRC-32
/// (polynomial 0xEDB88320), continuing from `seed`.
pub(crate) fn crc32(seed: u32, data: &[u8]) -> u32 {
    let table = crc_table();
    let mut crc = !seed;
    for &byte in data {
        let index = usize::from((crc as u8) ^ byte);
        crc = (crc >> 8) ^ table.get(index).copied().unwrap_or(0);
    }
    !crc
}

const fn crc16_table() -> [u16; 256] {
    let mut table = [0u16; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u16;
        let mut bit = 0;
        while bit < 8 {
            c = if c & 1 != 0 {
                0xA001 ^ (c >> 1)
            } else {
                c >> 1
            };
            bit += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
}

static CRC16_TABLE: [u16; 256] = crc16_table();

/// The 16-bit CRC of the object, class and handle-map blocks (§2.14.1): a
/// reflected CRC-16, polynomial 0xA001, continuing from `seed`. Those blocks
/// all start from 0xC0C1.
pub(crate) fn crc16(seed: u16, data: &[u8]) -> u16 {
    let mut crc = seed;
    for &byte in data {
        let index = usize::from((crc as u8) ^ byte);
        crc = (crc >> 8) ^ CRC16_TABLE.get(index).copied().unwrap_or(0);
    }
    crc
}

/// Where the CRC of every object, class table and handle-map block starts.
pub(crate) const CRC16_SEED: u16 = 0xC0C1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(0, b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(0, b""), 0);
    }

    #[test]
    fn crc16_matches_the_documented_table_and_the_standard_check_value() {
        // The specification prints the table; its second and third entries.
        assert_eq!(CRC16_TABLE[1], 0xC0C1);
        assert_eq!(CRC16_TABLE[2], 0xC181);
        assert_eq!(CRC16_TABLE[255], 0x4040);
        // CRC-16/ARC ("123456789" -> 0xBB3D), the same polynomial and reflection.
        assert_eq!(crc16(0, b"123456789"), 0xBB3D);
        assert_eq!(crc16(crc16(0, b"1234"), b"56789"), 0xBB3D);
    }

    #[test]
    fn crc32_continues_from_a_seed() {
        let whole = crc32(0, b"123456789");
        assert_eq!(crc32(crc32(0, b"1234"), b"56789"), whole);
    }

    #[test]
    fn page_checksum_known_values() {
        // sum1 = 1, 3, 6 and sum2 = 1, 4, 10, worked by hand.
        assert_eq!(page_checksum(0, &[1, 2, 3]), 0x000A_0006);
        assert_eq!(page_checksum(7, &[]), 7);
    }

    #[test]
    fn page_checksum_reduces_modulo_65521_between_chunks() {
        let data = vec![0xFF; 0x15B0 * 3 + 5];
        let mut sum1: u64 = 0;
        let mut sum2: u64 = 0;
        for (i, b) in data.iter().enumerate() {
            sum1 += u64::from(*b);
            sum2 += sum1;
            if (i + 1) % 0x15B0 == 0 || i + 1 == data.len() {
                sum1 %= 0xFFF1;
                sum2 %= 0xFFF1;
            }
        }
        assert_eq!(u64::from(page_checksum(0, &data)), (sum2 << 16) | sum1);
    }
}
