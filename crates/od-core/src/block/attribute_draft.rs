//! What the Attribute Definition and Attribute Editor dialogs edit:
//! plain drafts of the values, checked and turned into the real thing in one
//! place, so the dialogs only draw.

use std::f64::consts::PI;

use crate::block::attribute::{
    normalized_tag, tag_problem, AttributeDefinition, AttributeDefinitionId, AttributeFlags,
    AttributeReference, AttributeText, TextHorizontal, TextVertical,
};
use crate::block::{FieldText, Insert};
use crate::{Attributes, Point, TextStyleId};

/// How text hangs on its point, from the dialog's list (the entries this
/// build can draw: aligned and fit text are not among them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Justification {
    Left,
    Center,
    Right,
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    MiddleCenter,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl Justification {
    pub const ALL: [Justification; 12] = [
        Self::Left,
        Self::Center,
        Self::Right,
        Self::TopLeft,
        Self::TopCenter,
        Self::TopRight,
        Self::MiddleLeft,
        Self::MiddleCenter,
        Self::MiddleRight,
        Self::BottomLeft,
        Self::BottomCenter,
        Self::BottomRight,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Left => "Left",
            Self::Center => "Center",
            Self::Right => "Right",
            Self::TopLeft => "Top Left",
            Self::TopCenter => "Top Center",
            Self::TopRight => "Top Right",
            Self::MiddleLeft => "Middle Left",
            Self::MiddleCenter => "Middle Center",
            Self::MiddleRight => "Middle Right",
            Self::BottomLeft => "Bottom Left",
            Self::BottomCenter => "Bottom Center",
            Self::BottomRight => "Bottom Right",
        }
    }

    pub fn parts(self) -> (TextHorizontal, TextVertical) {
        use TextHorizontal::{Center, Left, Right};
        use TextVertical::{Baseline, Bottom, Middle, Top};
        match self {
            Self::Left => (Left, Baseline),
            Self::Center => (Center, Baseline),
            Self::Right => (Right, Baseline),
            Self::TopLeft => (Left, Top),
            Self::TopCenter => (Center, Top),
            Self::TopRight => (Right, Top),
            Self::MiddleLeft => (Left, Middle),
            Self::MiddleCenter => (Center, Middle),
            Self::MiddleRight => (Right, Middle),
            Self::BottomLeft => (Left, Bottom),
            Self::BottomCenter => (Center, Bottom),
            Self::BottomRight => (Right, Bottom),
        }
    }

    pub fn from_parts(h: TextHorizontal, v: TextVertical) -> Self {
        Self::ALL
            .into_iter()
            .find(|j| j.parts() == (h, v))
            .unwrap_or(Self::Left)
    }
}

/// The Attribute Definition dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct DefinitionDraft {
    pub invisible: bool,
    pub constant: bool,
    pub verify: bool,
    pub preset: bool,
    pub lock_position: bool,
    pub tag: String,
    pub prompt: String,
    pub default_value: String,
    pub justification: Justification,
    pub style: TextStyleId,
    pub height: f64,
    /// Degrees counter-clockwise.
    pub rotation: f64,
    /// Whether the insertion point is picked on the screen afterwards.
    pub on_screen: bool,
    pub x: f64,
    pub y: f64,
}

impl DefinitionDraft {
    /// The dialog as it opens: the modes and text height last used.
    pub fn new(
        flags: AttributeFlags,
        lock_position: bool,
        height: f64,
        style: TextStyleId,
    ) -> Self {
        Self {
            invisible: flags.invisible(),
            constant: flags.constant(),
            verify: flags.verify(),
            preset: flags.preset(),
            lock_position,
            tag: String::new(),
            prompt: String::new(),
            default_value: String::new(),
            justification: Justification::Left,
            style,
            height,
            rotation: 0.0,
            on_screen: true,
            x: 0.0,
            y: 0.0,
        }
    }

    /// The modes, as stored. A constant attribute is never asked for, so it
    /// is neither verified nor preset.
    pub fn flags(&self) -> AttributeFlags {
        AttributeFlags(0)
            .with(AttributeFlags::INVISIBLE, self.invisible)
            .with(AttributeFlags::CONSTANT, self.constant)
            .with(AttributeFlags::VERIFY, self.verify && !self.constant)
            .with(AttributeFlags::PRESET, self.preset && !self.constant)
    }

