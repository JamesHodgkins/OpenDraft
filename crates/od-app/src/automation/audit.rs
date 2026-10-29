//! `drawing.audit`: read-only housekeeping checks over a drawing, or a part
//! of it. The work is `od_core` (OVERKILL's comparison, PURGE's scans), so
//! the audit reports what those commands would find and never acts on it:
//! it changes nothing, and what a finding means is the caller's to decide.

use std::collections::{BTreeSet, HashMap, HashSet};

use od_api::audit_ops::{AuditArgs, AuditCheck, AuditFinding, AuditResult, NamedFinding};
use od_api::limits::{
    ALLOWED_LAYERS_MAX, AUDIT_COMPARISONS_MAX, AUDIT_OBJECTS_MAX, LISTED_DEFAULT, LISTED_MAX,
};
use od_api::{ApiError, ErrorCode, WireId};
use od_core::overkill::{overkill_capped, prepare, Ignore, OverkillError, Settings};
use od_core::purge::{purgeable, unnamed_with, Category, Item};
use od_core::{Document, EntityId, LayerId, Object};
use serde_json::Value;

use super::command_runs::encode;
use super::read::parse;
use super::resolve;
use super::scope::{objects_in, space_of};
use crate::state::AppState;

/// Longest name passed on.
const NAME_MAX: usize = 80;
/// Longest layer name in `allowed_layers`.
const PATTERN_MAX: usize = 64;

fn invalid(message: &str) -> ApiError {
    ApiError::new(ErrorCode::InvalidArguments, message)
}

/// A name from the drawing, made safe to pass on: no control characters, and
/// bounded. It is still untrusted text.
fn tidy(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_control())
        .take(NAME_MAX)
        .collect()
}

fn checks_wanted(args: &AuditArgs) -> Result<Vec<AuditCheck>, ApiError> {
    let wanted: Vec<AuditCheck> = match &args.checks {
        Some(list) if list.is_empty() => {
            return Err(invalid("checks must name at least one check."));
        }
        Some(list) => AuditCheck::ALL
            .into_iter()
            .filter(|c| list.contains(c))
            .collect(),
        None => AuditCheck::ALL
            .into_iter()
            .filter(|c| *c != AuditCheck::LayerNames || args.allowed_layers.is_some())
            .collect(),
    };
    if wanted.contains(&AuditCheck::LayerNames) && args.allowed_layers.is_none() {
        return Err(invalid(
            "layer_names needs allowed_layers: the names the standard allows.",
        ));
    }
    Ok(wanted)
}

fn patterns_of(args: &AuditArgs) -> Result<Vec<String>, ApiError> {
    let list = args.allowed_layers.clone().unwrap_or_default();
    let bad = list.len() > ALLOWED_LAYERS_MAX
        || list
            .iter()
            .any(|p| p.trim().is_empty() || p.chars().count() > PATTERN_MAX);
    if bad {
        return Err(
            invalid("allowed_layers takes up to 100 names of 1 to 64 characters.")
                .detail("limit", ALLOWED_LAYERS_MAX),
        );
    }
    Ok(list.iter().map(|p| p.trim().to_lowercase()).collect())
}

/// Whether a layer name is on the list: a name matches itself ignoring case,
/// and one ending in `*` matches every name that starts so. Layer 0 and
/// Defpoints (made by dimensions) always pass.
fn allowed(name: &str, patterns: &[String]) -> bool {
    let name = name.to_lowercase();
    name == "0"
        || name == "defpoints"
        || patterns.iter().any(|p| match p.strip_suffix('*') {
            Some(prefix) => name.starts_with(prefix),
            None => name == *p,
        })
}

fn pair_settings(tolerance: f64, overlapping: bool) -> Settings {
    Settings {
        tolerance,
        ignore: Ignore::default(),
        optimize_polylines: false,
        overlapping,
        end_to_end: false,
        maintain_associative: true,
    }
}

/// What OVERKILL would delete and what it would change, as ids; or why it
/// could not be asked in a quick look.
type Pairs = Result<(Vec<EntityId>, Vec<EntityId>), String>;

