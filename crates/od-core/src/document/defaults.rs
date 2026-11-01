//! A new, empty drawing: layer `0`, the standard styles and tables.

use super::*;

impl Default for Document {
    fn default() -> Self {
        let mut doc = Self {
            objects: Vec::new(),
            next_id: 0,
            positions: HashMap::new(),
            layers: Vec::new(),
            layer_positions: HashMap::new(),
            next_layer_id: 1,
            current_layer: LayerId::DEFAULT,
            new_color: Attr::ByLayer,
            new_linetype: Attr::ByLayer,
            new_lineweight: Attr::ByLayer,
            units: Units::default(),
            properties: DrawingProperties::default(),
            dim_style: DimStyle::default(),
            dim_styles: vec![DimStyle::default()],
            layouts: Vec::new(),
            next_layout_id: 1,
            next_viewport_id: 1,
            active_layout: None,
            active_frozen: Vec::new(),
            text_styles: vec![TextStyle::standard()],
            next_text_style_id: 1,
            current_text_style: TextStyleId::STANDARD,
            mleader_styles: vec![MLeaderStyle::standard()],
            next_mleader_style_id: 1,
            current_mleader_style: MLeaderStyleId::STANDARD,
            blocks: Vec::new(),
            next_block_id: 1,
            next_attribute_definition_id: 1,
            linetypes: LinetypeTable::default(),
            linetype_scale: 1.0,
            point_style: PointStyle::default(),
            spatial: Default::default(),
        };
        doc.push_layer(Layer::new(LayerId::DEFAULT, "0"));
        doc
    }
}
