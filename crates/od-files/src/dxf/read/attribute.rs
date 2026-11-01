//! Attributes from DXF: `ATTDEF` (a template in a block, or loose), and the
//! `ATTRIB` records that follow an `INSERT` whose group 66 is 1, up to its
//! `SEQEND`. The insert is held back until its `SEQEND`, so its attributes
//! can only ever belong to it; an insert whose sequence is cut short by any
//! other record is dropped and counted, never joined to the next one.
//!
//! An `ATTRIB` is placed in the drawing; `block_import` moves it into the
//! block's coordinates once the block is known. The multiline form, text
//! that is aligned or fitted, and text out of the XY plane are skipped by
//! name (a reference loses that value). `ACAD_FIELD` objects are not read:
//! an attribute that has extension data may hold a data field, and then it
//! comes in as its last text; a warning says how many.

use od_core::block::attribute::tag_problem;
use od_core::block::{
    AttributeDefinition, AttributeFlags, AttributeReference, AttributeText, FieldText,
    TextHorizontal, TextVertical,
};
use od_core::{Entity, Point, TextStyleId};

use super::{decode, Importer, Record};
use crate::plural;

/// An INSERT waiting for its attributes.
struct Open {
    record: Record,
    attributes: Vec<AttributeReference>,
}

#[derive(Default)]
pub(super) struct Reading {
    open: Option<Open>,
    /// Attributes with extension data, which may hold a data field.
    maybe_fields: usize,
    /// Constant attributes written on an insert, which holds none.
    constants: usize,
}

impl Reading {
    pub(super) fn is_open(&self) -> bool {
        self.open.is_some()
    }

    pub(super) fn warnings(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.constants > 0 {
            out.push(format!(
                "Skipped {} written on a block reference: a constant attribute is drawn from its definition.",
                plural(self.constants as u64, "constant attribute")
            ));
        }
        if self.maybe_fields == 0 {
            return out;
        }
        out.push(format!(
            "{} may hold data fields, which are not read: {} shown as the text it last had.",
            plural(self.maybe_fields as u64, "attribute"),
            if self.maybe_fields == 1 {
                "it is"
            } else {
                "they are"
            },
        ));
        out
    }
}

/// Why a record is not an attribute.
enum Problem {
    Unsupported(&'static str),
    Damaged,
}

impl Importer {
    /// The text settings of an `ATTDEF` or `ATTRIB`, in the file's
    /// coordinates.
    fn attribute_text(
        &self,
        record: &Record,
        kind: &'static str,
    ) -> Result<AttributeText, Problem> {
        let unsupported = |what: &'static str| Err(Problem::Unsupported(what));
        let normal = [
            record.number(210).unwrap_or(0.0),
            record.number(220).unwrap_or(0.0),
            record.number(230).unwrap_or(1.0),
        ];
        // A flat normal pointing down is a mirrored insert: the text's own
        // x axis runs the other way.
        let flipped = normal[2] < 0.0;
        let flat = normal[0].abs() <= 1e-9
            && normal[1].abs() <= 1e-9
            && (normal[2].abs() - 1.0).abs() <= 1e-9;
        if !flat {
            return unsupported(if kind == "ATTDEF" {
                "ATTDEF (not in the XY plane)"
            } else {
                "ATTRIB (not in the XY plane)"
            });
        }
        if record.groups.iter().any(|g| g.code == 101) {
            return unsupported(if kind == "ATTDEF" {
                "ATTDEF (multiline)"
            } else {
                "ATTRIB (multiline)"
            });
        }
        let (across, down) = (record.int(72).unwrap_or(0), record.int(74).unwrap_or(0));
        let (horizontal, vertical) = match (across, down) {
            (3 | 5, _) => {
                return unsupported(if kind == "ATTDEF" {
                    "ATTDEF (aligned or fit text)"
                } else {
                    "ATTRIB (aligned or fit text)"
                })
            }
            (4, _) => (TextHorizontal::Center, TextVertical::Middle),
            (h, v) => (
                match h {
                    0 => TextHorizontal::Left,
                    1 => TextHorizontal::Center,
                    2 => TextHorizontal::Right,
                    _ => return Err(Problem::Damaged),
                },
                match v {
                    0 => TextVertical::Baseline,
                    1 => TextVertical::Bottom,
                    2 => TextVertical::Middle,
                    3 => TextVertical::Top,
                    _ => return Err(Problem::Damaged),
                },
            ),
        };
        // Justified text is placed by its second point.
        let justified = across != 0 || down != 0;
        let position = if justified {
            Self::point(record, 11, 21).or_else(|| Self::point(record, 10, 20))
        } else {
            Self::point(record, 10, 20)
        }
        .ok_or(Problem::Damaged)?;
        let style = record
            .text(7)
            .and_then(|n| self.text_styles.by_name.get(&decode(n).to_lowercase()))
            .copied()
            .unwrap_or(TextStyleId::STANDARD);
        let (position, rotation) = if flipped {
            // OCS x is the drawing's -x: a point and an angle are reflected.
            let angle = record.number(50).unwrap_or(0.0).to_radians();
            (
                Point::new(-position.x, position.y),
                std::f64::consts::PI - angle,
            )
        } else {
            (position, record.number(50).unwrap_or(0.0).to_radians())
        };
        let text = AttributeText {
            position,
            height: record.number(40).unwrap_or(0.0),
            rotation: od_core::arc::normalize(rotation),
            width_factor: record.number(41).unwrap_or(1.0),
            oblique: record.number(51).unwrap_or(0.0).to_radians(),
            style,
            horizontal,
            vertical,
            generation_flags: (record.int(71).unwrap_or(0) & 0xFF) as u8,
        };
        text.validate().map(|()| text).map_err(|_| Problem::Damaged)
    }

