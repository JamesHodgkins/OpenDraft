//! How point objects are drawn (`PDMODE`, `PDSIZE`), always written.

use od_core::{Document, PointStyle};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub(super) struct PointStyleDto {
    point_mode: i32,
    point_size: f64,
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
    fn the_style_round_trips_and_is_written_at_the_default_too() {
        let mut document = Document::default();
        let plain = NativeFormat.write(&document).unwrap().bytes;
        assert!(String::from_utf8(plain)
            .unwrap()
            .contains("\"point_mode\": 0"));
        document.set_point_style(PointStyle {
            mode: 35,
            size: -3.5,
        });
        let loaded = roundtrip(&document);
        assert_eq!(loaded.point_style(), document.point_style());
    }
}
