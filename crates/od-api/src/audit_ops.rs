//! `drawing.audit` and `drawing.summary`: two read-only looks that answer
//! "is this drawing tidy?" and "what is where?" without reading every
//! entity. The checks are plain geometry and table lookups, not judgement:
//! a caller (a person, or a model) decides what a finding means.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::entity::{BoundsDto, PointDto};
use crate::ops::{BoundsArg, SpaceFilter};
use crate::WireId;

/// One housekeeping check. (Plain comments, not docs: the variants' text would
/// be repeated in every tool listing, and the tool description says it.)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AuditCheck {
    // Objects that repeat another exactly, or lie wholly on top of one.
    Duplicates,
    // Lines, arcs and the like overlapping another in part (same line or
    // circle); every object taking part is listed, plain duplicates are not.
    Overlaps,
    // Lines, arcs, circles and polylines with no length.
    ZeroLength,
    // Text objects with nothing in them.
    EmptyText,
    // Blocks, layers, text, dimension and multileader styles nothing uses.
    // Linetypes are not reported: a new drawing carries the stock set.
    UnusedDefinitions,
    // Layers whose names are not in `allowed_layers`.
    LayerNames,
}

impl AuditCheck {
    /// In the order results are given.
    pub const ALL: [AuditCheck; 6] = [
        AuditCheck::Duplicates,
        AuditCheck::Overlaps,
        AuditCheck::ZeroLength,
        AuditCheck::EmptyText,
        AuditCheck::UnusedDefinitions,
        AuditCheck::LayerNames,
    ];
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AuditArgs {
    pub document_id: String,
    /// Checks to run: all by default (`layer_names` only with `allowed_layers`).
    #[serde(default)]
    pub checks: Option<Vec<AuditCheck>>,
    /// Look only at objects on this layer.
    #[serde(default)]
    pub layer_id: Option<WireId>,
    /// Defaults to model space.
    #[serde(default)]
    pub space: Option<SpaceFilter>,
    /// Only objects whose box touches this: audit a big drawing in parts.
    #[serde(default)]
    pub bounds: Option<BoundsArg>,
    /// How far apart numbers may be and still match, in drawing units (default 0.000001).
    #[serde(default)]
    pub tolerance: Option<f64>,
    /// For `layer_names`: names the standard allows; a trailing `*` matches a prefix. Layer 0 and Defpoints pass.
    #[serde(default)]
    pub allowed_layers: Option<Vec<String>>,
    /// Ids or names listed per check: 30 by default, 200 at most (counts are complete).
    #[serde(default)]
    pub limit: Option<usize>,
}

/// A named thing a check found: a layer or an unused definition. The name is
/// drawing data: untrusted.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NamedFinding {
    /// `layer`, `block`, `text_style`, `dim_style` or `mleader_style`.
    pub kind: String,
    pub name: String,
    /// For a layer: how many objects are on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub objects: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AuditFinding {
    pub check: AuditCheck,
    /// All that were found, whatever `limit` listed.
    pub count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entity_ids: Vec<WireId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub names: Vec<NamedFinding>,
    /// More were found than listed.
    pub truncated: bool,
    /// Why the check did not run (too many objects to compare quickly), and
    /// what to do about it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub not_run: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AuditResult {
    pub document_id: String,
    pub revision: WireId,
    /// Objects that were looked at.
    pub scanned: usize,
    pub findings: Vec<AuditFinding>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SummaryArgs {
    pub document_id: String,
    /// Defaults to model space.
    #[serde(default)]
    pub space: Option<SpaceFilter>,
    /// Only objects whose box touches this; the density grid then covers this box.
    #[serde(default)]
    pub bounds: Option<BoundsArg>,
    /// Layers, block names and texts listed: 30 by default, 200 at most (totals are complete).
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KindCount {
    pub kind: String,
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayerCount {
    pub layer_id: WireId,
    pub name: String,
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BlockCount {
    pub name: String,
    /// How many inserts of it.
    pub count: usize,
    /// Where the first one is.
    pub first_at: PointDto,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextItem {
    pub entity_id: WireId,
    /// What a reader sees, on one line (line breaks shown as ` / `), cut at
    /// 120 characters. Untrusted drawing text.
    pub text: String,
    pub at: PointDto,
}

/// Object counts over a coarse grid, to decide where to look closer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Density {
    /// The box the grid covers.
    pub bounds: BoundsDto,
    pub columns: usize,
    pub rows: usize,
    /// `counts[row][column]`; row 0 is the bottom (lowest y), column 0 the
    /// left. Each object counts once, in the cell holding the centre of its
    /// box; one whose centre lies outside the box is not counted.
    pub counts: Vec<Vec<usize>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SummaryResult {
    pub document_id: String,
    pub revision: WireId,
    /// Objects that were looked at.
    pub scanned: usize,
    /// The box round those that have one (construction lines and rays do
    /// not), or none when there are none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<BoundsDto>,
    pub by_kind: Vec<KindCount>,
    pub by_layer: Vec<LayerCount>,
    pub layers_total: usize,
    pub blocks: Vec<BlockCount>,
    pub blocks_total: usize,
    pub texts: Vec<TextItem>,
    pub texts_total: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub density: Option<Density>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_are_named_in_snake_case_and_all_are_listed_once() {
        let names: Vec<String> = AuditCheck::ALL
            .iter()
            .map(|c| serde_json::to_string(c).unwrap())
            .collect();
        assert_eq!(names[4], "\"unused_definitions\"");
        let mut unique = names.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), AuditCheck::ALL.len());
    }

    #[test]
    fn arguments_reject_unknown_fields_and_bad_checks() {
        let ok = r#"{"document_id":"d","checks":["duplicates"],"limit":5}"#;
        assert!(serde_json::from_str::<AuditArgs>(ok).is_ok());
        assert!(serde_json::from_str::<AuditArgs>(r#"{"document_id":"d","fix":true}"#).is_err());
        assert!(
            serde_json::from_str::<AuditArgs>(r#"{"document_id":"d","checks":["tidy_up"]}"#)
                .is_err()
        );
        assert!(serde_json::from_str::<SummaryArgs>(r#"{"document_id":"d","depth":2}"#).is_err());
    }
}
