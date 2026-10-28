//! Linetypes: the named dash patterns objects and layers refer to. The table
//! lives on the `Document`; a library (`stock`, later `.lin` files) is only
//! where a definition comes from, and once loaded the drawing owns it.
//!
//! A pattern is one repeat of positive (dash), negative (gap) and zero (dot)
//! lengths, as in DXF group 49. `CONTINUOUS`, `BYLAYER` and `BYBLOCK` are real
//! records that always exist; the last two say where an object's linetype
//! comes from and have no pattern of their own.

pub mod dash;
pub mod paths;
pub mod stock;

#[cfg(test)]
mod dash_tests;
#[cfg(test)]
mod tests;

use std::fmt;

/// Longest linetype name accepted (AutoCAD's extended-names limit).
pub const MAX_NAME_BYTES: usize = 255;

/// Permanent identity of a linetype record; never reused within a drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LinetypeId(pub u64);

impl LinetypeId {
    pub const CONTINUOUS: LinetypeId = LinetypeId(1);
    pub const BYLAYER: LinetypeId = LinetypeId(2);
    pub const BYBLOCK: LinetypeId = LinetypeId(3);
    // The stock records a new drawing starts with (see `SEEDED`).
    pub const DASHED: LinetypeId = LinetypeId(4);
    pub const DOT: LinetypeId = LinetypeId(5);
    pub const DASHDOT: LinetypeId = LinetypeId(6);
    pub const CENTER: LinetypeId = LinetypeId(7);
    pub const HIDDEN: LinetypeId = LinetypeId(8);
    pub const PHANTOM: LinetypeId = LinetypeId(9);

    /// The three records every drawing has and none can remove.
    pub fn is_special(self) -> bool {
        matches!(self.0, 1..=3)
    }
}

/// The stock records a new drawing starts with, after the three special ones,
/// so the names layers and objects already use resolve. Their ids are fixed:
/// 4 to 9 in this order.
const SEEDED: [&str; 6] = ["DASHED", "DOT", "DASHDOT", "CENTER", "HIDDEN", "PHANTOM"];

/// The first id handed out for a record the user adds.
const FIRST_FREE_ID: u64 = 10;

/// One element of a repeating pattern. Lengths are strictly positive and
/// finite.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StrokeElement {
    Dash(f64),
    Gap(f64),
    Dot,
}

impl StrokeElement {
    /// The element for a DXF group 49 value: above zero a dash, below a gap,
    /// zero a dot. `None` for a value that is not a number.
    pub fn from_dxf(value: f64) -> Option<Self> {
        if !value.is_finite() {
            None
        } else if value > 0.0 {
            Some(Self::Dash(value))
        } else if value < 0.0 {
            Some(Self::Gap(-value))
        } else {
            Some(Self::Dot)
        }
    }

    /// The DXF group 49 value: gaps are negative.
    pub fn to_dxf(self) -> f64 {
        match self {
            Self::Dash(d) => d,
            Self::Gap(g) => -g,
            Self::Dot => 0.0,
        }
    }

    /// How far along the path the element reaches (a dot has no length).
    pub fn length(self) -> f64 {
        match self {
            Self::Dash(d) | Self::Gap(d) => d,
            Self::Dot => 0.0,
        }
    }

    fn is_valid(self) -> bool {
        match self {
            Self::Dash(d) | Self::Gap(d) => d.is_finite() && d > 0.0,
            Self::Dot => true,
        }
    }
}

/// A linetype record.
#[derive(Clone, Debug, PartialEq)]
pub struct Linetype {
    pub id: LinetypeId,
    pub name: String,
    pub description: String,
    /// One repeat of the pattern; empty for a continuous line.
    pub elements: Vec<StrokeElement>,
    /// DXF symbol-table flags (group 70), kept as read.
    pub flags: u16,
}

impl Linetype {
    /// Total length of one repeat: the sum of the absolute element lengths.
    pub fn length(&self) -> f64 {
        self.elements.iter().map(|e| e.length()).sum()
    }

    /// True when there is no pattern to draw.
    pub fn is_continuous(&self) -> bool {
        self.elements.is_empty()
    }

