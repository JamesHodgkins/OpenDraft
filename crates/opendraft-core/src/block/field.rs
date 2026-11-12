//! Data fields (DXF `ACAD_FIELD`): text whose value comes from somewhere
//! else, with the last value it had kept beside it.
//!
//! A field is stored as what it is (an evaluator, a code and a format, the
//! objects it points at, any child fields) and the value it last had. This
//! build evaluates none of them yet, so a field always shows its cached
//! value; one with no cache shows `####` and reports a diagnostic. The code
//! is never shown as if it were the text.

use crate::{BlockId, EntityId, LayoutId, TextStyleId};

use super::attribute::AttributeDefinitionId;

/// Longest field code.
pub const MAX_CODE_BYTES: usize = 256 * 1024;
/// Longest format string.
pub const MAX_FORMAT_BYTES: usize = 64 * 1024;
/// Longest evaluator name.
pub const MAX_EVALUATOR_BYTES: usize = 256;
/// Most fields in one text.
pub const MAX_FIELDS_PER_TEXT: usize = 64;
/// Deepest nesting of child fields.
pub const MAX_FIELD_DEPTH: usize = 16;
/// Most field nodes (the field and all its descendants) in one text.
pub const MAX_FIELD_NODES: usize = 4096;
/// Most object references in one field tree.
pub const MAX_FIELD_REFS: usize = 1024;
/// Largest binary cache of one field.
pub const MAX_CACHED_BYTES: usize = 1024 * 1024;
/// Longest literal text of an attribute value or prompt.
pub const MAX_TEXT_BYTES: usize = 64 * 1024;

/// What displays where a field has no cached value.
pub const NO_VALUE: &str = "####";

/// The thing a field points at, by the drawing's own ids, never a file handle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldObjectRef {
    Entity(EntityId),
    AttributeDefinition(AttributeDefinitionId),
    Block(BlockId),
    Layout(LayoutId),
    TextStyle(TextStyleId),
    /// A reference a file named that this drawing cannot resolve, kept as the
    /// file spelled it so the diagnostic can say what was lost.
    Unresolved(String),
}

/// The value a field last had, in the categories Autodesk files cache.
#[derive(Clone, Debug, PartialEq)]
pub enum FieldValue {
    Text(String),
    I32(i32),
    F64(f64),
    /// The id of an object, as the file's own number.
    Id(u64),
    Bytes(Vec<u8>),
}

impl FieldValue {
    /// The value as it is shown.
    pub fn display(&self) -> String {
        match self {
            Self::Text(s) => s.clone(),
            Self::I32(v) => v.to_string(),
            Self::F64(v) => v.to_string(),
            Self::Id(v) => v.to_string(),
            Self::Bytes(_) => NO_VALUE.to_string(),
        }
    }
}

/// How far a field's value can be trusted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldStatus {
    /// Never evaluated; the cache, if any, came from a file.
    Unevaluated,
    Success,
    /// No evaluator in this build handles it; the cache is all there is.
    Unsupported,
    /// The code could not be parsed.
    ParseError,
    /// Evaluation failed; the previous cache is kept.
    EvaluationFailed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DataField {
    pub evaluator: String,
    pub code: String,
    pub format: String,
    pub object_refs: Vec<FieldObjectRef>,
    pub children: Vec<DataField>,
    pub cached: Option<FieldValue>,
    pub status: FieldStatus,
}

impl DataField {
    /// A field no evaluator has seen, with the value a file cached for it.
    pub fn new(evaluator: &str, code: &str, cached: Option<FieldValue>) -> Self {
        Self {
            evaluator: evaluator.to_string(),
            code: code.to_string(),
            format: String::new(),
            object_refs: Vec::new(),
            children: Vec::new(),
            cached,
            status: FieldStatus::Unsupported,
        }
    }

    /// What the field shows: its cached value, or [`NO_VALUE`].
    pub fn display(&self) -> String {
        self.cached
            .as_ref()
            .map_or_else(|| NO_VALUE.to_string(), FieldValue::display)
    }

    /// Whether the field shows a real value rather than the placeholder.
    pub fn has_value(&self) -> bool {
        self.cached.is_some()
    }

    /// This field and every descendant.
    fn nodes(&self) -> usize {
        1 + self.children.iter().map(DataField::nodes).sum::<usize>()
    }

