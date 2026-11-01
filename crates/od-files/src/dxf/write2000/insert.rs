//! Block references and their attributes as R2000 entities: `INSERT` with
//! group 66 = 1, one `ATTRIB` owned by it for each attribute, and `SEQEND`;
//! and `ATTDEF`.
//!
//! An `ATTRIB` is written where it is drawn (the insert's transform applied
//! to the block's coordinates), unmirrored, in the normal plane. For text
//! that is justified, AutoCAD recomputes the first point from the second
//! when it opens the file, so both carry the one point.

use od_core::arc;
use od_core::block::{
    AttributeDefinition, AttributeFlags, AttributeText, TextHorizontal, TextVertical,
};
use od_core::{Document, EntityId, Insert, Object};

use super::entities::entity_head;
use super::{Handles, Ids};
use crate::dxf::write::{dxf_name, finite, Out};
use crate::FileError;

fn across(t: &AttributeText) -> i32 {
    match t.horizontal {
        TextHorizontal::Left => 0,
        TextHorizontal::Center => 1,
        TextHorizontal::Right => 2,
    }
}

fn down(t: &AttributeText) -> i32 {
    match t.vertical {
        TextVertical::Baseline => 0,
        TextVertical::Bottom => 1,
        TextVertical::Middle => 2,
        TextVertical::Top => 3,
    }
}

/// The `AcDbText` part both records share.
fn text_part(
    out: &mut Out,
    document: &Document,
    t: &AttributeText,
    value: &str,
) -> Result<(), FileError> {
    let (x, y) = (finite(t.position.x)?, finite(t.position.y)?);
    out.pair(100, "AcDbText");
    out.point3(10, x, y, 0.0);
    out.pair(40, finite(t.height)?);
    out.pair(1, value);
    if t.rotation != 0.0 {
        out.pair(50, finite(arc::normalize(t.rotation).to_degrees())?);
    }
    if t.width_factor != 1.0 {
        out.pair(41, finite(t.width_factor)?);
    }
    if t.oblique != 0.0 {
        out.pair(51, finite(t.oblique.to_degrees())?);
    }
    let style = document
        .text_style(t.style)
        .map_or_else(|| "Standard".to_owned(), |s| dxf_name(&s.name));
    out.pair(7, style);
    if t.generation_flags != 0 {
        out.pair(71, i32::from(t.generation_flags));
    }
    let (h, v) = (across(t), down(t));
    if h != 0 {
        out.pair(72, h);
    }
    if h != 0 || v != 0 {
        out.point3(11, x, y, 0.0);
    }
    Ok(())
}

/// The part after the text: tag, modes, length, vertical alignment, lock.
fn attribute_part(
    out: &mut Out,
    tag: &str,
    flags: AttributeFlags,
    length: u16,
    t: &AttributeText,
    lock: bool,
) {
    out.pair(2, tag);
    out.pair(70, i32::from(flags.0));
    if length != 0 {
        out.pair(73, i32::from(length));
    }
    if down(t) != 0 {
        out.pair(74, down(t));
    }
    if lock {
        out.pair(280, 1);
    }
}

/// The body of an `ATTDEF` (its head is already written).
pub(super) fn attribute_definition(
    out: &mut Out,
    document: &Document,
    d: &AttributeDefinition,
) -> Result<(), FileError> {
    text_part(out, document, &d.text, &d.default_value.display())?;
    out.pair(100, "AcDbAttributeDefinition");
    out.pair(3, &d.prompt);
    attribute_part(
        out,
        &d.tag,
        d.flags,
        d.field_length,
        &d.text,
        d.lock_position,
    );
    Ok(())
}

/// An `INSERT` with its attributes.
pub(super) fn insert(
    out: &mut Out,
    h: &mut Handles,
    ids: &Ids,
    document: &Document,
    (object, insert): (&Object, &Insert),
    owner: u32,
    paper: bool,
) -> Result<(), FileError> {
    let head = (object.id, &object.attrs);
    let handle = entity_head(out, h, ids, document, head, "INSERT", owner, paper);
    out.pair(100, "AcDbBlockReference");
    if !insert.attributes.is_empty() {
        out.pair(66, 1);
    }
    out.pair(2, &insert.block.name);
    out.point3(
        10,
        finite(insert.insertion.x)?,
        finite(insert.insertion.y)?,
        0.0,
    );
    out.pair(41, finite(insert.scale_x)?);
    out.pair(42, finite(insert.scale_y)?);
    out.pair(43, 1.0);
    if insert.rotation != 0.0 {
        out.pair(50, finite(arc::normalize(insert.rotation).to_degrees())?);
    }
    if insert.attributes.is_empty() {
        return Ok(());
    }
    // No object has this id, so nothing is looked up under it.
    let none = EntityId(u64::MAX);
    for r in &insert.attributes {
        let attrs = insert.attribute_attributes(r, &object.attrs);
        entity_head(
            out,
            h,
            ids,
            document,
            (none, &attrs),
            "ATTRIB",
            handle,
            paper,
        );
        let text = insert.attribute_text_in_drawing(&r.text);
        text_part(out, document, &text, &r.value.display())?;
        out.pair(100, "AcDbAttribute");
        attribute_part(out, &r.tag, r.flags, r.field_length, &text, r.lock_position);
    }
    entity_head(out, h, ids, document, head, "SEQEND", handle, paper);
    Ok(())
}
