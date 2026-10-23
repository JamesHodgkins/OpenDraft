use crate::error::{DwgError, DwgErrorKind, DwgStage};

/// Bounds on what one read may allocate or iterate. Fixed for the whole read.
/// The defaults are the specification's; tests use small ones to prove each
/// limit fails early.
#[derive(Debug, Clone)]
pub struct DwgLimits {
    pub max_file_bytes: u64,
    /// Declared bytes of every logical section read from one file, together.
    pub max_total_logical_bytes: u64,
    pub max_section_bytes: u64,
    pub max_section_pages: u64,
    /// The decompressed size of one page (an addition to the specification's list).
    pub max_page_bytes: u64,
    pub max_objects: u64,
    pub max_single_object_bytes: u64,
    pub max_string_code_units: u64,
    pub max_collection_items: u64,
    pub max_handle_bytes: u8,
    pub max_reference_depth: u32,
    pub max_warning_kinds: usize,
}

const MIB: u64 = 1 << 20;

impl Default for DwgLimits {
    fn default() -> Self {
        DwgLimits {
            max_file_bytes: 1024 * MIB,
            max_total_logical_bytes: 2048 * MIB,
            max_section_bytes: 512 * MIB,
            max_section_pages: 1_000_000,
            max_page_bytes: 16 * MIB,
            max_objects: 5_000_000,
            max_single_object_bytes: 16 * MIB,
            max_string_code_units: 8_000_000,
            max_collection_items: 10_000_000,
            max_handle_bytes: 8,
            max_reference_depth: 64,
            max_warning_kinds: 256,
        }
    }
}

/// Fails with `LimitExceeded` when `value` is over `max`.
pub(crate) fn check(
    stage: DwgStage,
    limit: &'static str,
    value: u64,
    max: u64,
) -> Result<(), DwgError> {
    if value > max {
        Err(DwgError::new(
            DwgErrorKind::LimitExceeded { limit, value },
            stage,
        ))
    } else {
        Ok(())
    }
}
