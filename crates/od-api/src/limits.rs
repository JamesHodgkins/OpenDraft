//! Named limits (spec section 7.1). Errors quote the limit that was hit.

pub const IPC_FRAME_BYTES: usize = 8 * 1024 * 1024;
pub const MCP_REQUEST_BYTES: usize = 4 * 1024 * 1024;
pub const QUERY_PAGE_DEFAULT: usize = 100;
pub const QUERY_PAGE_MAX: usize = 1_000;
pub const GET_IDS_MAX: usize = 1_000;
pub const MEASURE_IDS_MAX: usize = 1_000;
pub const INTERSECT_IDS_MAX: usize = 200;
pub const INTERSECT_PAIRS_MAX: usize = 10_000;
pub const INTERSECT_POINTS_MAX: usize = 10_000;
pub const BATCH_OPERATIONS_MAX: usize = 1_000;
pub const POINTS_PER_ENTITY_MAX: usize = 100_000;
pub const POINTS_PER_BATCH_MAX: usize = 50_000;
pub const KNOTS_PER_BATCH_MAX: usize = 100_000;
pub const RENDER_ENTITIES_MAX: usize = 5_000;
pub const RENDER_SIDE_MAX: u32 = 4_096;
/// Pixels in a PNG. A picture is time on the UI thread (about 35 ms at this
/// size in a release build), and a model downscales a larger one anyway.
pub const RENDER_PIXELS_MAX: u64 = 4_000_000;
pub const RENDER_RESULT_BYTES: usize = 6 * 1024 * 1024;
pub const VECTOR_VERTICES_MAX: usize = 500_000;
pub const STRING_BYTES_MAX: usize = 64 * 1024;
pub const QUEUED_REQUESTS_MAX: usize = 64;
/// Longest prompt a caller can put in front of the user.
pub const SELECTION_PROMPT_MAX: usize = 200;
/// How long a selection request waits for the user, by default and at most.
pub const SELECTION_WAIT_DEFAULT_S: u32 = 300;
pub const SELECTION_WAIT_MAX_S: u32 = 900;
/// The most objects one audit compares for duplicates and overlaps, and the
/// most pairs it tests: few enough to answer within a frame or two.
pub const AUDIT_OBJECTS_MAX: usize = 20_000;
pub const AUDIT_COMPARISONS_MAX: u64 = 1_000_000;
/// Ids, names and texts listed by default and at most.
pub const LISTED_DEFAULT: usize = 30;
pub const LISTED_MAX: usize = 200;
/// Most layer names an audit is given to check against.
pub const ALLOWED_LAYERS_MAX: usize = 100;
/// Cells along each side of the summary's density grid.
pub const DENSITY_CELLS: usize = 8;
/// Longest a caller may be asked to wait inside one poll of an operation.
pub const OPERATION_WAIT_MAX_S: u32 = 30;
pub const REQUEST_DEADLINE_MS: u64 = 30_000;

/// The page size to use for a caller's `limit`: the default when absent,
/// never above the maximum, never zero.
pub fn page_size(requested: Option<usize>) -> usize {
    requested
        .unwrap_or(QUERY_PAGE_DEFAULT)
        .clamp(1, QUERY_PAGE_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_size_is_bounded() {
        assert_eq!(page_size(None), 100);
        assert_eq!(page_size(Some(0)), 1);
        assert_eq!(page_size(Some(5)), 5);
        assert_eq!(page_size(Some(1_000_000)), QUERY_PAGE_MAX);
    }
}
