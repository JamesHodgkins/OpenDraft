//! Choices the native file names instead of numbering (DXF's codes are for
//! DXF), as serde twins of plain enums.

use od_core::mtext::{LineSpacingStyle, MTextAttachment, MTextFlow};
use od_core::{AngleFormat, Arrow, LengthFormat};
use serde::{Deserialize, Serialize};

/// A serde twin of a plain enum, so the file names the choice instead of
/// numbering it.
macro_rules! mirror {
    ($dto:ident <-> $core:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub(super) enum $dto {
            $($variant),+
        }

        impl From<$core> for $dto {
            fn from(value: $core) -> Self {
                match value {
                    $($core::$variant => Self::$variant),+
                }
            }
        }

        impl From<$dto> for $core {
            fn from(value: $dto) -> Self {
                match value {
                    $($dto::$variant => Self::$variant),+
                }
            }
        }
    };
}
pub(super) use mirror;

mirror!(ArrowDto <-> Arrow { Closed, Open, Tick, None });
mirror!(LengthFormatDto <-> LengthFormat {
    Scientific,
    Decimal,
    Engineering,
    Architectural,
    Fractional,
});
mirror!(AngleFormatDto <-> AngleFormat {
    DecimalDegrees,
    DegreesMinutesSeconds,
    Gradians,
    Radians,
    Surveyor,
});
mirror!(AttachmentPointDto <-> MTextAttachment {
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    MiddleCenter,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
});
mirror!(FlowDto <-> MTextFlow { LeftToRight, TopToBottom, ByStyle });
mirror!(SpacingStyleDto <-> LineSpacingStyle { AtLeast, Exact });
