//! The two XOR masks of the R2004-family container (Open Design Specification
//! §4.1 and §4.6). They are obfuscation, unrelated to password protection.

/// Length of the masked block in the file header, at file offset 0x80.
pub(crate) const FILE_HEADER_BLOCK: usize = 0x6C;

/// The pseudo-random sequence the file header block is XORed with. It is the
/// output of the linear congruential generator given in §4.1.
fn file_header_mask() -> impl Iterator<Item = u8> {
    let mut seed: u32 = 1;
    std::iter::repeat_with(move || {
        seed = seed.wrapping_mul(0x343FD).wrapping_add(0x269EC3);
        (seed >> 16) as u8
    })
}

/// Removes (or applies; it is its own inverse) the file header mask.
pub(crate) fn unmask_file_header(block: &mut [u8]) {
    for (byte, mask) in block.iter_mut().zip(file_header_mask()) {
        *byte ^= mask;
    }
}

/// Removes (or applies) the mask on a 32-byte data page header. `address` is
/// the header's absolute position in the file; only its low 32 bits count.
pub(crate) fn unmask_page_header(header: &mut [u8; 32], address: u64) {
    let mask = (0x4164_536B_u32 ^ address as u32).to_le_bytes();
    for (byte, m) in header.iter_mut().zip(mask.iter().cycle()) {
        *byte ^= m;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_header_mask_starts_with_the_documented_sequence() {
        let head: Vec<u8> = file_header_mask().take(16).collect();
        let documented = [
            0x29, 0x23, 0xBE, 0x84, 0xE1, 0x6C, 0xD6, 0xAE, 0x52, 0x90, 0x49, 0xF1, 0xF1, 0xBB,
            0xE9, 0xEB,
        ];
        assert_eq!(head, documented);
    }

    #[test]
    fn file_header_mask_matches_the_end_of_the_documented_table() {
        // The printed table has 92 of the 108 bytes; its last row is at 80.
        let tail: Vec<u8> = file_header_mask().take(92).skip(80).collect();
        assert_eq!(
            tail,
            [0xAD, 0x4F, 0x14, 0xF2, 0x44, 0x40, 0x66, 0xD0, 0x6B, 0xC4, 0x30, 0xB7]
        );
    }

    #[test]
    fn masks_are_their_own_inverse() {
        let mut block = [7u8; FILE_HEADER_BLOCK];
        unmask_file_header(&mut block);
        assert_ne!(block, [7u8; FILE_HEADER_BLOCK]);
        unmask_file_header(&mut block);
        assert_eq!(block, [7u8; FILE_HEADER_BLOCK]);

        let mut header = [3u8; 32];
        unmask_page_header(&mut header, 0x1234_5678_9ABC);
        unmask_page_header(&mut header, 0x1234_5678_9ABC);
        assert_eq!(header, [3u8; 32]);
    }

    #[test]
    fn page_header_mask_uses_the_low_32_bits_of_the_address() {
        let mut a = [0u8; 32];
        let mut b = [0u8; 32];
        unmask_page_header(&mut a, 0x100);
        unmask_page_header(&mut b, 0x1_0000_0100);
        assert_eq!(a, b);
        assert_eq!(a[..4], (0x4164_536B_u32 ^ 0x100).to_le_bytes());
    }
}
