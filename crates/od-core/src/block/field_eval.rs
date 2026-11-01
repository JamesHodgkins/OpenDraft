//! Evaluating data fields: the two evaluators this build has, and the planner
//! that brings the fields of inserts up to date.
//!
//! `AcObjProp` shows a property of an object in the drawing (`Length`, `Area`,
//! `Radius`, `Diameter`, `Circumference`, `Layer`, `Name`); the object is the
//! field's first reference, never written into the code as a handle. `AcVar`
//! with `Filename` shows the drawing's file name. Any other evaluator is left
//! as it is (`Unsupported`, cache kept). A field that cannot be evaluated now
//! (its object is gone, the property does not suit it, the drawing has no file
//! yet) keeps its last value and says `EvaluationFailed`.

use crate::drawing_properties::DrawingProperties;
use crate::units::{format_area, format_length};
use crate::{arc, bulge, Document, Entity, EntityId};

use super::field::{
    DataField, FieldObjectRef, FieldRun, FieldStatus, FieldText, FieldValue, MAX_TEXT_BYTES,
};
use super::field_kinds::{apply_case, format_time, FieldTime, DEFAULT_DATE};

/// The evaluator of object properties.
pub const OBJECT_PROPERTY: &str = "AcObjProp";
/// The evaluator of drawing variables.
pub const VARIABLE: &str = "AcVar";
/// The drawing variable that holds the file name.
const FILE_NAME: &str = "Filename";

/// What a field may ask of the world beyond the drawing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FieldContext {
    /// The drawing's file name with its extension; `None` until it is saved.
    pub file_name: Option<String>,
    /// The time now, in the user's own time zone; `None` if it is not known.
    pub now: Option<FieldTime>,
    /// The drawing's title, author and the rest of its description.
    pub properties: DrawingProperties,
}

/// A property `AcObjProp` can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectProperty {
    Length,
    Area,
    Radius,
    Diameter,
    Circumference,
    Layer,
    Name,
}

impl ObjectProperty {
    pub const ALL: [ObjectProperty; 7] = [
        Self::Length,
        Self::Area,
        Self::Radius,
        Self::Diameter,
        Self::Circumference,
        Self::Layer,
        Self::Name,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Length => "Length",
            Self::Area => "Area",
            Self::Radius => "Radius",
            Self::Diameter => "Diameter",
            Self::Circumference => "Circumference",
            Self::Layer => "Layer",
            Self::Name => "Name",
        }
    }

    /// The property called `name`, in any case.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|p| p.name().eq_ignore_ascii_case(name))
    }

    /// The properties `entity` has, in the order they are offered.
    pub fn of(entity: &Entity) -> Vec<Self> {
        Self::ALL
            .into_iter()
            .filter(|p| *p == Self::Layer || measure(entity, *p).is_some())
            .collect()
    }
}

/// What `property` is of `entity`, as a length, an area or a name; `None`
/// where the object has no such property.
enum Measured {
    Length(f64),
    Area(f64),
    Text(String),
}

fn measure(entity: &Entity, property: ObjectProperty) -> Option<Measured> {
    use ObjectProperty as P;
    let length = |v: f64| v.is_finite().then_some(Measured::Length(v));
    let area = |v: f64| v.is_finite().then_some(Measured::Area(v));
    match (entity, property) {
        (Entity::Insert(i), P::Name) => Some(Measured::Text(i.block.name.clone())),
        (Entity::Line { start, end }, P::Length) => {
            length((end.x - start.x).hypot(end.y - start.y))
        }
        (Entity::Circle { radius, .. }, P::Radius) | (Entity::Arc { radius, .. }, P::Radius) => {
            length(radius.abs())
        }
        (Entity::Circle { radius, .. }, P::Diameter)
        | (Entity::Arc { radius, .. }, P::Diameter) => length(radius.abs() * 2.0),
        (Entity::Circle { radius, .. }, P::Circumference) => {
            length(std::f64::consts::TAU * radius.abs())
        }
        (Entity::Circle { radius, .. }, P::Area) => area(std::f64::consts::PI * radius * radius),
        (
            Entity::Arc {
                radius, start, end, ..
            },
            P::Length,
        ) => length(radius.abs() * arc::sweep(*start, *end)),
        (Entity::Rectangle { min, max }, P::Length) => {
            length(2.0 * ((max.x - min.x).abs() + (max.y - min.y).abs()))
        }
        (Entity::Rectangle { min, max }, P::Area) => {
            area(((max.x - min.x) * (max.y - min.y)).abs())
        }
        (
            Entity::Polyline {
                points,
                closed,
                bulges,
                ..
            },
            P::Length,
        ) => length(bulge::path_length(points, bulges, *closed)),
        (
            Entity::Polyline {
                points,
                closed: true,
                bulges,
                ..
            },
            P::Area,
        ) => area(bulge::path_area(points, bulges).abs()),
        (Entity::Ellipse(e), P::Length | P::Circumference) => length(e.length()),
        (Entity::Ellipse(e), P::Area) => area(e.area()),
        _ => None,
    }
}