fn compare(document: &Document, ids: &[EntityId], settings: &Settings) -> Pairs {
    let prepared = prepare(document, ids, settings, true);
    match overkill_capped(
        &prepared.items,
        settings,
        AUDIT_OBJECTS_MAX,
        AUDIT_COMPARISONS_MAX,
    ) {
        Ok(outcome) => Ok((
            outcome.removed.iter().map(|i| prepared.ids[*i]).collect(),
            outcome
                .changed
                .iter()
                .map(|(i, _)| prepared.ids[*i])
                .collect(),
        )),
        Err(OverkillError::TooManyObjects { count, limit }) => Err(format!(
            "{count} objects is more than the {limit} compared at once. Narrow it with layer_id, space or bounds."
        )),
        Err(OverkillError::TooMuchWork { .. }) => Err(
            "Too many comparisons for a quick answer. Narrow it with layer_id, space or bounds."
                .to_owned(),
        ),
        Err(OverkillError::BadTolerance) => {
            Err("The tolerance must be zero or more.".to_owned())
        }
    }
}

fn finding(check: AuditCheck) -> AuditFinding {
    AuditFinding {
        check,
        count: 0,
        entity_ids: Vec::new(),
        names: Vec::new(),
        truncated: false,
        not_run: None,
    }
}

fn of_ids(check: AuditCheck, ids: &[EntityId], limit: usize) -> AuditFinding {
    AuditFinding {
        count: ids.len(),
        entity_ids: ids.iter().take(limit).map(|id| WireId(id.0)).collect(),
        truncated: ids.len() > limit,
        ..finding(check)
    }
}

fn of_names(check: AuditCheck, mut names: Vec<NamedFinding>, limit: usize) -> AuditFinding {
    let count = names.len();
    names.truncate(limit);
    AuditFinding {
        count,
        truncated: count > limit,
        names,
        ..finding(check)
    }
}

fn not_run(check: AuditCheck, why: &str) -> AuditFinding {
    AuditFinding {
        not_run: Some(why.to_owned()),
        ..finding(check)
    }
}

fn kind_of(item: &Item) -> &'static str {
    match item {
        Item::Block(_) => "block",
        Item::DimStyle(_) => "dim_style",
        Item::Layer(_) => "layer",
        Item::Linetype(_) => "linetype",
        Item::MLeaderStyle(_) => "mleader_style",
        Item::TextStyle(_) => "text_style",
        Item::ZeroLength(_) | Item::EmptyText(_) => "object",
    }
}

/// Objects taking part in a partial overlap: those OVERKILL would merge,
/// leaving out the plain duplicates, which have their own check.
fn overlapping(dup: &Pairs, merge: &Pairs) -> Result<Vec<EntityId>, String> {
    let (dup_removed, _) = dup.as_ref().map_err(String::clone)?;
    let (removed, changed) = merge.as_ref().map_err(String::clone)?;
    let plain: HashSet<&EntityId> = dup_removed.iter().collect();
    let taking_part: BTreeSet<EntityId> = removed
        .iter()
        .chain(changed)
        .filter(|id| !plain.contains(id))
        .copied()
        .collect();
    Ok(taking_part.into_iter().collect())
}

fn unused_definitions(document: &Document, limit: usize) -> AuditFinding {
    let names = purgeable(document)
        .into_iter()
        // A new drawing carries the stock linetypes, so they say nothing.
        .filter(|p| p.item.category() != Category::Linetypes)
        .map(|p| NamedFinding {
            kind: kind_of(&p.item).to_owned(),
            name: tidy(&p.name),
            objects: None,
        })
        .collect();
    of_names(AuditCheck::UnusedDefinitions, names, limit)
}

