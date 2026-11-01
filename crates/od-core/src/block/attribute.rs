//! Block attributes: a definition (DXF `ATTDEF`) is a template text inside a
//! block; each insert of that block owns one reference (`ATTRIB`) per
//! non-constant definition, holding its own value.
//!
//! A reference is not an entity of its own: it belongs to its insert, is
//! saved, copied and erased with it, and is placed in the block's own
//! coordinates, so moving the insert moves the text. A reference remembers
//! which definition it came from by [`AttributeDefinitionId`], never by tag,
//! because tags may repeat. A constant definition has no references: the
//! definition itself is drawn by every insert.
//!
//! Text is single-line. Its look comes from the style, the height and the
//! justification; the width factor and oblique angle are kept and written to
//! files but not yet applied when drawing.

use std::sync::Arc;

use crate::block::field::FieldText;
use crate::mtext::{MText, MTextAttachment};
use crate::text::escape_for_contents;
use crate::{Attributes, Point, TextStyleId};

use super::{BlockDef, BlockEntity, Insert};
use crate::Entity;

/// Permanent identity of an attribute definition; never reused in a drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AttributeDefinitionId(pub u64);

/// Longest tag.
pub const MAX_TAG_BYTES: usize = 255;
/// Most references one insert may hold.
pub const MAX_REFERENCES: usize = 10_000;

/// DXF group 70: invisible 1, constant 2, verify 4, preset 8. Bits this build
/// does not know are kept.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AttributeFlags(pub u16);

impl AttributeFlags {
    pub const INVISIBLE: u16 = 1;
    pub const CONSTANT: u16 = 2;
    pub const VERIFY: u16 = 4;
    pub const PRESET: u16 = 8;

    pub fn invisible(self) -> bool {
        self.0 & Self::INVISIBLE != 0
    }
    pub fn constant(self) -> bool {
        self.0 & Self::CONSTANT != 0
    }
    pub fn verify(self) -> bool {
        self.0 & Self::VERIFY != 0
    }
    pub fn preset(self) -> bool {
        self.0 & Self::PRESET != 0
    }

    /// With `bit` set or cleared.
    pub fn with(self, bit: u16, on: bool) -> Self {
        Self(if on { self.0 | bit } else { self.0 & !bit })
    }
}

/// Where the text sits across its anchor (DXF group 72, the values drawn here).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextHorizontal {
    #[default]
    Left,
    Center,
    Right,
}

/// Where the text sits up and down its anchor (DXF group 74).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextVertical {
    #[default]
    Baseline,
    Bottom,
    Middle,
    Top,
}

/// How one line of attribute text is placed. For justified text `position`
/// is the point the text is justified on (what DXF calls the alignment point).
#[derive(Clone, Debug, PartialEq)]
pub struct AttributeText {
    pub position: Point,
    pub height: f64,
    /// Radians counter-clockwise.
    pub rotation: f64,
    pub width_factor: f64,
    /// Radians.
    pub oblique: f64,
    pub style: TextStyleId,
    pub horizontal: TextHorizontal,
    pub vertical: TextVertical,
    /// DXF group 71: 2 backward, 4 upside down.
    pub generation_flags: u8,
}

impl AttributeText {
    pub fn new(position: Point, height: f64, style: TextStyleId) -> Self {
        Self {
            position,
            height,
            rotation: 0.0,
            width_factor: 1.0,
            oblique: 0.0,
            style,
            horizontal: TextHorizontal::Left,
            vertical: TextVertical::Baseline,
            generation_flags: 0,
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        let finite = [
            self.position.x,
            self.position.y,
            self.rotation,
            self.oblique,
        ]
        .iter()
        .all(|v| v.is_finite());
        if !finite {
            return Err("the text position, rotation or slant is not a finite number");
        }
        if !self.height.is_finite() || self.height <= 0.0 {
            return Err("the text height must be above zero");
        }
        if !self.width_factor.is_finite() || self.width_factor <= 0.0 {
            return Err("the text width factor must be above zero");
        }
        Ok(())
    }

    fn attachment(&self) -> MTextAttachment {
        use MTextAttachment::*;
        match (self.vertical, self.horizontal) {
            (TextVertical::Top, TextHorizontal::Left) => TopLeft,
            (TextVertical::Top, TextHorizontal::Center) => TopCenter,
            (TextVertical::Top, TextHorizontal::Right) => TopRight,
            (TextVertical::Middle, TextHorizontal::Left) => MiddleLeft,
            (TextVertical::Middle, TextHorizontal::Center) => MiddleCenter,
            (TextVertical::Middle, TextHorizontal::Right) => MiddleRight,
            (_, TextHorizontal::Left) => BottomLeft,
            (_, TextHorizontal::Center) => BottomCenter,
            (_, TextHorizontal::Right) => BottomRight,
        }
    }

    /// `text` as multiline text in the block's coordinates, which is how it
    /// is drawn, picked and measured. A baseline-justified line is drawn from
    /// its bottom edge.
    pub fn projection(&self, text: &str) -> MText {
        let mut m = MText::new(
            self.position,
            self.height,
            escape_for_contents(text),
            self.style,
        );
        m.direction = Point::new(self.rotation.cos(), self.rotation.sin());
        m.attachment = self.attachment();
        m
    }
}

/// The template in a block, or a stray one in the drawing.
#[derive(Clone, Debug, PartialEq)]
pub struct AttributeDefinition {
    pub id: AttributeDefinitionId,
    pub tag: String,
    pub prompt: String,
    pub default_value: FieldText,
    pub flags: AttributeFlags,
    pub lock_position: bool,
    /// DXF group 73, kept and not used.
    pub field_length: u16,
    pub text: AttributeText,
}

impl AttributeDefinition {
    /// How the definition is drawn on its own: by its tag.
    pub fn tag_view(&self) -> MText {
        self.text.projection(&self.tag)
    }