impl DataField {
    /// A field showing `property` of the object `id`. It has no value until
    /// it is evaluated.
    pub fn object_property(id: EntityId, property: ObjectProperty) -> Self {
        let mut field = Self::new(
            OBJECT_PROPERTY,
            &format!(
                "\\{OBJECT_PROPERTY} Object(%<\\_ObjId 0>%).{}",
                property.name()
            ),
            None,
        );
        field.object_refs = vec![FieldObjectRef::Entity(id)];
        field.status = FieldStatus::Unevaluated;
        field
    }

    /// A field showing the drawing's file name.
    pub fn file_name() -> Self {
        Self::variable(FILE_NAME, "")
    }

    /// The property an `AcObjProp` code names: the word after `).`.
    fn property(&self) -> Option<ObjectProperty> {
        let (_, after) = self.code.split_once(").")?;
        let word = after
            .split(|c: char| c.is_whitespace() || c == '\\')
            .next()?;
        ObjectProperty::from_name(word)
    }

    /// The field brought up to date with `doc` and `context`. Child fields
    /// are not evaluated (neither evaluator has any).
    pub fn evaluated(&self, doc: &Document, context: &FieldContext) -> DataField {
        let outcome = match self.evaluator.as_str() {
            OBJECT_PROPERTY => self.show_property(doc),
            VARIABLE => self.show_variable(context),
            _ => return self.clone(),
        };
        match outcome {
            Ok(text) => DataField {
                cached: Some(FieldValue::Text(text)),
                status: FieldStatus::Success,
                ..self.clone()
            },
            Err(status) => DataField {
                status,
                ..self.clone()
            },
        }
    }

    fn show_property(&self, doc: &Document) -> Result<String, FieldStatus> {
        let property = self.property().ok_or(FieldStatus::ParseError)?;
        let Some(FieldObjectRef::Entity(id)) = self.object_refs.first() else {
            return Err(FieldStatus::EvaluationFailed);
        };
        let object = doc.object(*id).ok_or(FieldStatus::EvaluationFailed)?;
        let text = match measure(&object.entity, property) {
            Some(Measured::Length(v)) => format_length(doc.units(), v),
            Some(Measured::Area(v)) => format_area(doc.units(), v),
            Some(Measured::Text(s)) => s,
            None if property == ObjectProperty::Layer => doc
                .layer(object.attrs.layer)
                .map(|l| l.name.clone())
                .ok_or(FieldStatus::EvaluationFailed)?,
            None => return Err(FieldStatus::EvaluationFailed),
        };
        (text.len() <= MAX_TEXT_BYTES)
            .then_some(text)
            .ok_or(FieldStatus::EvaluationFailed)
    }

    fn show_variable(&self, context: &FieldContext) -> Result<String, FieldStatus> {
        let name = self
            .code
            .trim_start_matches('\\')
            .strip_prefix(VARIABLE)
            .map(|rest| rest.split('\\').next().unwrap_or("").trim());
        let format = self.format_code().unwrap_or("");
        match name {
            Some(n) if n.eq_ignore_ascii_case(FILE_NAME) => context
                .file_name
                .as_deref()
                .map(|name| apply_case(name, format))
                .ok_or(FieldStatus::EvaluationFailed),
            Some(n) if n.eq_ignore_ascii_case("Date") => context
                .now
                .map(|t| {
                    let pattern = if format.is_empty() {
                        DEFAULT_DATE
                    } else {
                        format
                    };
                    format_time(&t, pattern)
                })
                .ok_or(FieldStatus::EvaluationFailed),
            Some(n) => context
                .properties
                .named(n)
                .map(|value| apply_case(value, format))
                .ok_or(FieldStatus::Unsupported),
            None => Err(FieldStatus::ParseError),
        }
    }
}