    /// Tag, flags, lock and field length, the same for both records.
    fn attribute_common(record: &Record) -> Option<(String, AttributeFlags, bool, u16)> {
        let tag = decode(record.text(2)?);
        tag_problem(&tag).is_none().then_some(())?;
        let flags = AttributeFlags(u16::try_from(record.int(70).unwrap_or(0)).ok()?);
        // AutoCAD writes a version 280 before the tag and the lock-position
        // 280 after it; other writers leave the version out. Only one after
        // the tag is the lock.
        let lock = record
            .groups
            .iter()
            .skip_while(|g| g.code != 2)
            .any(|g| g.code == 280 && g.value.trim() == "1");
        let length = u16::try_from(record.int(73).unwrap_or(0)).unwrap_or(0);
        Some((tag, flags, lock, length))
    }

    fn refuse(&mut self, problem: Problem) {
        match problem {
            Problem::Unsupported(what) => self.skip(what),
            Problem::Damaged => self.damaged += 1,
        }
    }

    /// An `ATTDEF`: in the block being read, or in the drawing.
    pub(super) fn attribute_definition(&mut self, record: &Record) {
        let text = match self.attribute_text(record, "ATTDEF") {
            Ok(text) => text,
            Err(problem) => return self.refuse(problem),
        };
        let Some((tag, flags, lock_position, field_length)) = Self::attribute_common(record) else {
            self.damaged += 1;
            return;
        };
        let definition = AttributeDefinition {
            id: self.document.allocate_attribute_definition_id(),
            prompt: decode(record.text(3).unwrap_or("")),
            default_value: FieldText::Literal(decode(record.text(1).unwrap_or(""))),
            tag,
            flags,
            lock_position,
            field_length,
            text,
        };
        if definition.validate().is_err() {
            self.damaged += 1;
            return;
        }
        self.add(record, Entity::AttributeDefinition(Box::new(definition)));
    }

    /// The records that belong to attributes: `ATTDEF`, `ATTRIB`, and the
    /// `SEQEND` of an open `INSERT`. Any other record cuts an open `INSERT`'s
    /// attributes short, which drops it. `false` when `record` is not theirs.
    pub(super) fn attribute_record(&mut self, record: &Record) -> bool {
        let name = record.name.as_str();
        if self.blocks.attributes.is_open() && !matches!(name, "ATTRIB" | "SEQEND") {
            self.drop_open_insert();
        }
        match name {
            "ATTDEF" => self.attribute_definition(record),
            "ATTRIB" => self.attrib(record),
            "SEQEND" if self.blocks.attributes.is_open() => self.finish_open_insert(),
            _ => return false,
        }
        true
    }

    /// Holds an `INSERT` with group 66 until its `SEQEND`.
    pub(super) fn open_insert(&mut self, record: &Record) {
        self.drop_open_insert();
        self.blocks.attributes.open = Some(Open {
            record: record.clone(),
            attributes: Vec::new(),
        });
    }

    /// An `ATTRIB` after an open `INSERT`.
    pub(super) fn attrib(&mut self, record: &Record) {
        if !self.blocks.attributes.is_open() {
            return self.skip("ATTRIB");
        }
        let text = match self.attribute_text(record, "ATTRIB") {
            Ok(text) => text,
            Err(problem) => return self.refuse(problem),
        };
        let common = Self::attribute_common(record);
        let Some((tag, flags, lock_position, field_length)) = common else {
            self.damaged += 1;
            return;
        };
        // An insert holds no constant attribute: the definition is drawn.
        if flags.constant() {
            self.blocks.attributes.constants += 1;
            return;
        }
        if record
            .groups
            .iter()
            .any(|g| g.code == 102 && g.value.starts_with("{ACAD_XDICTIONARY"))
        {
            self.blocks.attributes.maybe_fields += 1;
        }
        let reference = AttributeReference {
            source: None,
            tag,
            value: FieldText::Literal(decode(record.text(1).unwrap_or(""))),
            flags,
            lock_position,
            field_length,
            text,
            // `block_import` drops this when it is the definition's own.
            attrs: Some(self.attributes(record)),
        };
        if reference.validate().is_err() {
            self.damaged += 1;
        } else if let Some(open) = self.blocks.attributes.open.as_mut() {
            open.attributes.push(reference);
        }
    }

