//! A safe reader for AutoCAD DWG files: AC1024 (2010), AC1027 (2013) and
//! AC1032 (2018 and later).
//!
//! This crate knows nothing about OpenDraft. It depends on nothing else in
//! the workspace and has no dependencies at all, so it can be lifted out and
//! used on its own. Every size and offset in a file is untrusted: reads are
//! bounded, arithmetic is checked, allocations are capped by [`DwgLimits`],
//! and a long read can be cancelled.
//!
//! The binary layout follows the *Open Design Specification for .dwg files*
//! (version 5.4.1).
//!
//! What exists so far:
//!
//! - the container: [`Container::open`] verifies the file header and page
//!   maps, and [`Container::read_section`] returns a logical section such as
//!   `AcDb:Header` or `AcDb:AcDbObjects`, decompressed and checksum-verified;
//! - the object database: [`Inventory::read`] lists every object with its
//!   type, owner and layer, after checking each one's envelope and CRC;
//! - the drawing: [`Drawing::read`] returns the layers, the flat shapes of
//!   model space (lines, circles, arcs, ellipses, splines, polylines and block
//!   references) in
//!   drawing order, the named blocks with their shapes, the header's unit
//!   settings, and an [`ImportReport`] of everything else. Text (TEXT and
//!   MTEXT, with the style table) is read; anonymous blocks, paper space and
//!   the rest are counted, not read.

mod attrib;
mod bit;
mod checksum;
mod classes;
mod common;
mod compression;
mod container;
mod cursor;
mod database;
mod drawing;
mod entities;
mod error;
mod framed;
mod geometry;
mod handles;
mod header;
mod inventory;
mod limits;
mod lineweight;
mod mask;
mod object;
mod pass;
mod report;
mod section_map;
mod sections;
mod shapes;
mod spline;
mod tables;
#[cfg(test)]
mod testkit;
mod text;
mod types;
mod version;

pub use classes::{ClassRecord, ClassRegistry};
pub use container::{section_names, Container};
pub use drawing::{Drawing, DwgBlock, DwgColor, DwgEntity, DwgLayer, DwgLinetype};
pub use error::{DwgError, DwgErrorKind, DwgStage};
pub use handles::DwgHandle;
pub use header::DwgHeader;
pub use inventory::{Inventory, ObjectSummary};
pub use limits::DwgLimits;
pub use lineweight::DwgLineweight;
pub use report::{Degradation, ImportReport, Normalization, Skipped};
pub use section_map::SectionInfo;
pub use shapes::{DwgAttribute, Shape, MAX_ABS_COORDINATE};
pub use tables::DwgLinetypeDef;
pub use text::DwgTextStyle;
pub use types::{FixedType, ObjectKind};
pub use version::DwgVersion;

pub(crate) use container::check_cancelled;