impl FieldText {
    /// The text with every field brought up to date.
    pub fn evaluated(&self, doc: &Document, context: &FieldContext) -> FieldText {
        match self {
            Self::Literal(_) => self.clone(),
            Self::Runs(runs) => Self::Runs(
                runs.iter()
                    .map(|r| match r {
                        FieldRun::Field(f) => FieldRun::Field(f.evaluated(doc, context)),
                        FieldRun::Literal(_) => r.clone(),
                    })
                    .collect(),
            ),
        }
    }
}

/// What an update did.
#[derive(Debug, Default, PartialEq)]
pub struct FieldUpdate {
    /// The inserts to replace, with their new contents.
    pub edits: Vec<(EntityId, Entity)>,
    /// Fields looked at, and fields whose value or status changed.
    pub found: usize,
    pub changed: usize,
    /// Fields that could not be evaluated now.
    pub failed: usize,
}

/// The data fields of the objects `ids` that hold any, evaluated: the
/// attributes of block references and multiline text. Other objects, and
/// ones without fields, are passed over.
pub fn update_fields(doc: &Document, ids: &[EntityId], context: &FieldContext) -> FieldUpdate {
    let mut update = FieldUpdate::default();
    for id in ids {
        let edited = match doc.get(*id) {
            Some(Entity::Insert(insert)) => update_insert(doc, insert, context, &mut update),
            Some(Entity::MText(text)) if !text.fields.is_empty() => {
                let after = text.evaluated(doc, context);
                text.tally(&after, &mut update);
                (after != **text).then(|| Entity::MText(Box::new(after)))
            }
            _ => None,
        };
        if let Some(entity) = edited {
            update.edits.push((*id, entity));
        }
    }
    update
}