    /// Why a `.lin` file could not hold this pattern, if it could not:
    /// AutoCAD allows 2 to 12 elements and a first one that is not a gap.
    /// A record read from a DXF file is kept even if this says no.
    pub fn lin_problem(&self) -> Option<&'static str> {
        if !(2..=12).contains(&self.elements.len()) {
            Some("a .lin pattern has 2 to 12 elements")
        } else if matches!(self.elements[0], StrokeElement::Gap(_)) {
            Some("a .lin pattern cannot start with a gap")
        } else {
            None
        }
    }
}

/// Why a record could not be added or changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinetypeError {
    EmptyName,
    NameTooLong,
    /// A record with this name (ignoring case) exists.
    DuplicateName(String),
    /// An id is already used by another record.
    DuplicateId(u64),
    /// A length is zero, negative or not a number.
    BadElement,
    /// The record is one of the three that cannot change.
    Special(String),
    Unknown(String),
    /// A table read from a file lacks one of the three special records, or
    /// gives it a pattern.
    BadSpecial(&'static str),
}

impl fmt::Display for LinetypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => write!(f, "a linetype needs a name"),
            Self::NameTooLong => write!(f, "a linetype name is at most {MAX_NAME_BYTES} bytes"),
            Self::DuplicateName(n) => write!(f, "a linetype called {n} already exists"),
            Self::DuplicateId(i) => write!(f, "linetype id {i} appears twice"),
            Self::BadElement => write!(f, "a dash or gap length must be a positive number"),
            Self::Special(n) => write!(f, "{n} cannot be changed or removed"),
            Self::Unknown(n) => write!(f, "there is no linetype {n}"),
            Self::BadSpecial(n) => write!(f, "the linetype table has no proper {n} record"),
        }
    }
}

impl std::error::Error for LinetypeError {}

/// The drawing's linetypes in table order.
#[derive(Clone, Debug, PartialEq)]
pub struct LinetypeTable {
    records: Vec<Linetype>,
    next_id: u64,
}

fn record(id: u64, name: &str, description: &str, pattern: &[f64]) -> Linetype {
    Linetype {
        id: LinetypeId(id),
        name: name.to_owned(),
        description: description.to_owned(),
        elements: pattern
            .iter()
            .filter_map(|v| StrokeElement::from_dxf(*v))
            .collect(),
        flags: 0,
    }
}

impl Default for LinetypeTable {
    fn default() -> Self {
        let mut records = vec![
            record(LinetypeId::CONTINUOUS.0, "CONTINUOUS", "Solid line", &[]),
            record(LinetypeId::BYLAYER.0, "BYLAYER", "", &[]),
            record(LinetypeId::BYBLOCK.0, "BYBLOCK", "", &[]),
        ];
        for (i, name) in SEEDED.iter().enumerate() {
            if let Some(s) = stock::find(name) {
                records.push(record(4 + i as u64, s.name, s.description, s.pattern));
            }
        }
        Self {
            records,
            next_id: FIRST_FREE_ID,
        }
    }
}

impl LinetypeTable {
    /// A table from records read from a file, with the counter that keeps
    /// ids of deleted records retired. The three special records must be
    /// there and have no pattern.
    pub fn from_records(records: Vec<Linetype>, next_id: u64) -> Result<Self, LinetypeError> {
        let mut table = Self {
            records: Vec::new(),
            next_id: FIRST_FREE_ID,
        };
        for record in records {
            table.push(record)?;
        }
        for (id, name) in [
            (LinetypeId::CONTINUOUS, "CONTINUOUS"),
            (LinetypeId::BYLAYER, "BYLAYER"),
            (LinetypeId::BYBLOCK, "BYBLOCK"),
        ] {
            let proper = table
                .get(id)
                .is_some_and(|r| r.name.eq_ignore_ascii_case(name) && r.is_continuous());
            if !proper {
                return Err(LinetypeError::BadSpecial(name));
            }
        }
        table.reserve_ids(next_id);
        Ok(table)
    }

    /// Records in table order.
    pub fn records(&self) -> &[Linetype] {
        &self.records
    }

    pub fn get(&self, id: LinetypeId) -> Option<&Linetype> {
        self.records.iter().find(|r| r.id == id)
    }

