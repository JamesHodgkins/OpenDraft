//! ATTRIB and ATTDEF (sections 20.4.4 and 20.4.5): single-line text with a tag and
//! modes. An ATTRIB is read for the reference that owns it, and an ATTDEF is
//! a template shape. The multiline kind carries an embedded MTEXT and is
//! named as unsupported.

use crate::error::DwgError;
use crate::handles::DwgHandle;
use crate::limits::DwgLimits;
use crate::object::ObjectStreams;
use crate::report::{ImportReport, Skipped};
use crate::shapes::{DwgAttribute, Shape};
use crate::text::{line, read_text, TextData};
use crate::version::DwgVersion;

/// An ATTRIB or ATTDEF: a text with a tag and flags. Only the single-line kind
/// is read; the multiline kind carries an embedded MTEXT and is noted as such.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AttribData {
    pub text: TextData,
    pub multiline: bool,
    pub invisible: bool,
    pub constant: bool,
    /// The mode bits as stored: invisible 1, constant 2, verify 4, preset 8.
    pub flags: u8,
    pub tag: String,
    /// An ATTDEF's prompt (empty for an ATTRIB).
    pub prompt: String,
    pub field_length: u16,
    pub lock_position: bool,
}

/// ATTRIB and ATTDEF (§20.4.4 and §20.4.5): the TEXT fields, a version byte,
/// from 2018 the kind, then (single line) the tag, field length, flags and
/// the position lock. An ATTDEF's prompt follows.
pub(crate) fn read_attrib(
    s: &mut ObjectStreams<'_>,
    base: DwgHandle,
    version: DwgVersion,
    limits: &DwgLimits,
    definition: bool,
) -> Result<AttribData, DwgError> {
    let text = read_text(s, base, limits)?;
    s.data.rc()?; // version
    let multiline = version >= DwgVersion::R2018 && s.data.rc()? != 1;
    if multiline {
        return Ok(AttribData {
            text,
            multiline,
            invisible: false,
            constant: false,
            flags: 0,
            tag: String::new(),
            prompt: String::new(),
            field_length: 0,
            lock_position: false,
        });
    }
    let tag = s.strings.text(limits.max_string_code_units)?;
    let field_length = s.data.bs()?;
    let flags = s.data.rc()?;
    let lock_position = s.data.bit()?;
    let prompt = if definition {
        s.strings.text(limits.max_string_code_units)?
    } else {
        String::new()
    };
    Ok(AttribData {
        text,
        multiline,
        invisible: flags & 1 != 0,
        constant: flags & 2 != 0,
        flags,
        tag,
        prompt,
        field_length,
        lock_position,
    })
}

/// An ATTRIB: the value it holds and how it is set, for the reference that
/// owns it. `hidden` is the entity itself being switched off.
pub(crate) fn attribute(
    d: &AttribData,
    hidden: bool,
    report: &mut ImportReport,
) -> Result<DwgAttribute, Skipped> {
    if d.multiline {
        return Err(Skipped::Unsupported("multiline attribute".into()));
    }
    Ok(DwgAttribute {
        tag: d.tag.clone(),
        flags: d.flags | u8::from(hidden),
        lock_position: d.lock_position,
        field_length: d.field_length,
        text: line(&d.text, report, true)?,
    })
}

/// An ATTDEF: the template, with its default value as its text.
pub(crate) fn definition(d: &AttribData, report: &mut ImportReport) -> Result<Shape, Skipped> {
    if d.multiline {
        return Err(Skipped::Unsupported("multiline attribute".into()));
    }
    Ok(Shape::AttributeDefinition {
        tag: d.tag.clone(),
        prompt: d.prompt.clone(),
        flags: d.flags,
        lock_position: d.lock_position,
        field_length: d.field_length,
        text: Box::new(line(&d.text, report, true)?),
    })
}