/// `insert` with its attributes' fields evaluated, if that changes it.
fn update_insert(
    doc: &Document,
    insert: &crate::block::Insert,
    context: &FieldContext,
    update: &mut FieldUpdate,
) -> Option<Entity> {
    let mut changed = insert.clone();
    let mut touched = false;
    for r in &mut changed.attributes {
        if !r.value.has_fields() {
            continue;
        }
        let value = r.value.evaluated(doc, context);
        update.found += r.value.fields().count();
        update.failed += value
            .fields()
            .filter(|f| f.status == FieldStatus::EvaluationFailed)
            .count();
        update.changed += r
            .value
            .fields()
            .zip(value.fields())
            .filter(|(before, after)| before != after)
            .count();
        if value != r.value {
            r.value = value;
            touched = true;
        }
    }
    touched.then(|| Entity::Insert(Box::new(changed)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Point;

    fn doc_with(entity: Entity) -> (Document, EntityId) {
        let mut doc = Document::default();
        let id = doc.add(entity);
        (doc, id)
    }

    fn shown(field: &DataField) -> String {
        field.display()
    }

    #[test]
    fn a_circle_shows_its_radius_diameter_circumference_and_area() {
        let (doc, id) = doc_with(Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: 2.0,
        });
        let ctx = FieldContext::default();
        let value = |p| shown(&DataField::object_property(id, p).evaluated(&doc, &ctx));
        assert_eq!(
            value(ObjectProperty::Radius),
            format_length(doc.units(), 2.0)
        );
        assert_eq!(
            value(ObjectProperty::Diameter),
            format_length(doc.units(), 4.0)
        );
        assert_eq!(
            value(ObjectProperty::Area),
            format_area(doc.units(), std::f64::consts::PI * 4.0)
        );
        assert_eq!(
            value(ObjectProperty::Circumference),
            format_length(doc.units(), std::f64::consts::TAU * 2.0)
        );
    }

    #[test]
    fn lines_arcs_rectangles_and_polylines_have_a_length() {
        let ctx = FieldContext::default();
        for (entity, expected) in [
            (
                Entity::Line {
                    start: Point::new(0.0, 0.0),
                    end: Point::new(3.0, 4.0),
                },
                5.0,
            ),
            (
                Entity::Rectangle {
                    min: Point::new(0.0, 0.0),
                    max: Point::new(2.0, 3.0),
                },
                10.0,
            ),
            (
                Entity::Arc {
                    center: Point::new(0.0, 0.0),
                    radius: 2.0,
                    start: 0.0,
                    end: std::f64::consts::PI,
                },
                2.0 * std::f64::consts::PI,
            ),
            (
                Entity::Polyline {
                    points: vec![
                        Point::new(0.0, 0.0),
                        Point::new(3.0, 0.0),
                        Point::new(3.0, 4.0),
                    ],
                    closed: false,
                    bulges: Vec::new(),
                    widths: Vec::new(),
                },
                7.0,
            ),
        ] {
            let (doc, id) = doc_with(entity);
            let field =
                DataField::object_property(id, ObjectProperty::Length).evaluated(&doc, &ctx);
            assert_eq!(field.status, FieldStatus::Success);
            assert_eq!(shown(&field), format_length(doc.units(), expected));
        }
    }

    #[test]
    fn a_property_the_object_lacks_keeps_the_last_value_and_fails() {
        let (doc, id) = doc_with(Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(1.0, 0.0),
        });
        let mut field = DataField::object_property(id, ObjectProperty::Radius);
        field.cached = Some(FieldValue::Text("old".into()));
        let after = field.evaluated(&doc, &FieldContext::default());
        assert_eq!(after.status, FieldStatus::EvaluationFailed);
        assert_eq!(shown(&after), "old");
    }

    #[test]
    fn an_erased_object_fails_and_keeps_its_value() {
        let (mut doc, id) = doc_with(Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: 1.0,
        });
        let ctx = FieldContext::default();
        let shown_once =
            DataField::object_property(id, ObjectProperty::Radius).evaluated(&doc, &ctx);
        doc.remove(id);
        let after = shown_once.evaluated(&doc, &ctx);
        assert_eq!(after.status, FieldStatus::EvaluationFailed);
        assert_eq!(shown(&after), shown(&shown_once));
    }

    #[test]
    fn layer_is_the_name_of_the_objects_layer() {
        let (doc, id) = doc_with(Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: 1.0,
        });
        let field = DataField::object_property(id, ObjectProperty::Layer)
            .evaluated(&doc, &FieldContext::default());
        assert_eq!(shown(&field), "0");
    }

    #[test]
    fn the_file_name_comes_from_the_context_and_is_unavailable_before_a_save() {
        let doc = Document::default();
        let named = FieldContext {
            file_name: Some("plan.odx".into()),
            ..FieldContext::default()
        };
        let field = DataField::file_name().evaluated(&doc, &named);
        assert_eq!(shown(&field), "plan.odx");
        let unsaved = DataField::file_name().evaluated(&doc, &FieldContext::default());
        assert_eq!(unsaved.status, FieldStatus::EvaluationFailed);
        assert_eq!(shown(&unsaved), crate::block::field::NO_VALUE);
    }

    #[test]
    fn an_evaluator_this_build_lacks_is_left_alone() {
        let field = DataField::new(
            "AcSm",
            "\\AcSm SheetSet",
            Some(FieldValue::Text("x".into())),
        );
        let after = field.evaluated(&Document::default(), &FieldContext::default());
        assert_eq!(after, field);
        let other = DataField::new(VARIABLE, "\\AcVar Nonesuch", None)
            .evaluated(&Document::default(), &FieldContext::default());
        assert_eq!(other.status, FieldStatus::Unsupported);
    }

    #[test]
    fn a_code_without_a_property_is_a_parse_error() {
        let mut field = DataField::object_property(EntityId(1), ObjectProperty::Length);
        field.code = "\\AcObjProp nonsense".into();
        let after = field.evaluated(&Document::default(), &FieldContext::default());
        assert_eq!(after.status, FieldStatus::ParseError);
    }

    #[test]
    fn properties_offered_follow_the_object() {
        let circle = Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: 1.0,
        };
        assert!(ObjectProperty::of(&circle).contains(&ObjectProperty::Area));
        assert!(!ObjectProperty::of(&circle).contains(&ObjectProperty::Length));
        assert!(ObjectProperty::from_name("aREA") == Some(ObjectProperty::Area));
    }

    #[test]
    fn multiline_text_fields_are_updated_with_the_inserts() {
        let mut text =
            crate::MText::new(Point::new(0.0, 0.0), 2.5, "", crate::TextStyleId::STANDARD);
        text.insert_field(0, DataField::file_name());
        let (doc, id) = doc_with(Entity::MText(Box::new(text)));
        let named = FieldContext {
            file_name: Some("plan.odx".into()),
            ..FieldContext::default()
        };
        let update = update_fields(&doc, &[id], &named);
        assert_eq!((update.found, update.changed, update.failed), (1, 1, 0));
        let Some((_, Entity::MText(after))) = update.edits.first() else {
            panic!("one edit")
        };
        assert_eq!(after.field_values(), ["plan.odx"]);
        // Nothing to do the second time.
        let mut again = doc.clone();
        again.replace(id, Entity::MText(after.clone()));
        assert!(update_fields(&again, &[id], &named).edits.is_empty());
    }
}

