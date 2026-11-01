//! The native format's description of attribute definitions, the references
//! an insert holds, and the data fields in their text.

use od_core::block::{
    AttributeDefinition, AttributeDefinitionId, AttributeFlags, AttributeReference, AttributeText,
    DataField, FieldObjectRef, FieldRun, FieldStatus, FieldText, FieldValue, TextHorizontal,
    TextVertical,
};
use od_core::{
    Attributes, BlockId, Color, EntityId, LayerId, LayoutId, LinetypeId, Lineweight, TextStyleId,
};
use serde::{Deserialize, Serialize};

use super::entity::{from_point, invalid, point, PointDto};
use super::{attr_from_dto, attr_to_dto, AttrDto, ColorDto};
use crate::FileError;

fn one() -> f64 {
    1.0
}
fn is_zero(v: &f64) -> bool {
    *v == 0.0
}
fn is_one(v: &f64) -> bool {
    *v == 1.0
}
fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum HorizontalDto {
    Left,
    Center,
    Right,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum VerticalDto {
    Baseline,
    Bottom,
    Middle,
    Top,
}

#[derive(Serialize, Deserialize)]
pub(super) struct AttributeTextDto {
    position: PointDto,
    height: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    rotation: f64,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    width_factor: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    oblique: f64,
    style: u64,
    horizontal: HorizontalDto,
    vertical: VerticalDto,
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    generation_flags: u8,
}

impl AttributeTextDto {
    fn of(t: &AttributeText) -> Self {
        Self {
            position: point(t.position),
            height: t.height,
            rotation: t.rotation,
            width_factor: t.width_factor,
            oblique: t.oblique,
            style: t.style.0,
            horizontal: match t.horizontal {
                TextHorizontal::Left => HorizontalDto::Left,
                TextHorizontal::Center => HorizontalDto::Center,
                TextHorizontal::Right => HorizontalDto::Right,
            },
            vertical: match t.vertical {
                TextVertical::Baseline => VerticalDto::Baseline,
                TextVertical::Bottom => VerticalDto::Bottom,
                TextVertical::Middle => VerticalDto::Middle,
                TextVertical::Top => VerticalDto::Top,
            },
            generation_flags: t.generation_flags,
        }
    }

    fn text(&self) -> Result<AttributeText, FileError> {
        Ok(AttributeText {
            position: from_point(self.position)?,
            height: self.height,
            rotation: self.rotation,
            width_factor: self.width_factor,
            oblique: self.oblique,
            style: TextStyleId(self.style),
            horizontal: match self.horizontal {
                HorizontalDto::Left => TextHorizontal::Left,
                HorizontalDto::Center => TextHorizontal::Center,
                HorizontalDto::Right => TextHorizontal::Right,
            },
            vertical: match self.vertical {
                VerticalDto::Baseline => TextVertical::Baseline,
                VerticalDto::Bottom => TextVertical::Bottom,
                VerticalDto::Middle => TextVertical::Middle,
                VerticalDto::Top => TextVertical::Top,
            },
            generation_flags: self.generation_flags,
        })
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct AttributeDefinitionDto {
    id: u64,
    tag: String,
    prompt: String,
    default_value: FieldTextDto,
    #[serde(default)]
    flags: u16,
    #[serde(default)]
    lock_position: bool,
    #[serde(default)]
    field_length: u16,
    text: AttributeTextDto,
}

impl AttributeDefinitionDto {
    pub(super) fn of(d: &AttributeDefinition) -> Self {
        Self {
            id: d.id.0,
            tag: d.tag.clone(),
            prompt: d.prompt.clone(),
            default_value: FieldTextDto::of(&d.default_value),
            flags: d.flags.0,
            lock_position: d.lock_position,
            field_length: d.field_length,
            text: AttributeTextDto::of(&d.text),
        }
    }

    pub(super) fn to_definition(&self) -> Result<AttributeDefinition, FileError> {
        let d = AttributeDefinition {
            id: AttributeDefinitionId(self.id),
            tag: self.tag.clone(),
            prompt: self.prompt.clone(),
            default_value: self.default_value.text(),
            flags: AttributeFlags(self.flags),
            lock_position: self.lock_position,
            field_length: self.field_length,
            text: self.text.text()?,
        };
        d.validate()
            .or_else(|why| invalid(format!("attribute \"{}\": {why}", d.tag)))?;
        Ok(d)
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct AttributeReferenceDto {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<u64>,
    tag: String,
    value: FieldTextDto,
    #[serde(default)]
    flags: u16,
    #[serde(default)]
    lock_position: bool,
    #[serde(default)]
    field_length: u16,
    text: AttributeTextDto,
    /// Properties of its own; left out when it follows its definition.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    look: Option<AttributeLookDto>,
}

/// An attribute's own layer, colour, linetype and lineweight.
#[derive(Serialize, Deserialize)]
struct AttributeLookDto {
    layer: u64,
    color: AttrDto<ColorDto>,
    linetype: AttrDto<u64>,
    lineweight: AttrDto<u16>,
    #[serde(flatten)]
    scale: super::linetype::ScaleDto,
}

impl AttributeLookDto {
    fn of(a: &Attributes) -> Self {
        Self {
            layer: a.layer.0,
            color: attr_to_dto(&a.color, |c| (*c).into()),
            linetype: attr_to_dto(&a.linetype, |id| id.0),
            lineweight: attr_to_dto(&a.lineweight, |w| w.0),
            scale: super::linetype::ScaleDto::of(a),
        }
    }

    fn attributes(&self) -> Attributes {
        let mut a = Attributes {
            layer: LayerId(self.layer),
            color: attr_from_dto(self.color.clone(), Color::from),
            linetype: attr_from_dto(self.linetype.clone(), LinetypeId),
            lineweight: attr_from_dto(self.lineweight.clone(), Lineweight),
            ..Attributes::on_layer(LayerId(self.layer))
        };
        self.scale.apply(&mut a);
        a
    }
}

impl AttributeReferenceDto {
    pub(super) fn of(r: &AttributeReference) -> Self {
        Self {
            source: r.source.map(|id| id.0),
            tag: r.tag.clone(),
            value: FieldTextDto::of(&r.value),
            flags: r.flags.0,
            lock_position: r.lock_position,
            field_length: r.field_length,
            text: AttributeTextDto::of(&r.text),
            look: r.attrs.as_ref().map(AttributeLookDto::of),
        }
    }

    pub(super) fn to_reference(&self) -> Result<AttributeReference, FileError> {
        let r = AttributeReference {
            source: self.source.map(AttributeDefinitionId),
            tag: self.tag.clone(),
            value: self.value.text(),
            flags: AttributeFlags(self.flags),
            lock_position: self.lock_position,
            field_length: self.field_length,
            text: self.text.text()?,
            attrs: self.look.as_ref().map(AttributeLookDto::attributes),
        };
        r.validate()
            .or_else(|why| invalid(format!("attribute \"{}\": {why}", r.tag)))?;
        Ok(r)
    }
}

/// A string, or a list of strings and fields.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum FieldTextDto {
    Literal(String),
    Runs(#[serde(deserialize_with = "super::limits::field_items")] Vec<FieldRunDto>),
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum FieldRunDto {
    Literal(String),
    Field(Box<DataFieldDto>),
}

impl FieldTextDto {
    fn of(t: &FieldText) -> Self {
        match t {
            FieldText::Literal(s) => Self::Literal(s.clone()),
            FieldText::Runs(runs) => Self::Runs(
                runs.iter()
                    .map(|r| match r {
                        FieldRun::Literal(s) => FieldRunDto::Literal(s.clone()),
                        FieldRun::Field(f) => FieldRunDto::Field(Box::new(DataFieldDto::of(f))),
                    })
                    .collect(),
            ),
        }
    }

    fn text(&self) -> FieldText {
        match self {
            Self::Literal(s) => FieldText::Literal(s.clone()),
            Self::Runs(runs) => FieldText::Runs(
                runs.iter()
                    .map(|r| match r {
                        FieldRunDto::Literal(s) => FieldRun::Literal(s.clone()),
                        FieldRunDto::Field(f) => FieldRun::Field(f.field()),
                    })
                    .collect(),
            ),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StatusDto {
    Unevaluated,
    Success,
    Unsupported,
    ParseError,
    EvaluationFailed,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RefDto {
    Entity { id: u64 },
    AttributeDefinition { id: u64 },
    Block { id: u64 },
    Layout { id: u64 },
    TextStyle { id: u64 },
    Unresolved { token: String },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ValueDto {
    Text {
        value: String,
    },
    I32 {
        value: i32,
    },
    F64 {
        value: f64,
    },
    Id {
        value: u64,
    },
    Bytes {
        #[serde(deserialize_with = "super::limits::cached_bytes")]
        value: Vec<u8>,
    },
}

#[derive(Serialize, Deserialize)]
struct DataFieldDto {
    evaluator: String,
    code: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    format: String,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "super::limits::field_items"
    )]
    object_refs: Vec<RefDto>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "super::limits::field_items"
    )]
    children: Vec<DataFieldDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cached: Option<ValueDto>,
    status: StatusDto,
}

impl DataFieldDto {
    fn of(f: &DataField) -> Self {
        Self {
            evaluator: f.evaluator.clone(),
            code: f.code.clone(),
            format: f.format.clone(),
            object_refs: f
                .object_refs
                .iter()
                .map(|r| match r {
                    FieldObjectRef::Entity(id) => RefDto::Entity { id: id.0 },
                    FieldObjectRef::AttributeDefinition(id) => {
                        RefDto::AttributeDefinition { id: id.0 }
                    }
                    FieldObjectRef::Block(id) => RefDto::Block { id: id.0 },
                    FieldObjectRef::Layout(id) => RefDto::Layout { id: id.0 },
                    FieldObjectRef::TextStyle(id) => RefDto::TextStyle { id: id.0 },
                    FieldObjectRef::Unresolved(token) => RefDto::Unresolved {
                        token: token.clone(),
                    },
                })
                .collect(),
            children: f.children.iter().map(Self::of).collect(),
            cached: f.cached.as_ref().map(|v| match v {
                FieldValue::Text(value) => ValueDto::Text {
                    value: value.clone(),
                },
                FieldValue::I32(value) => ValueDto::I32 { value: *value },
                FieldValue::F64(value) => ValueDto::F64 { value: *value },
                FieldValue::Id(value) => ValueDto::Id { value: *value },
                FieldValue::Bytes(value) => ValueDto::Bytes {
                    value: value.clone(),
                },
            }),
            status: match f.status {
                FieldStatus::Unevaluated => StatusDto::Unevaluated,
                FieldStatus::Success => StatusDto::Success,
                FieldStatus::Unsupported => StatusDto::Unsupported,
                FieldStatus::ParseError => StatusDto::ParseError,
                FieldStatus::EvaluationFailed => StatusDto::EvaluationFailed,
            },
        }
    }

    fn field(&self) -> DataField {
        DataField {
            evaluator: self.evaluator.clone(),
            code: self.code.clone(),
            format: self.format.clone(),
            object_refs: self
                .object_refs
                .iter()
                .map(|r| match r {
                    RefDto::Entity { id } => FieldObjectRef::Entity(EntityId(*id)),
                    RefDto::AttributeDefinition { id } => {
                        FieldObjectRef::AttributeDefinition(AttributeDefinitionId(*id))
                    }
                    RefDto::Block { id } => FieldObjectRef::Block(BlockId(*id)),
                    RefDto::Layout { id } => FieldObjectRef::Layout(LayoutId(*id)),
                    RefDto::TextStyle { id } => FieldObjectRef::TextStyle(TextStyleId(*id)),
                    RefDto::Unresolved { token } => FieldObjectRef::Unresolved(token.clone()),
                })
                .collect(),
            children: self.children.iter().map(Self::field).collect(),
            cached: self.cached.as_ref().map(|v| match v {
                ValueDto::Text { value } => FieldValue::Text(value.clone()),
                ValueDto::I32 { value } => FieldValue::I32(*value),
                ValueDto::F64 { value } => FieldValue::F64(*value),
                ValueDto::Id { value } => FieldValue::Id(*value),
                ValueDto::Bytes { value } => FieldValue::Bytes(value.clone()),
            }),
            status: match self.status {
                StatusDto::Unevaluated => FieldStatus::Unevaluated,
                StatusDto::Success => FieldStatus::Success,
                StatusDto::Unsupported => FieldStatus::Unsupported,
                StatusDto::ParseError => FieldStatus::ParseError,
                StatusDto::EvaluationFailed => FieldStatus::EvaluationFailed,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use od_core::{
        Attributes, BlockDef, BlockEntity, Document, Entity, Insert, LayerId, Point, TextStyleId,
    };

    use super::*;
    use crate::{FileError, FileFormat, NativeFormat};

    fn definition(doc: &mut Document, tag: &str, flags: u16) -> AttributeDefinition {
        AttributeDefinition {
            id: doc.allocate_attribute_definition_id(),
            tag: tag.into(),
            prompt: format!("{tag}?"),
            default_value: FieldText::from("none"),
            flags: AttributeFlags(flags),
            lock_position: tag == "LOCKED",
            field_length: 7,
            text: AttributeText {
                horizontal: TextHorizontal::Center,
                vertical: TextVertical::Middle,
                rotation: 0.5,
                width_factor: 0.8,
                oblique: 0.1,
                generation_flags: 2,
                ..AttributeText::new(Point::new(1.0, 2.0), 3.0, TextStyleId::STANDARD)
            },
        }
    }

    fn field() -> DataField {
        DataField {
            evaluator: "AcVar".into(),
            code: r"\AcVar Filename".into(),
            format: "%tc1".into(),
            object_refs: vec![
                FieldObjectRef::Entity(EntityId(4)),
                FieldObjectRef::Unresolved("2A3F".into()),
            ],
            children: vec![DataField::new(
                "AcObjProp",
                "child",
                Some(FieldValue::F64(2.5)),
            )],
            cached: Some(FieldValue::Text("plan.dwg".into())),
            status: FieldStatus::Success,
        }
    }

    /// A block of three definitions, a constant one among them, and an
    /// insert holding a literal, a field, and an orphan.
    fn drawing() -> Document {
        let mut doc = Document::default();
        let defs = [
            definition(&mut doc, "NAME", 0),
            definition(&mut doc, "KIND", AttributeFlags::CONSTANT | 0x40),
            definition(&mut doc, "LOCKED", AttributeFlags::INVISIBLE),
        ];
        let block = Arc::new(BlockDef {
            unit: Default::default(),
            id: doc.allocate_block_id(),
            name: "Title".into(),
            base: Point::default(),
            description: String::new(),
            entities: defs
                .iter()
                .map(|d| BlockEntity {
                    entity: Entity::AttributeDefinition(Box::new(d.clone())),
                    attrs: Attributes::on_layer(LayerId::DEFAULT),
                })
                .collect(),
        });
        doc.push_block(block.clone());
        let mut insert = Insert::new(block.clone(), Point::new(5.0, 5.0));
        insert.attributes = block.new_references(|d| d.default_value.clone());
        insert.attributes[0].value = FieldText::Runs(vec![
            FieldRun::Literal("File ".into()),
            FieldRun::Field(field()),
        ]);
        let mut orphan = insert.attributes[1].clone();
        orphan.source = None;
        orphan.tag = "OLD".into();
        insert.attributes.push(orphan);
        doc.add(Entity::Insert(Box::new(insert)));
        let loose = definition(
            &mut doc,
            "LOOSE",
            AttributeFlags::VERIFY | AttributeFlags::PRESET,
        );
        doc.add(Entity::AttributeDefinition(Box::new(loose)));
        doc
    }

    fn read(file: &serde_json::Value) -> Result<Document, FileError> {
        NativeFormat
            .read(file.to_string().as_bytes())
            .map(|l| l.document)
    }

    fn as_json(doc: &Document) -> serde_json::Value {
        serde_json::from_slice(&NativeFormat.write(doc).unwrap().bytes).unwrap()
    }

    #[test]
    fn definitions_references_and_fields_survive_a_round_trip_exactly() {
        let doc = drawing();
        assert_eq!(doc.validate(), Ok(()));
        let back = read(&as_json(&doc)).unwrap();
        assert_eq!(back.blocks(), doc.blocks());
        assert_eq!(back.objects(), doc.objects());
        assert_eq!(
            back.next_attribute_definition_id(),
            doc.next_attribute_definition_id()
        );
        assert_eq!(back.validate(), Ok(()));
    }

    #[test]
    fn a_plain_value_is_a_json_string() {
        let json = as_json(&drawing());
        let insert = json["drawing"]["objects"][0]["entity"].clone();
        assert_eq!(insert["attributes"][1]["value"], "none");
        assert!(insert["attributes"][0]["value"].is_array());
        assert!(insert["attributes"][2].get("source").is_none());
    }

    fn refused(edit: impl FnOnce(&mut serde_json::Value)) -> bool {
        let mut json = as_json(&drawing());
        edit(&mut json);
        read(&json).is_err()
    }

    #[test]
    fn damaged_attributes_are_refused() {
        // A reference to a definition the block does not have.
        assert!(refused(|f| {
            f["drawing"]["objects"][0]["entity"]["attributes"][0]["source"] = 99.into();
        }));
        // A repeated source.
        assert!(refused(|f| {
            f["drawing"]["objects"][0]["entity"]["attributes"][1]["source"] =
                f["drawing"]["objects"][0]["entity"]["attributes"][0]["source"].clone();
        }));
        // A tag with a space, and a text with no height.
        assert!(refused(|f| {
            f["drawing"]["objects"][0]["entity"]["attributes"][0]["tag"] = "A B".into();
        }));
        assert!(refused(|f| {
            f["drawing"]["blocks"][0]["objects"][0]["entity"]["text"]["height"] = 0.into();
        }));
        // A constant reference.
        assert!(refused(|f| {
            f["drawing"]["objects"][0]["entity"]["attributes"][0]["flags"] = 2.into();
        }));
    }

    #[test]
    fn a_counter_behind_the_ids_in_use_is_brought_ahead_of_them() {
        let mut json = as_json(&drawing());
        json["drawing"]["next_attribute_definition_id"] = 1.into();
        let doc = read(&json).unwrap();
        assert_eq!(doc.validate(), Ok(()));
        assert!(doc.next_attribute_definition_id() > 4);
    }

    #[test]
    fn too_many_parts_in_a_field_are_refused_while_reading() {
        let part = serde_json::json!({"evaluator": "x", "code": "y", "status": "success"});
        assert!(refused(|f| {
            f["drawing"]["objects"][0]["entity"]["attributes"][0]["value"] =
                serde_json::Value::Array(vec![part.clone(); 4097]);
        }));
    }

    #[test]
    fn an_attributes_own_properties_round_trip_and_are_left_out_when_it_has_none() {
        use od_core::{Attr, Color, Layer};
        let mut doc = drawing();
        let marks = doc.allocate_layer_id();
        doc.push_layer(Layer::new(marks, "Marks"));
        let Some(Entity::Insert(i)) = doc
            .objects()
            .iter()
            .find_map(|o| match &o.entity {
                Entity::Insert(i) => Some(i.clone()),
                _ => None,
            })
            .map(Entity::Insert)
        else {
            panic!("an insert")
        };
        let id = doc.objects()[0].id;
        let mut changed = (*i).clone();
        changed.attributes[0].attrs = Some(Attributes {
            color: Attr::Set(Color::Rgb(1, 2, 3)),
            lineweight: Attr::ByBlock,
            ..Attributes::on_layer(marks)
        });
        doc.replace(id, Entity::Insert(Box::new(changed)));
        let json = as_json(&doc);
        let attributes = &json["drawing"]["objects"][0]["entity"]["attributes"];
        assert!(attributes[0]["look"].is_object());
        assert!(attributes[1].get("look").is_none());
        let back = read(&json).unwrap();
        assert_eq!(back.objects(), doc.objects());
        assert_eq!(back.validate(), Ok(()));
    }
}
