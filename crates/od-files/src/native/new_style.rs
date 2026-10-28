//! The colour, linetype and lineweight new objects are drawn with
//! (`CECOLOR`, `CELTYPE`, `CELWEIGHT`). Each is left out while ByLayer, so
//! files from before they existed still read.

use od_core::{Color, Document, LinetypeId, Lineweight};
use serde::{Deserialize, Serialize};

use super::{attr_from_dto, attr_to_dto, AttrDto, ColorDto};

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub(super) struct NewStyleDto {
    #[serde(skip_serializing_if = "by_layer")]
    new_color: AttrDto<ColorDto>,
    #[serde(skip_serializing_if = "by_layer")]
    new_linetype: AttrDto<u64>,
    #[serde(skip_serializing_if = "by_layer")]
    new_lineweight: AttrDto<u16>,
}

fn by_layer<T>(attr: &AttrDto<T>) -> bool {
    matches!(attr, AttrDto::ByLayer)
}

impl NewStyleDto {
    pub(super) fn of(document: &Document) -> Self {
        let (color, linetype, lineweight) = document.new_object_style();
        Self {
            new_color: attr_to_dto(color, |c| (*c).into()),
            new_linetype: attr_to_dto(linetype, |id| id.0),
            new_lineweight: attr_to_dto(lineweight, |w| w.0),
        }
    }

    pub(super) fn load(self, document: &mut Document) {
        document.set_new_object_style(
            attr_from_dto(self.new_color, Color::from),
            attr_from_dto(self.new_linetype, LinetypeId),
            attr_from_dto(self.new_lineweight, Lineweight),
        );
    }
}

#[cfg(test)]
mod tests {
    use od_core::Attr;

    use super::*;
    use crate::native::tests::roundtrip;
    use crate::native::NativeFormat;
    use crate::FileFormat;

    #[test]
    fn the_current_style_round_trips_and_is_left_out_when_by_layer() {
        let mut document = Document::default();
        let plain = NativeFormat.write(&document).unwrap().bytes;
        assert!(!String::from_utf8(plain).unwrap().contains("new_"));
        document.set_new_object_style(
            Attr::Set(Color::Rgb(1, 2, 3)),
            Attr::Set(od_core::LinetypeId::CENTER),
            Attr::Set(Lineweight(50)),
        );
        let loaded = roundtrip(&document);
        assert_eq!(loaded.new_object_style(), document.new_object_style());
    }
}