#[cfg(test)]
mod date_tests {
    use super::*;
    use crate::block::field_kinds::FieldTime;

    const T: FieldTime = FieldTime {
        year: 2026,
        month: 11,
        day: 1,
        weekday: 6,
        hour: 9,
        minute: 5,
        second: 0,
    };

    fn context(file: Option<&str>) -> FieldContext {
        FieldContext {
            file_name: file.map(str::to_owned),
            now: Some(T),
            ..FieldContext::default()
        }
    }

    #[test]
    fn the_date_field_shows_the_time_now_in_its_format() {
        let doc = Document::default();
        let show = |format| {
            DataField::variable("Date", format)
                .evaluated(&doc, &context(None))
                .display()
        };
        assert_eq!(show(""), "11/1/2026");
        assert_eq!(show("yyyy-MM-dd HH:mm"), "2026-11-01 09:05");
    }

    #[test]
    fn a_date_without_a_clock_fails_and_keeps_its_value() {
        let after = DataField::variable("Date", "")
            .evaluated(&Document::default(), &FieldContext::default());
        assert_eq!(after.status, FieldStatus::EvaluationFailed);
    }

    #[test]
    fn the_file_name_takes_a_case_format() {
        let shown = DataField::variable("Filename", "%tc1")
            .evaluated(&Document::default(), &context(Some("plan.odx")))
            .display();
        assert_eq!(shown, "PLAN.ODX");
    }
}

#[cfg(test)]
mod property_tests {
    use super::*;
    use crate::block::field_kinds::KINDS;

    fn described() -> FieldContext {
        FieldContext {
            properties: DrawingProperties {
                title: "Plot 7".into(),
                author: "ada lovelace".into(),
                ..DrawingProperties::default()
            },
            ..FieldContext::default()
        }
    }

    #[test]
    fn a_drawing_property_shows_its_value_and_takes_a_case() {
        let doc = Document::default();
        let show = |name, format| {
            DataField::variable(name, format)
                .evaluated(&doc, &described())
                .display()
        };
        assert_eq!(show("Title", ""), "Plot 7");
        assert_eq!(show("Author", "%tc4"), "Ada Lovelace");
        assert_eq!(
            show("author", "%tc1"),
            "ADA LOVELACE",
            "any case of the name"
        );
    }

    #[test]
    fn an_empty_property_is_an_empty_value_not_a_failure() {
        let field =
            DataField::variable("Subject", "").evaluated(&Document::default(), &described());
        assert_eq!(field.status, FieldStatus::Success);
        assert_eq!(field.display(), "");
    }

    #[test]
    fn every_document_field_the_dialog_lists_can_be_evaluated() {
        let doc = Document::default();
        for kind in KINDS.iter().filter(|k| k.name != "Object") {
            let field = DataField::of_kind(kind.name, "").unwrap();
            let after = field.evaluated(&doc, &described());
            assert_ne!(
                after.status,
                FieldStatus::Unsupported,
                "{} is listed but not evaluated",
                kind.name
            );
        }
    }
}
