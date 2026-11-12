//! `layers.apply_batch`: create layers, change their properties and choose
//! the current one, atomically. Deleting or merging layers is not offered:
//! the API cannot yet say what happens to what is on them.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::ColorDto;
use crate::WireId;

/// An existing layer, or one created earlier in the same batch.
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(untagged, deny_unknown_fields)]
pub enum LayerTarget {
    Id { layer_id: WireId },
    Ref { client_ref: String },
}

/// Properties of a layer; every field is optional on `update`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayerProps {
    /// Unique ignoring case; layer 0 cannot be renamed.
    #[serde(default)]
    pub name: Option<String>,
    /// `foreground` or an rgb colour; a layer's colour is never by-layer.
    #[serde(default)]
    pub color: Option<ColorDto>,
    /// An existing linetype name.
    #[serde(default)]
    pub linetype: Option<String>,
    /// Hundredths of a millimetre, one of the standard weights.
    #[serde(default)]
    pub lineweight_hundredths_mm: Option<u16>,
    #[serde(default)]
    pub visible: Option<bool>,
    /// The current layer cannot be frozen.
    #[serde(default)]
    pub frozen: Option<bool>,
    #[serde(default)]
    pub locked: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum LayerOperation {
    Create {
        client_ref: String,
        /// Required.
        name: String,
        #[serde(default)]
        props: LayerProps,
    },
    Update {
        target: LayerTarget,
        props: LayerProps,
    },
    /// A frozen layer cannot be current.
    SetCurrent { target: LayerTarget },
}

#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayersBatchArgs {
    pub document_id: String,
    pub expected_revision: WireId,
    #[serde(default)]
    pub label: Option<String>,
    pub operations: Vec<LayerOperation>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayersBatchResult {
    pub document_id: String,
    pub revision_before: WireId,
    pub revision_after: WireId,
    pub changed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_step: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history_token: Option<WireId>,
    /// Each created layer's id, by its `client_ref`.
    pub ids: BTreeMap<String, WireId>,
    pub current_layer_id: WireId,
}
