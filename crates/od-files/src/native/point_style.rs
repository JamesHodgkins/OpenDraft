//! How point objects are drawn (`PDMODE`, `PDSIZE`). Each is left out at its
//! default, so files from before points existed still read.

use od_core::{Document, PointStyle};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub(super) struct PointStyleDto {
    #[serde(skip_serializing_if = "is_zero_mode")]
    point_mode: i32,
    #[serde(skip_serializing_if = "is_zero_size")]
    point_size: f64,
}

fn is_zero_mode(v: &i32) -> bool {
    *v == 0
}

fn is_zero_size(v: &f64) -> bool {
    *v == 0.0
}

impl PointStyleDto {
    pub(super) fn of(document: &Document) -> Self {
        let style = document.point_style();
        Self {
            point_mode: style.mode,
            point_size: style.size,
        }
    }

    /// A mode or size the drawing cannot hold leaves the default style.
    pub(super) fn load(self, document: &mut Document) {
        document.set_point_style(PointStyle {
            mode: self.point_mode,
            size: self.point_size,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::tests::roundtrip;
    use crate::native::NativeFormat;
    use crate::FileFormat;

    #[test]
    fn the_style_round_trips_and_is_left_out_at_the_default() {
        let mut document = Document::default();
        let plain = NativeFormat.write(&document).unwrap().bytes;
        assert!(!String::from_utf8(plain).unwrap().contains("point_"));
        document.set_point_style(PointStyle {
            mode: 35,
            size: -3.5,
        });
        let loaded = roundtrip(&document);
        assert_eq!(loaded.point_style(), document.point_style());
    }
}
