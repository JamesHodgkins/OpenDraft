//! Templates to start a drawing from: a few starters built into the program
//! and the user's own `.odt` files in the templates folder beside the
//! settings. A template is only ever read: starting from one makes a new
//! unsaved drawing.

use std::path::{Path, PathBuf};

use od_core::units::{InsertUnits, LengthFormat, Units};
use od_core::Document;

/// More than this many files in the folder are not listed.
const MAX_USER_TEMPLATES: usize = 200;

/// A starter built into the program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Starter {
    Millimetres,
    Inches,
    FeetAndInches,
}

impl Starter {
    pub const ALL: [Starter; 3] = [
        Starter::Millimetres,
        Starter::Inches,
        Starter::FeetAndInches,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Millimetres => "Blank: millimetres",
            Self::Inches => "Blank: inches",
            Self::FeetAndInches => "Blank: feet and inches",
        }
    }

    /// The drawing a new one starts as.
    pub fn document(self) -> Document {
        let units = match self {
            Self::Millimetres => Units {
                length_precision: 2,
                insert: InsertUnits::from_code(4).unwrap_or_default(),
                ..Units::default()
            },
            Self::Inches => Units {
                length_precision: 3,
                insert: InsertUnits::from_code(1).unwrap_or_default(),
                ..Units::default()
            },
            Self::FeetAndInches => Units {
                length: LengthFormat::Architectural,
                length_precision: 4,
                insert: InsertUnits::from_code(1).unwrap_or_default(),
                ..Units::default()
            },
        };
        let mut document = Document::default();
        document.set_units(units);
        document
    }
}

/// Where a template comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Starter(Starter),
    File(PathBuf),
}

/// One template as the picker lists it.
#[derive(Clone, Debug, PartialEq)]
pub struct Template {
    pub name: String,
    pub source: Source,
    /// What it sets up, or why it cannot be used.
    pub summary: Result<String, String>,
}

/// What a drawing sets up, in a line: units, layouts, layers.
pub fn summarise(document: &Document) -> String {
    let units = document.units();
    let insert = units.insert.label();
    let unit = if units.insert == InsertUnits::UNITLESS {
        "unitless".to_owned()
    } else {
        insert.to_lowercase()
    };
    let layouts = match document.layouts().len() {
        0 => "model space only".to_owned(),
        n => crate::text::plural(n, "layout"),
    };
    format!(
        "{}, {unit}  \u{b7}  {layouts}  \u{b7}  {}",
        units.length.label(),
        crate::text::plural(document.layers().len(), "layer")
    )
}

pub fn starters() -> Vec<Template> {
    Starter::ALL
        .into_iter()
        .map(|starter| Template {
            name: starter.name().to_owned(),
            source: Source::Starter(starter),
            summary: Ok(summarise(&starter.document())),
        })
        .collect()
}

/// The `.odt` files in `dir`, by name; one that cannot be read is listed as
/// unavailable, with why. A folder that is not there has none.
pub fn scan(dir: &Path) -> Vec<Template> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = read
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && od_files::is_template(p))
        .collect();
    paths.sort_by_key(|p| p.file_name().map(|n| n.to_ascii_lowercase()));
    paths.truncate(MAX_USER_TEMPLATES);
    paths
        .into_iter()
        .map(|path| {
            let name = path
                .file_stem()
                .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
            let summary = od_files::load(&path)
                .map(|loaded| summarise(&loaded.document))
                .map_err(|e| e.to_string());
            Template {
                name,
                source: Source::File(path),
                summary,
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "templates_tests.rs"]
mod tests;