fn layer_names(document: &Document, patterns: &[String], limit: usize) -> AuditFinding {
    let mut on_layer: HashMap<LayerId, usize> = HashMap::new();
    for object in document.objects() {
        *on_layer.entry(object.attrs.layer).or_default() += 1;
    }
    let names = document
        .layers()
        .iter()
        .filter(|layer| !allowed(&layer.name, patterns))
        .map(|layer| NamedFinding {
            kind: "layer".to_owned(),
            name: tidy(&layer.name),
            objects: Some(on_layer.get(&layer.id).copied().unwrap_or(0)),
        })
        .collect();
    of_names(AuditCheck::LayerNames, names, limit)
}

/// `drawing.audit`.
pub(super) fn audit(state: &AppState, instance: &str, arguments: Value) -> Result<Value, ApiError> {
    let args: AuditArgs = parse(arguments)?;
    let doc = resolve(state, instance, &args.document_id)?;
    let document = doc.session.document();
    let space = space_of(document, args.space.as_ref())?;
    let layer = match args.layer_id {
        None => None,
        Some(id) => {
            let id = LayerId(id.0);
            if document.layer(id).is_none() {
                return Err(ApiError::new(ErrorCode::InvalidReference, "No such layer.")
                    .detail("layer_id", id.0));
            }
            Some(id)
        }
    };
    let tolerance = args.tolerance.unwrap_or(1e-6);
    if !(tolerance.is_finite() && tolerance >= 0.0) {
        return Err(invalid("tolerance must be zero or more."));
    }
    let checks = checks_wanted(&args)?;
    let patterns = patterns_of(&args)?;
    let limit = args.limit.unwrap_or(LISTED_DEFAULT).clamp(1, LISTED_MAX);

    let in_scope: Vec<&Object> = objects_in(document, space, args.bounds.as_ref())?
        .into_iter()
        .filter(|o| layer.is_none_or(|l| o.attrs.layer == l))
        .collect();
    let ids: Vec<EntityId> = in_scope.iter().map(|o| o.id).collect();
    let scope: HashSet<EntityId> = ids.iter().copied().collect();

    // Duplicates and overlaps share work, and overlaps leave out duplicates.
    let wants = |c: AuditCheck| checks.contains(&c);
    let dup = (wants(AuditCheck::Duplicates) || wants(AuditCheck::Overlaps))
        .then(|| compare(document, &ids, &pair_settings(tolerance, false)));
    let merge = wants(AuditCheck::Overlaps)
        .then(|| compare(document, &ids, &pair_settings(tolerance, true)));
    let unnamed = (wants(AuditCheck::ZeroLength) || wants(AuditCheck::EmptyText))
        .then(|| unnamed_with(document, true));
    let in_scope_only = |found: &[EntityId]| -> Vec<EntityId> {
        found
            .iter()
            .filter(|id| scope.contains(id))
            .copied()
            .collect()
    };

    let mut findings = Vec::new();
    for check in &checks {
        findings.push(match check {
            AuditCheck::Duplicates => match dup.as_ref() {
                Some(Ok((removed, _))) => of_ids(*check, removed, limit),
                Some(Err(why)) => not_run(*check, why),
                None => finding(*check),
            },
            AuditCheck::Overlaps => match dup.as_ref().zip(merge.as_ref()) {
                Some((dup, merge)) => match overlapping(dup, merge) {
                    Ok(ids) => of_ids(*check, &ids, limit),
                    Err(why) => not_run(*check, &why),
                },
                None => finding(*check),
            },
            AuditCheck::ZeroLength => {
                let found = unnamed.as_ref().map(|u| in_scope_only(&u.zero_length));
                of_ids(*check, &found.unwrap_or_default(), limit)
            }
            AuditCheck::EmptyText => {
                let found = unnamed.as_ref().map(|u| in_scope_only(&u.empty_text));
                of_ids(*check, &found.unwrap_or_default(), limit)
            }
            AuditCheck::UnusedDefinitions => unused_definitions(document, limit),
            AuditCheck::LayerNames => layer_names(document, &patterns, limit),
        });
    }
    encode(&AuditResult {
        document_id: args.document_id,
        revision: WireId(doc.session.revision()),
        scanned: ids.len(),
        findings,
    })
}