    /// The `SEQEND` of an open `INSERT`: the insert is placed.
    pub(super) fn finish_open_insert(&mut self) {
        if let Some(open) = self.blocks.attributes.open.take() {
            self.place_insert(&open.record, open.attributes);
        }
    }

    /// An open `INSERT` that was not closed by a `SEQEND` is damaged.
    pub(super) fn drop_open_insert(&mut self) {
        if self.blocks.attributes.open.take().is_some() {
            self.damaged += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use od_core::Entity;

    use super::super::read;

    type Pairs<'a> = Vec<(i32, &'a str)>;

    fn file(blocks: &[(i32, &str)], entities: &[(i32, &str)]) -> Vec<u8> {
        let mut pairs: Pairs = vec![(0, "SECTION"), (2, "BLOCKS")];
        pairs.extend_from_slice(blocks);
        pairs.extend([(0, "ENDSEC"), (0, "SECTION"), (2, "ENTITIES")]);
        pairs.extend_from_slice(entities);
        pairs.extend([(0, "ENDSEC"), (0, "EOF")]);
        let mut text = String::new();
        for (code, value) in pairs {
            text.push_str(&format!("{code:>3}\r\n{value}\r\n"));
        }
        text.into_bytes()
    }

    /// Block TAG: one ATTDEF called `A` at (1, 2).
    fn tag_block() -> Pairs<'static> {
        vec![
            (0, "BLOCK"),
            (8, "0"),
            (2, "TAG"),
            (70, "0"),
            (10, "0"),
            (20, "0"),
            (0, "ATTDEF"),
            (8, "0"),
            (10, "1"),
            (20, "2"),
            (40, "2"),
            (1, "dflt"),
            (3, "Value?"),
            (2, "A"),
            (70, "0"),
            (0, "ENDBLK"),
        ]
    }