    fn refs(&self) -> usize {
        self.object_refs.len() + self.children.iter().map(DataField::refs).sum::<usize>()
    }

    fn depth(&self) -> usize {
        1 + self
            .children
            .iter()
            .map(DataField::depth)
            .max()
            .unwrap_or(0)
    }

    fn check(&self) -> Result<(), String> {
        if self.evaluator.len() > MAX_EVALUATOR_BYTES
            || self.code.len() > MAX_CODE_BYTES
            || self.format.len() > MAX_FORMAT_BYTES
            || [&self.evaluator, &self.code, &self.format]
                .iter()
                .any(|s| s.contains('\0'))
        {
            return Err("a field's evaluator, code or format is too long or holds NUL".into());
        }
        match &self.cached {
            Some(FieldValue::F64(v)) if !v.is_finite() => {
                return Err("a field's value is not a finite number".into());
            }
            Some(FieldValue::Bytes(b)) if b.len() > MAX_CACHED_BYTES => {
                return Err("a field's cached data is too large".into());
            }
            Some(FieldValue::Text(s)) if s.len() > MAX_TEXT_BYTES || s.contains('\0') => {
                return Err("a field's cached text is too long or holds NUL".into());
            }
            _ => {}
        }
        self.children.iter().try_for_each(DataField::check)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FieldRun {
    Literal(String),
    Field(DataField),
}

/// The text of an attribute: plain, or runs of plain text and fields.
#[derive(Clone, Debug, PartialEq)]
pub enum FieldText {
    Literal(String),
    Runs(Vec<FieldRun>),
}

impl Default for FieldText {
    fn default() -> Self {
        Self::Literal(String::new())
    }
}

impl From<&str> for FieldText {
    fn from(s: &str) -> Self {
        Self::Literal(s.to_string())
    }
}

impl FieldText {
    /// What the text shows: literals as they are, fields as their cache.
    pub fn display(&self) -> String {
        match self {
            Self::Literal(s) => s.clone(),
            Self::Runs(runs) => runs
                .iter()
                .map(|r| match r {
                    FieldRun::Literal(s) => s.clone(),
                    FieldRun::Field(f) => f.display(),
                })
                .collect(),
        }
    }

    /// The fields in it, in order.
    pub fn fields(&self) -> impl Iterator<Item = &DataField> {
        let runs: &[FieldRun] = match self {
            Self::Literal(_) => &[],
            Self::Runs(runs) => runs,
        };
        runs.iter().filter_map(|r| match r {
            FieldRun::Field(f) => Some(f),
            FieldRun::Literal(_) => None,
        })
    }

    pub fn has_fields(&self) -> bool {
        self.fields().next().is_some()
    }

    /// Whether some field in it has no value to show.
    pub fn has_unavailable_field(&self) -> bool {
        self.fields().any(|f| !f.has_value())
    }

    /// The plain text, when there are no fields.
    pub fn as_literal(&self) -> Option<&str> {
        match self {
            Self::Literal(s) => Some(s),
            Self::Runs(_) => None,
        }
    }

    /// Whether the text is within every limit.
    pub fn validate(&self) -> Result<(), String> {
        let literal_ok = |s: &str| s.len() <= MAX_TEXT_BYTES && !s.contains('\0');
        match self {
            Self::Literal(s) if !literal_ok(s) => Err("text is too long or holds NUL".into()),
            Self::Literal(_) => Ok(()),
            Self::Runs(runs) => {
                let fields: Vec<&DataField> = self.fields().collect();
                if fields.len() > MAX_FIELDS_PER_TEXT {
                    return Err("too many fields in one text".into());
                }
                if runs
                    .iter()
                    .any(|r| matches!(r, FieldRun::Literal(s) if !literal_ok(s)))
                {
                    return Err("text is too long or holds NUL".into());
                }
                let nodes: usize = fields.iter().map(|f| f.nodes()).sum();
                let refs: usize = fields.iter().map(|f| f.refs()).sum();
                if nodes > MAX_FIELD_NODES
                    || refs > MAX_FIELD_REFS
                    || fields.iter().any(|f| f.depth() > MAX_FIELD_DEPTH)
                {
                    return Err("a field tree is too large".into());
                }
                fields.iter().try_for_each(|f| f.check())
            }
        }
    }

    /// Every object reference in every field, children included.
    pub fn object_refs(&self) -> Vec<&FieldObjectRef> {
        fn walk<'a>(f: &'a DataField, out: &mut Vec<&'a FieldObjectRef>) {
            out.extend(&f.object_refs);
            f.children.iter().for_each(|c| walk(c, out));
        }
        let mut out = Vec::new();
        self.fields().for_each(|f| walk(f, &mut out));
        out
    }

    /// The same text with every reference passed through `map`.
    pub fn map_refs(&self, map: &dyn Fn(&FieldObjectRef) -> FieldObjectRef) -> FieldText {
        fn field(f: &DataField, map: &dyn Fn(&FieldObjectRef) -> FieldObjectRef) -> DataField {
            DataField {
                object_refs: f.object_refs.iter().map(map).collect(),
                children: f.children.iter().map(|c| field(c, map)).collect(),
                ..f.clone()
            }
        }
        match self {
            Self::Literal(s) => Self::Literal(s.clone()),
            Self::Runs(runs) => Self::Runs(
                runs.iter()
                    .map(|r| match r {
                        FieldRun::Literal(s) => FieldRun::Literal(s.clone()),
                        FieldRun::Field(f) => FieldRun::Field(field(f, map)),
                    })
                    .collect(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(cache: Option<FieldValue>) -> DataField {
        DataField::new("AcVar", "\\AcVar Filename", cache)
    }

    #[test]
    fn display_joins_literals_and_cached_values() {
        let text = FieldText::Runs(vec![
            FieldRun::Literal("File: ".into()),
            FieldRun::Field(field(Some(FieldValue::Text("plan.dwg".into())))),
        ]);
        assert_eq!(text.display(), "File: plan.dwg");
        assert!(text.has_fields() && !text.has_unavailable_field());
    }

    #[test]
    fn a_field_without_a_cache_shows_the_placeholder_never_its_code() {
        let text = FieldText::Runs(vec![FieldRun::Field(field(None))]);
        assert_eq!(text.display(), NO_VALUE);
        assert!(text.has_unavailable_field());
    }

    #[test]
    fn numbers_display_as_numbers() {
        assert_eq!(FieldValue::I32(7).display(), "7");
        assert_eq!(FieldValue::F64(2.5).display(), "2.5");
        assert_eq!(FieldValue::Bytes(vec![1]).display(), NO_VALUE);
    }

    #[test]
    fn limits_refuse_deep_wide_and_hostile_fields() {
        let mut deep = field(None);
        for _ in 0..MAX_FIELD_DEPTH {
            deep = DataField {
                children: vec![deep],
                ..field(None)
            };
        }
        let text = FieldText::Runs(vec![FieldRun::Field(deep)]);
        assert!(text.validate().is_err());

        let many = FieldText::Runs(
            (0..=MAX_FIELDS_PER_TEXT)
                .map(|_| FieldRun::Field(field(None)))
                .collect(),
        );
        assert!(many.validate().is_err());

        let nul = FieldText::Literal("a\0b".into());
        assert!(nul.validate().is_err());

        let nan = FieldText::Runs(vec![FieldRun::Field(field(Some(FieldValue::F64(
            f64::NAN,
        ))))]);
        assert!(nan.validate().is_err());

        let ok = FieldText::Runs(vec![FieldRun::Field(field(Some(FieldValue::I32(1))))]);
        assert!(ok.validate().is_ok());
    }

    #[test]
    fn references_are_collected_and_remapped_through_children() {
        let child = DataField {
            object_refs: vec![FieldObjectRef::Entity(EntityId(2))],
            ..field(None)
        };
        let parent = DataField {
            object_refs: vec![FieldObjectRef::Entity(EntityId(1))],
            children: vec![child],
            ..field(None)
        };
        let text = FieldText::Runs(vec![FieldRun::Field(parent)]);
        assert_eq!(text.object_refs().len(), 2);
        let moved = text.map_refs(&|r| match r {
            FieldObjectRef::Entity(id) => FieldObjectRef::Entity(EntityId(id.0 + 10)),
            other => other.clone(),
        });
        let refs = moved.object_refs();
        assert_eq!(refs[0], &FieldObjectRef::Entity(EntityId(11)));
        assert_eq!(refs[1], &FieldObjectRef::Entity(EntityId(12)));
    }
}
