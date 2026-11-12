//! The bounds that keep hostile or damaged contents from using unbounded
//! time or memory. Checked before anything is reserved.

/// Caps on one `MText`'s contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextLimits {
    pub max_content_bytes: usize,
    pub max_nodes: usize,
    pub max_paragraphs: usize,
    /// Nested `{ ... }` groups.
    pub max_group_depth: usize,
    /// Longest payload of a numeric control such as `\H2.5;`.
    pub max_numeric_payload: usize,
    /// Longest payload of any other control (a font name, a stack).
    pub max_control_payload: usize,
}

impl Default for TextLimits {
    fn default() -> Self {
        Self {
            max_content_bytes: 1 << 20,
            max_nodes: 100_000,
            max_paragraphs: 100_000,
            max_group_depth: 8,
            max_numeric_payload: 64,
            max_control_payload: 1024,
        }
    }
}
