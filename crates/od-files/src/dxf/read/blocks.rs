//! Blocks from DXF: the named definitions in the BLOCKS section and the
//! `INSERT` entities that place them. Definitions are read when the BLOCKS
//! section begins (after the layer table, before any entity that inserts
//! them) and built by `block_import`. Anonymous blocks (the pictures of
//! dimensions, hatches and dynamic blocks), external references and the
//! spaces' own blocks are not definitions here.

use std::collections::HashMap;

use od_core::{Entity, Point};

use super::layouts::Spaces;
use super::polyline::Vertex;
use super::{decode, Importer, Record};
use crate::block_import::{self, ImportedBlocks, Item, PendingBlock, PendingInsert};

/// Why a block in the file is not read.
#[derive(Clone, Copy)]
enum Unread {
    Anonymous,
    External,
}

/// The importer's block state.
#[derive(Default)]
pub(super) struct BlockReading {
    /// The objects of the definition being read, while one is.
    collecting: Option<Vec<(Item<String>, od_core::Attributes)>>,
    built: ImportedBlocks<String>,
    /// Blocks in the file that are not read, by lower-cased name.
    unread: HashMap<String, Unread>,
    anonymous_references: u64,
    external_references: u64,
}

impl BlockReading {
    pub(super) fn warnings(&self) -> Vec<String> {
        let mut out = self.built.warnings();
        if self.anonymous_references > 0 {
            out.push(block_import::anonymous_references(
                self.anonymous_references,
            ));
        }
        if self.external_references > 0 {
            out.push(block_import::external_references(self.external_references));
        }
        out
    }
}

/// Whether a block is one of the spaces' own (`*Model_Space`, R12's
/// `$MODEL_SPACE`, a layout's sheet).
fn is_space(name: &str, spaces: &Spaces) -> bool {
    let lower = name.to_lowercase();
    lower.starts_with("*model_space")
        || lower.starts_with("*paper_space")
        || lower == "$model_space"
        || lower == "$paper_space"
        || spaces.is_sheet_block(name)
}

impl Importer {
    /// Reads the definitions in a BLOCKS section, `records` starting after
    /// its SECTION record, and builds them into the block table.
    pub(super) fn read_blocks(&mut self, records: &[Record], spaces: &Spaces) {
        let mut pending: Vec<PendingBlock<String>> = Vec::new();
        let mut current: Option<PendingBlock<String>> = None;
        let mut open: Option<(&Record, Vec<Vertex>)> = None;
        for record in records {
            match record.name.as_str() {
                "ENDSEC" => break,
                "BLOCK" => {
                    let name = decode(record.text(2).unwrap_or(""));
                    let flags = record.int(70).unwrap_or(0);
                    let key = name.to_lowercase();
                    current = None;
                    if is_space(&name, spaces) {
                        continue;
                    } else if flags & (4 | 8) != 0 {
                        self.blocks.unread.insert(key, Unread::External);
                    } else if flags & 1 != 0 || name.starts_with('*') {
                        self.blocks.unread.insert(key, Unread::Anonymous);
                    } else {
                        current = Some(PendingBlock {
                            key,
                            name,
                            base: Self::point(record, 10, 20).unwrap_or(Point::new(0.0, 0.0)),
                            description: decode(record.text(4).unwrap_or("")),
                            items: Vec::new(),
                        });
                        self.blocks.collecting = Some(Vec::new());
                    }
                }
                "ENDBLK" => {
                    if let Some(mut block) = current.take() {
                        block.items = self.blocks.collecting.take().unwrap_or_default();
                        pending.push(block);
                    }
                }
                _ if current.is_some() => self.shape(record, &mut open),
                _ => {}
            }
        }
        self.blocks.collecting = None;
        self.blocks.built = ImportedBlocks::build(&mut self.document, pending);
    }

    /// An INSERT entity. One with rows or columns (a MINSERT array) is
    /// skipped; the ATTRIB records that may follow it are skipped on their own.
    pub(super) fn insert(&mut self, record: &Record) {
        let columns = record.int(70).unwrap_or(1);
        let rows = record.int(71).unwrap_or(1);
        if columns > 1 || rows > 1 {
            self.skip("INSERT (array)");
            return;
        }
        let Some(insertion) = Self::point(record, 10, 20) else {
            self.damaged += 1;
            return;
        };
        let key = decode(record.text(2).unwrap_or("")).to_lowercase();
        match self.blocks.unread.get(&key) {
            Some(Unread::Anonymous) => self.blocks.anonymous_references += 1,
            Some(Unread::External) => self.blocks.external_references += 1,
            None => {
                let insert = PendingInsert {
                    block: key,
                    insertion,
                    scale_x: record.number(41).unwrap_or(1.0),
                    scale_y: record.number(42).unwrap_or(1.0),
                    rotation: record.number(50).unwrap_or(0.0).to_radians(),
                };
                self.add_item(record, Item::Insert(insert));
            }
        }
    }

    /// Whether a block definition is being read (its objects go to it).
    pub(super) fn reading_block(&self) -> bool {
        self.blocks.collecting.is_some()
    }