    fn insert<'a>(at: &'a str, with_attributes: bool) -> Pairs<'a> {
        vec![
            (0, "INSERT"),
            (8, "0"),
            (66, if with_attributes { "1" } else { "0" }),
            (2, "TAG"),
            (10, at),
            (20, "0"),
        ]
    }

    fn attrib<'a>(value: &'a str, extra: &[(i32, &'a str)]) -> Pairs<'a> {
        let mut out = vec![
            (0, "ATTRIB"),
            (8, "0"),
            (10, "11"),
            (20, "22"),
            (40, "2"),
            (1, value),
            (2, "A"),
            (70, "0"),
        ];
        out.extend_from_slice(extra);
        out
    }

    fn values(doc: &od_core::Document) -> Vec<Vec<String>> {
        doc.model_objects()
            .filter_map(|o| match &o.entity {
                Entity::Insert(i) => Some(i.attributes.iter().map(|r| r.value.display()).collect()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn an_insert_and_its_attribute_come_in_and_the_text_is_moved_into_the_block() {
        let mut e = insert("100", true);
        e.extend(attrib("mine", &[]));
        e.push((0, "SEQEND"));
        let loaded = read(&file(&tag_block(), &e)).unwrap();
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
        loaded.document.validate().unwrap();
        let Entity::Insert(i) = &loaded.document.objects()[0].entity else {
            panic!("an insert")
        };
        // The file put it at (11, 22); the insert is at (100, 0).
        assert_eq!(
            i.attributes[0].text.position,
            od_core::Point::new(-89.0, 22.0)
        );
        assert_eq!(
            i.attributes[0].source,
            Some(i.block.attribute_definitions().next().unwrap().1.id)
        );
    }

    #[test]
    fn a_sequence_cut_short_is_dropped_and_never_joins_the_next_insert() {
        let mut e = insert("0", true);
        e.extend(attrib("lost", &[]));
        // No SEQEND: the next INSERT ends it.
        e.extend(insert("50", false));
        let loaded = read(&file(&tag_block(), &e)).unwrap();
        assert_eq!(loaded.warnings, ["Skipped 1 damaged objects."]);
        assert_eq!(values(&loaded.document), [Vec::<String>::new()]);
        // Cut off by the end of the section as well.
        let mut e = insert("0", true);
        e.extend(attrib("lost", &[]));
        let loaded = read(&file(&tag_block(), &e)).unwrap();
        assert_eq!(values(&loaded.document).len(), 0);
        assert_eq!(loaded.warnings, ["Skipped 1 damaged objects."]);
    }

    #[test]
    fn an_attribute_with_no_insert_is_counted_and_unsupported_text_is_named() {
        let mut e = attrib("stray", &[]);
        e.extend(insert("0", true));
        e.extend(attrib("fitted", &[(72, "3")]));
        e.extend(attrib("kept", &[]));
        e.push((0, "SEQEND"));
        let loaded = read(&file(&tag_block(), &e)).unwrap();
        assert_eq!(
            loaded.warnings,
            ["Skipped 2 unsupported objects: 1 ATTRIB, 1 ATTRIB (aligned or fit text)."]
        );
        assert_eq!(values(&loaded.document), [vec!["kept".to_string()]]);
    }

    #[test]
    fn an_insert_inside_a_block_keeps_its_attributes() {
        let mut blocks = tag_block();
        blocks.extend([
            (0, "BLOCK"),
            (8, "0"),
            (2, "OUTER"),
            (70, "0"),
            (10, "0"),
            (20, "0"),
        ]);
        blocks.extend(insert("5", true));
        blocks.extend(attrib("inner", &[]));
        blocks.extend([(0, "SEQEND"), (0, "ENDBLK")]);
        let loaded = read(&file(&blocks, &[])).unwrap();
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
        loaded.document.validate().unwrap();
        let outer = loaded.document.block_named("OUTER").unwrap();
        let Entity::Insert(i) = &outer.entities[0].entity else {
            panic!("an insert")
        };
        assert_eq!(i.attributes[0].value.display(), "inner");
    }

    #[test]
    fn a_mirrored_attribute_is_reflected_and_a_tilted_one_is_skipped() {
        let mut e = insert("0", true);
        let flip = [(210, "0"), (220, "0"), (230, "-1"), (50, "30")];
        e.extend(attrib("flipped", &flip));
        e.extend(attrib("tilted", &[(210, "1"), (220, "0"), (230, "0")]));
        e.push((0, "SEQEND"));
        let loaded = read(&file(&tag_block(), &e)).unwrap();
        assert_eq!(
            loaded.warnings,
            ["Skipped 1 unsupported object: 1 ATTRIB (not in the XY plane)."]
        );
        let Entity::Insert(i) = &loaded.document.objects()[0].entity else {
            panic!("an insert")
        };
        let text = &i.attributes[0].text;
        assert_eq!(text.position, od_core::Point::new(-11.0, 22.0));
        assert!((text.rotation - 150f64.to_radians()).abs() < 1e-9);
    }

    #[test]
    fn a_bad_tag_or_height_is_damaged_and_a_second_280_after_the_tag_locks() {
        let mut e = insert("0", true);
        e.extend([
            (0, "ATTRIB"),
            (10, "0"),
            (20, "0"),
            (40, "2"),
            (1, "v"),
            (2, "has space"),
            (70, "0"),
        ]);
        e.extend(attrib("locked", &[(280, "1")]));
        e.push((0, "SEQEND"));
        let loaded = read(&file(&tag_block(), &e)).unwrap();
        assert_eq!(loaded.warnings, ["Skipped 1 damaged objects."]);
        let Entity::Insert(i) = &loaded.document.objects()[0].entity else {
            panic!("an insert")
        };
        assert!(i.attributes[0].lock_position);
    }

    #[test]
    fn an_attribute_on_another_layer_than_its_definition_keeps_its_own() {
        let mut e = insert("0", true);
        let mut marked = attrib("marked", &[(62, "1")]);
        marked[1] = (8, "Marks");
        e.extend(marked);
        e.push((0, "SEQEND"));
        let loaded = read(&file(&tag_block(), &e)).unwrap();
        let Entity::Insert(i) = &loaded.document.objects()[0].entity else {
            panic!("an insert")
        };
        let own = i.attributes[0].attrs.as_ref().expect("its own properties");
        assert_eq!(loaded.document.layer(own.layer).unwrap().name, "Marks");
        assert_eq!(
            own.color,
            od_core::Attr::Set(od_core::Color::Rgb(255, 0, 0))
        );
        assert_eq!(loaded.document.validate(), Ok(()));
    }

    #[test]
    fn an_attribute_that_looks_like_its_definition_keeps_none() {
        // The definition is on layer 0 with nothing set; so is this attribute.
        let mut e = insert("0", true);
        e.extend(attrib("same", &[]));
        e.push((0, "SEQEND"));
        let loaded = read(&file(&tag_block(), &e)).unwrap();
        let Entity::Insert(i) = &loaded.document.objects()[0].entity else {
            panic!("an insert")
        };
        assert_eq!(i.attributes[0].attrs, None);
    }
}