    /// How a constant definition is drawn inside an insert: by its value.
    pub fn value_view(&self) -> MText {
        self.text.projection(&self.default_value.display())
    }

    pub fn validate(&self) -> Result<(), String> {
        if let Some(why) = tag_problem(&self.tag) {
            return Err(why.into());
        }
        if self.prompt.len() > super::field::MAX_TEXT_BYTES || self.prompt.contains('\0') {
            return Err("the prompt is too long or holds NUL".into());
        }
        self.default_value.validate()?;
        self.text.validate().map_err(String::from)
    }

    /// A reference to this definition holding `value`.
    pub fn reference(&self, value: FieldText) -> AttributeReference {
        AttributeReference {
            source: Some(self.id),
            tag: self.tag.clone(),
            value,
            flags: self.flags,
            lock_position: self.lock_position,
            field_length: self.field_length,
            text: self.text.clone(),
            attrs: None,
        }
    }
}

/// One insert's copy of a definition: its own value, in the block's
/// coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct AttributeReference {
    /// The definition it came from; `None` for one read from a file that
    /// matches no definition (an orphan).
    pub source: Option<AttributeDefinitionId>,
    pub tag: String,
    pub value: FieldText,
    pub flags: AttributeFlags,
    pub lock_position: bool,
    pub field_length: u16,
    pub text: AttributeText,
    /// Properties of this attribute alone (layer, colour, linetype,
    /// lineweight); `None` follows its definition, as a new one does.
    pub attrs: Option<Attributes>,
}

impl AttributeReference {
    /// The text as drawn, in the block's coordinates.
    pub fn view(&self) -> MText {
        self.text.projection(&self.value.display())
    }

    pub fn validate(&self) -> Result<(), String> {
        if let Some(why) = tag_problem(&self.tag) {
            return Err(why.into());
        }
        if self.flags.constant() {
            return Err("an insert cannot hold a constant attribute".into());
        }
        self.value.validate()?;
        self.text.validate().map_err(String::from)
    }
}

/// Why `tag` cannot be an attribute tag, if it cannot.
pub fn tag_problem(tag: &str) -> Option<&'static str> {
    if tag.is_empty() {
        Some("An attribute tag cannot be empty.")
    } else if tag.len() > MAX_TAG_BYTES {
        Some("An attribute tag is too long.")
    } else if tag.chars().any(|c| c.is_whitespace() || c.is_control()) {
        Some("An attribute tag cannot contain spaces.")
    } else if tag.contains('!') {
        Some("An attribute tag cannot contain \"!\".")
    } else {
        None
    }
}

/// A tag as the command line stores it: upper case.
pub fn normalized_tag(tag: &str) -> String {
    tag.to_uppercase()
}

impl BlockDef {
    /// The definitions placed directly in this block, in order, with the
    /// object each lives in.
    pub fn attribute_definitions(
        &self,
    ) -> impl Iterator<Item = (&BlockEntity, &AttributeDefinition)> {
        self.entities.iter().filter_map(|e| match &e.entity {
            Entity::AttributeDefinition(d) => Some((e, &**d)),
            _ => None,
        })
    }

    /// The object holding definition `id`.
    pub fn attribute_definition(&self, id: AttributeDefinitionId) -> Option<&BlockEntity> {
        self.attribute_definitions()
            .find(|(_, d)| d.id == id)
            .map(|(e, _)| e)
    }

    /// The references a new insert of this block starts with: one per
    /// non-constant definition, in order, each holding `answer(definition)`.
    pub fn new_references(
        &self,
        mut answer: impl FnMut(&AttributeDefinition) -> FieldText,
    ) -> Vec<AttributeReference> {
        self.attribute_definitions()
            .filter(|(_, d)| !d.flags.constant())
            .take(MAX_REFERENCES)
            .map(|(_, d)| d.reference(answer(d)))
            .collect()
    }

    /// Whether inserting this block asks for values.
    pub fn has_attributes(&self) -> bool {
        self.attribute_definitions().next().is_some()
    }
}

