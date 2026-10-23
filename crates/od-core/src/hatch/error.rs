//! Why a hatch cannot be checked or drawn.

use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum HatchError {
    /// A number is not finite, or is beyond what the maths can handle.
    NonFinite,
    /// More loops, edges, lines or points than a hatch may have.
    TooLarge(&'static str),
    /// Loop `n` (from 0) does not end where it began.
    OpenLoop(usize),
    /// Loop `n` is an edge or polyline that could not exist.
    BadLoop(usize, &'static str),
    /// A pattern line whose copies sit on top of each other.
    PatternSpacing,
    /// A pattern line whose dashes add up to nothing.
    PatternDashes,
    /// The pattern would take more lines or strokes than the limits allow.
    TooDense { estimated: u64, limit: u64 },
    /// A transform that a hatch cannot follow exactly.
    Unsupported(&'static str),
}

impl fmt::Display for HatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite => write!(f, "a number in the hatch is not usable"),
            Self::TooLarge(what) => write!(f, "the hatch has too many {what}"),
            Self::OpenLoop(i) => write!(f, "boundary {} is not closed", i + 1),
            Self::BadLoop(i, why) => write!(f, "boundary {} is not valid: {why}", i + 1),
            Self::PatternSpacing => write!(f, "a pattern line has no spacing"),
            Self::PatternDashes => write!(f, "a pattern line has no dash length"),
            Self::TooDense { estimated, limit } => write!(
                f,
                "the pattern is too dense: about {estimated} lines against a limit of {limit}"
            ),
            Self::Unsupported(what) => write!(f, "a hatch cannot {what}"),
        }
    }
}

impl std::error::Error for HatchError {}
