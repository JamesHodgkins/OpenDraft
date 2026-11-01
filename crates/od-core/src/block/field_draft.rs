//! What the Field dialog holds while it is open: the category and field
//! chosen, the format (or, for an object field, the property), and the
//! object picked. Pure, so a command can carry it to the dialog and back.

use crate::{Document, EntityId};

use super::field::DataField;
use super::field_eval::{FieldContext, ObjectProperty};
use super::field_kinds::{formats, FieldCategory, FieldKind, FormatChoice, KINDS, OBJECT};

/// An object picked for an object field, with what each of its properties
/// shows now.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectChoice {
    pub id: EntityId,
    /// What kind of object it is, for the dialog to name.
    pub label: String,
    pub properties: Vec<(ObjectProperty, String)>,
}

impl ObjectChoice {
    /// The object `id` of `doc`, `None` if there is no such object.
    pub fn of(doc: &Document, id: EntityId, context: &FieldContext) -> Option<Self> {
        let entity = doc.get(id)?;
        let properties = ObjectProperty::of(entity)
            .into_iter()
            .map(|p| {
                let shown = DataField::object_property(id, p)
                    .evaluated(doc, context)
                    .display();
                (p, shown)
            })
            .collect();
        Some(Self {
            id,
            label: entity.kind().display_name().to_owned(),
            properties,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldDraft {
    /// `None` is "All".
    pub category: Option<FieldCategory>,
    pub kind: &'static str,
    /// An index into [`Self::formats`]: 0 is `(none)`, or for an object
    /// field the property chosen.
    pub format: usize,
    pub object: Option<ObjectChoice>,
    /// Whether object fields are offered (the text editor cannot pick one).
    objects: bool,
}

impl FieldDraft {
    pub fn new(objects: bool) -> Self {
        Self {
            category: None,
            kind: KINDS[0].name,
            format: 0,
            object: None,
            objects,
        }
    }

    fn fields_in(
        &self,
        category: Option<FieldCategory>,
    ) -> impl Iterator<Item = &'static FieldKind> {
        let objects = self.objects;
        KINDS
            .iter()
            .filter(move |k| category.is_none_or(|c| c == k.category))
            .filter(move |k| objects || k.name != OBJECT)
    }

    /// The categories that list some field.
    pub fn categories(&self) -> Vec<FieldCategory> {
        FieldCategory::ALL
            .into_iter()
            .filter(|c| self.fields_in(Some(*c)).next().is_some())
            .collect()
    }

    /// The fields the category lists.
    pub fn listed(&self) -> Vec<&'static FieldKind> {
        self.fields_in(self.category).collect()
    }

    /// Show another category; the field stays chosen if it is still listed.
    pub fn select_category(&mut self, category: Option<FieldCategory>) {
        self.category = category;
        if !self.listed().iter().any(|k| k.name == self.kind) {
            if let Some(first) = self.listed().first() {
                self.kind = first.name;
            }
            self.format = 0;
        }
    }

    pub fn select_kind(&mut self, name: &'static str) {
        if self.kind != name {
            self.kind = name;
            self.format = 0;
        }
    }

    pub fn is_object(&self) -> bool {
        self.kind == OBJECT
    }

    /// The Format list: formats of the field, or the picked object's
    /// properties (none before an object is picked).
    pub fn formats(&self, context: &FieldContext) -> Vec<FormatChoice> {
        if !self.is_object() {
            return formats(self.kind, context);
        }
        self.object
            .iter()
            .flat_map(|o| &o.properties)
            .map(|(p, _)| FormatChoice {
                label: p.name().to_owned(),
                code: p.name().to_owned(),
            })
            .collect()
    }

    /// Use `choice` as the object, starting at its first property.
    pub fn pick(&mut self, choice: ObjectChoice) {
        self.object = Some(choice);
        self.format = 0;
    }

    /// What an object field's chosen property showed when the object was
    /// picked.
    pub fn object_value(&self) -> Option<&str> {
        let (_, value) = self.object.as_ref()?.properties.get(self.format)?;
        Some(value)
    }

    /// The field as chosen, not yet evaluated.
    pub fn field(&self, context: &FieldContext) -> Option<DataField> {
        if self.is_object() {
            let choice = self.object.as_ref()?;
            let (property, _) = choice.properties.get(self.format)?;
            return Some(DataField::object_property(choice.id, *property));
        }
        let code = formats(self.kind, context)
            .get(self.format)
            .map(|f| f.code.clone())?;
        DataField::of_kind(self.kind, &code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Entity, Point};

    fn circle_doc() -> (Document, EntityId) {
        let mut doc = Document::default();
        let id = doc.add(Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: 2.0,
        });
        (doc, id)
    }

    #[test]
    fn a_category_lists_only_its_fields_and_keeps_a_field_it_still_lists() {
        let mut d = FieldDraft::new(false);
        d.select_kind("Filename");
        d.format = 2;
        d.select_category(Some(FieldCategory::Document));
        assert_eq!((d.kind, d.format), ("Filename", 2), "still listed, kept");
        d.select_category(Some(FieldCategory::DateTime));
        assert_eq!((d.kind, d.format), ("Date", 0), "moved to the first listed");
        assert!(d
            .listed()
            .iter()
            .all(|k| k.category == FieldCategory::DateTime));
    }

    #[test]
    fn choosing_another_field_starts_its_format_afresh() {
        let mut d = FieldDraft {
            format: 3,
            ..FieldDraft::new(false)
        };
        d.select_kind("Date");
        assert_eq!(d.format, 3, "the same field keeps its format");
        d.select_kind("Filename");
        assert_eq!(d.format, 0);
    }

    #[test]
    fn objects_are_listed_only_where_they_can_be_picked() {
        let with = FieldDraft::new(true);
        let without = FieldDraft::new(false);
        assert!(with.listed().iter().any(|k| k.name == OBJECT));
        assert!(!without.listed().iter().any(|k| k.name == OBJECT));
        assert!(with.categories().contains(&FieldCategory::Objects));
        assert!(!without.categories().contains(&FieldCategory::Objects));
    }

    #[test]
    fn the_field_built_carries_the_format_chosen() {
        let context = FieldContext::default();
        let mut d = FieldDraft::new(false);
        assert_eq!(d.field(&context).unwrap().format_code(), None);
        d.select_kind("Filename");
        d.format = 1;
        let field = d.field(&context).unwrap();
        assert_eq!(field.expression(), "%<\\AcVar Filename \\f \"%tc1\">%");
        d.format = 99;
        assert!(d.field(&context).is_none());
    }

    #[test]
    fn an_object_field_needs_an_object_and_follows_the_property_chosen() {
        let (doc, id) = circle_doc();
        let context = FieldContext::default();
        let mut d = FieldDraft::new(true);
        d.select_kind(OBJECT);
        assert!(d.field(&context).is_none(), "no object yet");
        assert!(d.formats(&context).is_empty());
        d.pick(ObjectChoice::of(&doc, id, &context).unwrap());
        let names: Vec<_> = d.formats(&context).into_iter().map(|f| f.label).collect();
        assert!(names.contains(&"Radius".to_owned()) && names.contains(&"Layer".to_owned()));
        d.format = names.iter().position(|n| n == "Radius").unwrap();
        let field = d.field(&context).unwrap();
        assert_eq!(
            field.expression(),
            format!("%<\\AcObjProp Object(%<\\_ObjId {}>%).Radius>%", id.0)
        );
        assert_eq!(
            d.object_value(),
            Some(crate::units::format_length(doc.units(), 2.0).as_str())
        );
    }

    #[test]
    fn a_missing_object_is_not_a_choice() {
        let none = ObjectChoice::of(&Document::default(), EntityId(9), &FieldContext::default());
        assert!(none.is_none());
    }
}
