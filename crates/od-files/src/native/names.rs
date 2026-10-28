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

/// A serde enum for a setting `DimStyle` keeps as AutoCAD's small number: the
/// file names the choice, and a number outside the list cannot be written.
macro_rules! coded {
    ($dto:ident { $($variant:ident = $code:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub(super) enum $dto {
            $($variant),+
        }

        impl $dto {
            pub(super) fn from_code(code: u8) -> Option<Self> {
                match code {
                    $($code => Some(Self::$variant),)+
                    _ => None,
                }
            }

            pub(super) fn code(self) -> u8 {
                match self {
                    $(Self::$variant => $code),+
                }
            }
        }
    };
}

coded!(TextPlacementDto { Centred = 0, Above = 1, Outside = 2, Jis = 3, Below = 4 });
coded!(TextJustifyDto {
    Centred = 0,
    AtExtension1 = 1,
    AtExtension2 = 2,
    OverExtension1 = 3,
    OverExtension2 = 4,
});
coded!(FitDto { Both = 0, Arrows = 1, Text = 2, BestFit = 3 });
coded!(TextMoveDto { MoveLine = 0, Leader = 1, Free = 2 });
coded!(ArcSymbolDto { Before = 0, Above = 1, None = 2 });
coded!(ToleranceDto { None = 0, Symmetrical = 1, Deviation = 2, Limits = 3 });

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