    /// Why the dialog cannot be accepted, if it cannot.
    pub fn problem(&self) -> Option<String> {
        if let Some(why) = tag_problem(self.tag.trim()) {
            return Some(why.to_owned());
        }
        if !(self.height.is_finite() && self.height > 0.0) {
            return Some("The text height must be above zero.".to_owned());
        }
        if !self.rotation.is_finite() {
            return Some("The rotation is not a number.".to_owned());
        }
        if !(self.on_screen || (self.x.is_finite() && self.y.is_finite())) {
            return Some("The insertion point is not a number.".to_owned());
        }
        None
    }

    /// The point typed in, when it is not picked.
    pub fn typed_point(&self) -> Point {
        Point::new(self.x, self.y)
    }

    /// The definition this describes, with the tag in upper case.
    pub fn definition(&self, id: AttributeDefinitionId, at: Point) -> AttributeDefinition {
        let (horizontal, vertical) = self.justification.parts();
        let mut text = AttributeText::new(at, self.height, self.style);
        text.rotation = crate::arc::normalize(self.rotation.to_radians());
        text.horizontal = horizontal;
        text.vertical = vertical;
        AttributeDefinition {
            id,
            tag: normalized_tag(self.tag.trim()),
            prompt: if self.constant {
                String::new()
            } else {
                self.prompt.clone()
            },
            default_value: FieldText::Literal(self.default_value.clone()),
            flags: self.flags(),
            lock_position: self.lock_position,
            field_length: 0,
            text,
        }
    }
}

/// One attribute's text settings in the Attribute Editor.
#[derive(Clone, Debug, PartialEq)]
pub struct TextOptions {
    pub style: TextStyleId,
    pub justification: Justification,
    pub backwards: bool,
    pub upside_down: bool,
    pub height: f64,
    pub width_factor: f64,
    /// Degrees.
    pub rotation: f64,
    /// Degrees.
    pub oblique: f64,
}

impl TextOptions {
    fn of(t: &AttributeText) -> Self {
        Self {
            style: t.style,
            justification: Justification::from_parts(t.horizontal, t.vertical),
            backwards: t.generation_flags & 2 != 0,
            upside_down: t.generation_flags & 4 != 0,
            height: t.height,
            width_factor: t.width_factor,
            rotation: t.rotation.to_degrees(),
            oblique: t.oblique.to_degrees(),
        }
    }

    fn write(&self, t: &mut AttributeText) {
        let (horizontal, vertical) = self.justification.parts();
        t.style = self.style;
        t.horizontal = horizontal;
        t.vertical = vertical;
        t.generation_flags = (t.generation_flags & !6)
            | if self.backwards { 2 } else { 0 }
            | if self.upside_down { 4 } else { 0 };
        t.height = self.height;
        t.width_factor = self.width_factor;
        t.rotation = crate::arc::normalize(self.rotation.to_radians());
        t.oblique = (self.oblique.to_radians()).clamp(-PI / 2.0 + 1e-6, PI / 2.0 - 1e-6);
    }
}

/// One row of the attribute list, with the settings of the other tabs.
#[derive(Clone, Debug, PartialEq)]
pub struct EditorRow {
    pub tag: String,
    pub prompt: String,
    /// What is shown and edited. A data field is shown and not edited.
    pub value: String,
    pub is_field: bool,
    /// Whether no definition of the block stands behind it.
    pub orphan: bool,
    pub text: TextOptions,
    pub look: Attributes,
}

/// The Attribute Editor.
#[derive(Clone, Debug, PartialEq)]
pub struct EditorDraft {
    pub block: String,
    pub rows: Vec<EditorRow>,
    original: Vec<EditorRow>,
    pub selected: usize,
    /// The tab: 0 Attribute, 1 Text Options, 2 Properties.
    pub tab: usize,
}

impl EditorDraft {
    /// The editor on `insert`, whose own properties are `own`.
    pub fn new(insert: &Insert, own: &Attributes) -> Self {
        let rows: Vec<EditorRow> = insert
            .attributes
            .iter()
            .map(|r| {
                let definition = r
                    .source
                    .and_then(|id| insert.block.attribute_definition(id))
                    .and_then(|e| match &e.entity {
                        crate::Entity::AttributeDefinition(d) => Some(d),
                        _ => None,
                    });
                EditorRow {
                    tag: r.tag.clone(),
                    prompt: definition.map_or_else(|| r.tag.clone(), |d| d.prompt.clone()),
                    value: r.value.display(),
                    is_field: r.value.has_fields(),
                    orphan: definition.is_none(),
                    text: TextOptions::of(&r.text),
                    look: insert.attribute_attributes(r, own),
                }
            })
            .collect();
        Self {
            block: insert.block.name.clone(),
            original: rows.clone(),
            rows,
            selected: 0,
            tab: 0,
        }
    }

