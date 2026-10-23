/// The automation API major version this build speaks. Independent of the
/// MCP protocol version. A changed meaning or a removed field bumps it;
/// an added optional field does not.
pub const API_VERSION: u32 = 1;

/// The highest version both sides support, or `None` if their ranges do not
/// overlap (the caller then reports an upgrade message).
pub fn negotiate(ours: &[u32], theirs: &[u32]) -> Option<u32> {
    ours.iter().copied().filter(|v| theirs.contains(v)).max()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_highest_common_version() {
        assert_eq!(negotiate(&[1, 2], &[2, 3]), Some(2));
        assert_eq!(negotiate(&[1], &[1]), Some(1));
        assert_eq!(negotiate(&[1], &[2]), None);
        assert_eq!(negotiate(&[], &[1]), None);
    }
}