    /// The record called `name`, ignoring ASCII case as DXF does.
    pub fn named(&self, name: &str) -> Option<&Linetype> {
        self.records
            .iter()
            .find(|r| r.name.eq_ignore_ascii_case(name))
    }

    /// The next id that would be handed out.
    pub fn next_id(&self) -> u64 {
        self.next_id
    }

    /// Make sure ids already used are never handed out again. Only raises it.
    pub fn reserve_ids(&mut self, next: u64) {
        self.next_id = self.next_id.max(next);
    }

    /// Add a new record with a fresh id.
    pub fn add(
        &mut self,
        name: &str,
        description: &str,
        elements: Vec<StrokeElement>,
    ) -> Result<LinetypeId, LinetypeError> {
        let id = LinetypeId(self.next_id);
        self.push(Linetype {
            id,
            name: name.to_owned(),
            description: description.to_owned(),
            elements,
            flags: 0,
        })?;
        Ok(id)
    }

    /// Append a record that already has its id (what loading and undo use).
    /// Checks the name, the id and the lengths; changes nothing on failure.
    pub fn push(&mut self, record: Linetype) -> Result<(), LinetypeError> {
        self.check_name(&record.name)?;
        if self.records.iter().any(|r| r.id == record.id) {
            return Err(LinetypeError::DuplicateId(record.id.0));
        }
        if !record.elements.iter().all(|e| e.is_valid()) {
            return Err(LinetypeError::BadElement);
        }
        self.next_id = self.next_id.max(crate::document::after(record.id.0));
        self.records.push(record);
        Ok(())
    }

    /// Add the stock definition called `name` unless the drawing has a record
    /// of that name already, and return the record either way.
    pub fn load_stock(&mut self, name: &str) -> Result<LinetypeId, LinetypeError> {
        if let Some(existing) = self.named(name) {
            return Ok(existing.id);
        }
        let s = stock::find(name).ok_or_else(|| LinetypeError::Unknown(name.to_owned()))?;
        self.add(
            s.name,
            s.description,
            s.pattern
                .iter()
                .filter_map(|v| StrokeElement::from_dxf(*v))
                .collect(),
        )
    }

    /// Replace the record with the same id (never a special one), returning
    /// the old record. The new name may not clash with another record.
    pub fn replace(&mut self, new: Linetype) -> Result<Linetype, LinetypeError> {
        let index = self
            .records
            .iter()
            .position(|r| r.id == new.id)
            .ok_or_else(|| LinetypeError::Unknown(new.name.clone()))?;
        if new.id.is_special() {
            return Err(LinetypeError::Special(self.records[index].name.clone()));
        }
        self.check_name_except(&new.name, new.id)?;
        if !new.elements.iter().all(|e| e.is_valid()) {
            return Err(LinetypeError::BadElement);
        }
        Ok(std::mem::replace(&mut self.records[index], new))
    }

    /// Remove a record (never a special one) and return it. Whether anything
    /// still uses it is for the caller to check.
    pub fn remove(&mut self, id: LinetypeId) -> Result<Linetype, LinetypeError> {
        let index = self
            .records
            .iter()
            .position(|r| r.id == id)
            .ok_or_else(|| LinetypeError::Unknown(id.0.to_string()))?;
        if id.is_special() {
            return Err(LinetypeError::Special(self.records[index].name.clone()));
        }
        Ok(self.records.remove(index))
    }

    /// True when the table is exactly what a new drawing starts with.
    pub fn is_stock(&self) -> bool {
        *self == Self::default()
    }

    fn check_name(&self, name: &str) -> Result<(), LinetypeError> {
        self.check_name_except(name, LinetypeId(0))
    }

    fn check_name_except(&self, name: &str, except: LinetypeId) -> Result<(), LinetypeError> {
        if name.trim().is_empty() {
            return Err(LinetypeError::EmptyName);
        }
        if name.len() > MAX_NAME_BYTES {
            return Err(LinetypeError::NameTooLong);
        }
        let clash = self
            .records
            .iter()
            .any(|r| r.id != except && r.name.eq_ignore_ascii_case(name));
        if clash {
            return Err(LinetypeError::DuplicateName(name.to_owned()));
        }
        Ok(())
    }
}