    /// Whether anything was changed since it opened (Apply is for this).
    pub fn is_changed(&self) -> bool {
        self.rows != self.original
    }

    /// Why the editor cannot be accepted, if it cannot.
    pub fn problem(&self) -> Option<String> {
        self.rows.iter().find_map(|row| {
            let t = &row.text;
            let bad = !(t.height.is_finite() && t.height > 0.0)
                || !(t.width_factor.is_finite() && t.width_factor > 0.0)
                || !(t.rotation.is_finite() && t.oblique.is_finite());
            bad.then(|| format!("The text settings of {} are not valid.", row.tag))
        })
    }

    /// `insert` with the changes made to the rows, or `None` when nothing
    /// was changed. Only a row that was changed is touched. A look equal to
    /// what the attribute would follow anyway is stored as "follows".
    pub fn apply(&self, insert: &Insert, own: &Attributes) -> Option<Insert> {
        if !self.is_changed() || self.rows.len() != insert.attributes.len() {
            return None;
        }
        let mut out = insert.clone();
        for (i, (row, before)) in self.rows.iter().zip(&self.original).enumerate() {
            if row == before {
                continue;
            }
            let mut r: AttributeReference = insert.attributes[i].clone();
            if row.value != before.value && !row.is_field {
                r.value = FieldText::Literal(row.value.clone());
            }
            if row.text != before.text {
                row.text.write(&mut r.text);
            }
            if row.look != before.look {
                r.attrs = Some(row.look.clone());
                let follows = AttributeReference {
                    attrs: None,
                    ..r.clone()
                };
                if insert.attribute_attributes(&follows, own) == row.look {
                    r.attrs = None;
                }
            }
            out.attributes[i] = r;
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::block::{BlockDef, BlockEntity};
    use crate::{Attr, Color, Entity, LayerId};

    fn definition(id: u64, tag: &str, prompt: &str) -> BlockEntity {
        let d = DefinitionDraft {
            tag: tag.into(),
            prompt: prompt.into(),
            default_value: "dflt".into(),
            ..DefinitionDraft::new(AttributeFlags(0), false, 2.0, TextStyleId::STANDARD)
        }
        .definition(AttributeDefinitionId(id), Point::new(0.0, 1.0));
        BlockEntity {
            entity: Entity::AttributeDefinition(Box::new(d)),
            attrs: Attributes::on_layer(LayerId::DEFAULT),
        }
    }

    fn insert() -> Insert {
        let block = Arc::new(BlockDef {
            unit: Default::default(),
            id: crate::BlockId(1),
            name: "Door".into(),
            base: Point::default(),
            description: String::new(),
            entities: vec![definition(1, "A", "A?"), definition(2, "B", "B?")],
        });
        let mut i = Insert::new(block.clone(), Point::new(5.0, 5.0));
        i.attributes = block.new_references(|d| d.default_value.clone());
        i
    }

    #[test]
    fn a_definition_needs_a_tag_and_a_height_and_a_constant_is_neither_verified_nor_preset() {
        let mut d = DefinitionDraft::new(AttributeFlags(0), true, 1.0, TextStyleId::STANDARD);
        assert!(d.problem().is_some(), "no tag");
        d.tag = "two words".into();
        assert!(d.problem().is_some());
        d.tag = "part_no".into();
        assert_eq!(d.problem(), None);
        d.height = 0.0;
        assert!(d.problem().is_some());
        d.height = 2.5;
        d.verify = true;
        d.preset = true;
        d.constant = true;
        d.prompt = "ignored".into();
        let def = d.definition(AttributeDefinitionId(4), Point::new(1.0, 2.0));
        assert_eq!(def.tag, "PART_NO");
        assert_eq!(def.flags.0, AttributeFlags::CONSTANT);
        assert_eq!(def.prompt, "", "a constant has no prompt");
        assert!(def.lock_position);
    }

    #[test]
    fn justification_maps_both_ways() {
        for j in Justification::ALL {
            let (h, v) = j.parts();
            assert_eq!(Justification::from_parts(h, v), j, "{}", j.label());
        }
        let mut d = DefinitionDraft::new(AttributeFlags(0), false, 1.0, TextStyleId::STANDARD);
        d.tag = "A".into();
        d.justification = Justification::MiddleCenter;
        d.rotation = 90.0;
        let def = d.definition(AttributeDefinitionId(1), Point::default());
        assert_eq!(
            (def.text.horizontal, def.text.vertical),
            (TextHorizontal::Center, TextVertical::Middle)
        );
        assert!((def.text.rotation - PI / 2.0).abs() < 1e-12);
    }

    #[test]
    fn the_editor_lists_the_attributes_with_their_prompts_and_changes_nothing_until_edited() {
        let i = insert();
        let own = Attributes::on_layer(LayerId::DEFAULT);
        let draft = EditorDraft::new(&i, &own);
        assert_eq!(draft.block, "Door");
        let tags: Vec<_> = draft.rows.iter().map(|r| (&*r.tag, &*r.prompt)).collect();
        assert_eq!(tags, [("A", "A?"), ("B", "B?")]);
        assert!(!draft.is_changed());
        assert_eq!(draft.apply(&i, &own), None);
    }

    #[test]
    fn editing_a_value_changes_that_attribute_only() {
        let i = insert();
        let own = Attributes::on_layer(LayerId::DEFAULT);
        let mut draft = EditorDraft::new(&i, &own);
        draft.rows[1].value = "new".into();
        assert!(draft.is_changed());
        let out = draft.apply(&i, &own).unwrap();
        assert_eq!(out.attributes[0], i.attributes[0]);
        assert_eq!(out.attributes[1].value.display(), "new");
        assert_eq!(out.attributes[1].text, i.attributes[1].text);
    }

    #[test]
    fn text_options_and_properties_are_written_and_a_look_that_follows_is_not_stored() {
        let i = insert();
        let own = Attributes::on_layer(LayerId::DEFAULT);
        let mut draft = EditorDraft::new(&i, &own);
        let row = &mut draft.rows[0];
        row.text.height = 5.0;
        row.text.justification = Justification::BottomRight;
        row.text.backwards = true;
        row.text.rotation = 45.0;
        row.look.color = Attr::Set(Color::Rgb(9, 9, 9));
        let out = draft.apply(&i, &own).unwrap();
        let r = &out.attributes[0];
        assert_eq!(r.text.height, 5.0);
        assert_eq!(r.text.generation_flags & 2, 2);
        assert_eq!(
            (r.text.horizontal, r.text.vertical),
            (TextHorizontal::Right, TextVertical::Bottom)
        );
        assert!(r.attrs.is_some());
        // Setting the look back to what it follows stores nothing.
        let mut back = EditorDraft::new(&out, &own);
        back.rows[0].look = Attributes::on_layer(LayerId::DEFAULT);
        let out = back.apply(&out, &own).unwrap();
        assert_eq!(out.attributes[0].attrs, None);
    }

    #[test]
    fn a_data_field_is_shown_and_never_replaced_by_the_editor() {
        use crate::block::{DataField, FieldRun, FieldValue};
        let mut i = insert();
        let field = DataField::new("AcVar", "x", Some(FieldValue::Text("shown".into())));
        i.attributes[0].value = FieldText::Runs(vec![FieldRun::Field(field)]);
        let own = Attributes::on_layer(LayerId::DEFAULT);
        let mut draft = EditorDraft::new(&i, &own);
        assert!(draft.rows[0].is_field);
        assert_eq!(draft.rows[0].value, "shown");
        draft.rows[0].value = "typed".into();
        draft.rows[0].text.height = 3.0;
        let out = draft.apply(&i, &own).unwrap();
        assert!(out.attributes[0].value.has_fields(), "the field stays");
        assert_eq!(out.attributes[0].text.height, 3.0);
    }

    #[test]
    fn a_zero_height_or_width_is_a_problem() {
        let i = insert();
        let own = Attributes::on_layer(LayerId::DEFAULT);
        let mut draft = EditorDraft::new(&i, &own);
        assert_eq!(draft.problem(), None);
        draft.rows[0].text.width_factor = 0.0;
        assert!(draft.problem().unwrap().contains('A'));
    }
}