    /// Adds an object to the definition being read, or to the drawing.
    pub(super) fn add_item(&mut self, record: &Record, item: Item<String>) {
        let attrs = self.attributes(record);
        if let Some(items) = self.blocks.collecting.as_mut() {
            items.push((item, attrs));
            self.imported += 1;
            return;
        }
        let entity: Option<Entity> = match item {
            Item::Entity(entity) => Some(entity),
            Item::Insert(insert) => self.blocks.built.insert(insert),
        };
        if let Some(entity) = entity {
            let id = self.document.add_with(entity, attrs);
            if let Some(handle) = record.text(5) {
                self.handles.insert(handle.to_uppercase(), id);
            }
            self.imported += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use od_core::{Attr, Color, LayerId};

    use super::super::read;
    use super::*;

    fn dxf(blocks: &[(i32, &str)], entities: &[(i32, &str)]) -> Vec<u8> {
        let mut pairs: Vec<(i32, &str)> = vec![(0, "SECTION"), (2, "BLOCKS")];
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

    const LINE: [(i32, &str); 6] = [
        (0, "LINE"),
        (8, "0"),
        (10, "0"),
        (20, "0"),
        (11, "1"),
        (21, "0"),
    ];

    fn block<'a>(name: &'a str, flags: &'a str, body: &[(i32, &'a str)]) -> Vec<(i32, &'a str)> {
        let mut out = vec![
            (0, "BLOCK"),
            (8, "0"),
            (2, name),
            (70, flags),
            (10, "1"),
            (20, "2"),
            (4, "A tick"),
        ];
        out.extend_from_slice(body);
        out.push((0, "ENDBLK"));
        out
    }

    #[test]
    fn a_block_and_its_reference_come_in() {
        let mut blocks = block("*Model_Space", "0", &[]);
        blocks.extend(block("Tick", "0", &LINE));
        let insert = [
            (0, "INSERT"),
            (8, "Marks"),
            (62, "3"),
            (2, "TICK"),
            (10, "30"),
            (20, "25"),
            (41, "2"),
            (42, "-1"),
            (50, "90"),
        ];
        let loaded = read(&dxf(&blocks, &insert)).unwrap();
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
        let doc = &loaded.document;
        doc.validate().unwrap();
        assert_eq!(doc.blocks().len(), 1);
        let tick = &doc.blocks()[0];
        assert_eq!(tick.name, "Tick");
        assert_eq!(
            (tick.base, tick.description.as_str()),
            (Point::new(1.0, 2.0), "A tick")
        );
        assert_eq!(tick.entities.len(), 1);
        assert_eq!(tick.entities[0].attrs.layer, LayerId::DEFAULT);
        assert_eq!(doc.len(), 1);
        let object = &doc.objects()[0];
        let Entity::Insert(i) = &object.entity else {
            panic!("not a block reference");
        };
        assert!(std::sync::Arc::ptr_eq(&i.block, tick));
        assert_eq!(i.insertion, Point::new(30.0, 25.0));
        assert_eq!((i.scale_x, i.scale_y), (2.0, -1.0));
        assert!((i.rotation - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert_eq!(object.attrs.color, Attr::Set(Color::Rgb(0, 255, 0)));
        assert_ne!(object.attrs.layer, LayerId::DEFAULT);
    }

    #[test]
    fn nested_blocks_may_come_in_any_order() {
        let nested = [(0, "INSERT"), (8, "0"), (2, "Inner"), (10, "0"), (20, "0")];
        let mut blocks = block("Outer", "0", &nested);
        blocks.extend(block("Inner", "0", &LINE));
        let loaded = read(&dxf(&blocks, &[])).unwrap();
        let doc = &loaded.document;
        doc.validate().unwrap();
        let names: Vec<&str> = doc.blocks().iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, ["Outer", "Inner"]);
        assert!(doc.blocks()[0].holds(doc.blocks()[1].id));
    }

    #[test]
    fn anonymous_and_external_blocks_are_not_read() {
        let mut blocks = block("*U1", "1", &LINE);
        blocks.extend(block("*D0", "1", &LINE));
        blocks.extend(block("Site", "4", &[]));
        let refs = [
            (0, "INSERT"),
            (8, "0"),
            (2, "*U1"),
            (10, "0"),
            (20, "0"),
            (0, "INSERT"),
            (8, "0"),
            (2, "Site"),
            (10, "0"),
            (20, "0"),
            (0, "INSERT"),
            (8, "0"),
            (2, "Site"),
            (10, "0"),
            (20, "0"),
            (70, "2"),
        ];
        let loaded = read(&dxf(&blocks, &refs)).unwrap();
        assert!(loaded.document.blocks().is_empty() && loaded.document.is_empty());
        assert_eq!(
            loaded.warnings,
            [
                "Skipped 1 unsupported object: 1 INSERT (array).",
                &block_import::anonymous_references(1),
                &block_import::external_references(1),
            ]
        );
    }

    #[test]
    fn by_block_properties_are_kept() {
        let body = [
            (0, "LINE"),
            (8, "0"),
            (62, "0"),
            (6, "BYBLOCK"),
            (10, "0"),
            (20, "0"),
            (11, "1"),
            (21, "0"),
        ];
        let loaded = read(&dxf(&block("B", "0", &body), &[])).unwrap();
        let e = &loaded.document.blocks()[0].entities[0];
        assert_eq!(
            (&e.attrs.color, &e.attrs.linetype),
            (&Attr::ByBlock, &Attr::ByBlock)
        );
        assert!(loaded.warnings.is_empty());
    }
}