impl Insert {
    /// This insert pointing at `block`, a redefinition of its block. Its
    /// references stay as they are, except that one whose definition is gone
    /// becomes an orphan (no source), so nothing is lost until the user
    /// synchronises.
    pub fn rebound(&self, block: Arc<BlockDef>) -> Insert {
        let mut insert = self.clone();
        for r in &mut insert.attributes {
            let alive = r.source.is_some_and(|id| {
                block.attribute_definition(id).is_some_and(
                    |e| matches!(&e.entity, Entity::AttributeDefinition(d) if !d.flags.constant()),
                )
            });
            if !alive {
                r.source = None;
            }
        }
        insert.block = block;
        insert
    }
}

impl Insert {
    /// The properties of the text of `r`: its own, or those of its
    /// definition's object, or `own` (the insert's) for an orphan.
    pub fn attribute_attributes(&self, r: &AttributeReference, own: &Attributes) -> Attributes {
        traits_of(&self.block, r, own).clone()
    }
}

/// The traits a reference is drawn with: its own, else those of its
/// definition's object, else `fallback` (an orphan's).
pub(crate) fn traits_of<'a>(
    block: &'a Arc<BlockDef>,
    r: &'a AttributeReference,
    fallback: &'a Attributes,
) -> &'a Attributes {
    r.attrs
        .as_ref()
        .or_else(|| {
            r.source
                .and_then(|id| block.attribute_definition(id))
                .map(|e| &e.attrs)
        })
        .unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn definition(id: u64, tag: &str, flags: u16) -> AttributeDefinition {
        AttributeDefinition {
            id: AttributeDefinitionId(id),
            tag: tag.into(),
            prompt: format!("{tag}?"),
            default_value: FieldText::from("none"),
            flags: AttributeFlags(flags),
            lock_position: false,
            field_length: 0,
            text: AttributeText::new(Point::new(1.0, 1.0), 2.0, TextStyleId::STANDARD),
        }
    }

    #[test]
    fn tags_refuse_spaces_bang_and_nothing() {
        assert!(tag_problem("PART").is_none());
        assert!(tag_problem("").is_some());
        assert!(tag_problem("TWO WORDS").is_some());
        assert!(tag_problem("NO!").is_some());
        assert!(tag_problem(&"X".repeat(MAX_TAG_BYTES + 1)).is_some());
        assert_eq!(normalized_tag("part_no"), "PART_NO");
    }

    #[test]
    fn flags_keep_unknown_bits() {
        let f = AttributeFlags(0x40 | AttributeFlags::CONSTANT);
        assert!(f.constant() && !f.invisible());
        assert_eq!(f.with(AttributeFlags::CONSTANT, false).0, 0x40);
        assert_eq!(f.with(AttributeFlags::VERIFY, true).0, 0x46);
    }

    #[test]
    fn text_must_have_size() {
        let mut d = definition(1, "A", 0);
        assert!(d.validate().is_ok());
        d.text.height = 0.0;
        assert!(d.validate().is_err());
        d.text.height = 2.0;
        d.text.position.x = f64::NAN;
        assert!(d.validate().is_err());
    }

    #[test]
    fn a_reference_cannot_be_constant() {
        let d = definition(1, "A", AttributeFlags::CONSTANT);
        assert!(d.reference("x".into()).validate().is_err());
    }

    #[test]
    fn justification_chooses_the_attachment() {
        let mut t = AttributeText::new(Point::new(0.0, 0.0), 1.0, TextStyleId::STANDARD);
        assert_eq!(t.projection("a").attachment, MTextAttachment::BottomLeft);
        t.horizontal = TextHorizontal::Center;
        t.vertical = TextVertical::Middle;
        assert_eq!(t.projection("a").attachment, MTextAttachment::MiddleCenter);
        t.rotation = std::f64::consts::FRAC_PI_2;
        let d = t.projection("a").direction;
        assert!(d.x.abs() < 1e-12 && (d.y - 1.0).abs() < 1e-12);
    }

    #[test]
    fn projected_text_is_escaped() {
        let t = AttributeText::new(Point::new(0.0, 0.0), 1.0, TextStyleId::STANDARD);
        assert_eq!(t.projection("a{b}").contents, escape_for_contents("a{b}"));
    }

    #[test]
    fn new_references_skip_constants_and_keep_order() {
        let block = BlockDef {
            unit: Default::default(),
            id: crate::BlockId(1),
            name: "B".into(),
            base: Point::default(),
            description: String::new(),
            entities: [
                definition(1, "A", 0),
                definition(2, "C", AttributeFlags::CONSTANT),
                definition(3, "B", AttributeFlags::INVISIBLE),
            ]
            .into_iter()
            .map(|d| BlockEntity {
                entity: Entity::AttributeDefinition(Box::new(d)),
                attrs: Attributes::on_layer(crate::LayerId::DEFAULT),
            })
            .collect(),
        };
        let refs = block.new_references(|d| d.default_value.clone());
        let tags: Vec<_> = refs.iter().map(|r| r.tag.as_str()).collect();
        assert_eq!(tags, ["A", "B"]);
        assert_eq!(refs[1].source, Some(AttributeDefinitionId(3)));
        assert!(block.has_attributes());
    }
}
