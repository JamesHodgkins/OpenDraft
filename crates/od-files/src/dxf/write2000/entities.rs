//! Viewports and drawing objects as R2000 entities.

use od_core::{arc, Attr, Attributes, Document, Entity, EntityId, Layout, Object, Viewport};

use super::{hex, true_color, weight_code, Handles, Ids};
use crate::dxf::write::{aci_for, dxf_name, finite, Out};
use crate::FileError;

/// The sheet-wide viewport AutoCAD keeps first in a layout, then each viewport.
pub(super) fn write_viewports(
    out: &mut Out,
    h: &mut Handles,
    ids: &Ids,
    layout: &Layout,
    index: usize,
) -> Result<(), FileError> {
    let owner = ids.records[index];
    let (w, hgt) = (layout.width, layout.height);
    let viewport = |out: &mut Out,
                    handle: u32,
                    v: &Viewport,
                    status: i32,
                    id: i32,
                    flags: i32|
     -> Result<(), FileError> {
        out.pair(0, "VIEWPORT");
        out.pair(5, hex(handle));
        out.pair(330, hex(owner));
        out.pair(100, "AcDbEntity");
        out.pair(67, 1);
        out.pair(8, 0);
        out.pair(100, "AcDbViewport");
        out.point3(10, finite(v.center.x)?, finite(v.center.y)?, 0.0);
        out.pair(40, finite(v.width)?);
        out.pair(41, finite(v.height)?);
        out.pair(68, status);
        out.pair(69, id);
        out.pair(12, finite(v.view_center.x)?);
        out.pair(22, finite(v.view_center.y)?);
        out.pair(13, 0.0);
        out.pair(23, 0.0);
        out.pair(14, 10.0);
        out.pair(24, 10.0);
        out.pair(15, 10.0);
        out.pair(25, 10.0);
        out.point3(16, 0.0, 0.0, 1.0);
        out.point3(17, 0.0, 0.0, 0.0);
        out.pair(42, 50.0);
        out.pair(43, 0.0);
        out.pair(44, 0.0);
        out.pair(45, finite(v.height / v.scale)?);
        out.pair(50, 0.0);
        out.pair(51, 0.0);
        out.pair(72, 100);
        for layer in &v.frozen {
            if let Some(handle) = ids.layers.get(layer) {
                out.pair(331, hex(*handle));
            }
        }
        out.pair(90, flags);
        out.pair(1, "");
        out.pair(281, 0);
        out.pair(71, 0);
        out.pair(74, 0);
        out.point3(110, 0.0, 0.0, 0.0);
        out.point3(111, 1.0, 0.0, 0.0);
        out.point3(112, 0.0, 1.0, 0.0);
        out.pair(79, 0);
        out.pair(146, 0.0);
        Ok(())
    };
    let sheet = Viewport {
        id: od_core::ViewportId(0),
        center: od_core::Point::new(w / 2.0, hgt / 2.0),
        width: w * 1.1,
        height: hgt * 1.1,
        view_center: od_core::Point::new(w / 2.0, hgt / 2.0),
        scale: 1.0,
        locked: false,
        on: true,
        frozen: Vec::new(),
    };
    viewport(out, ids.overall[index], &sheet, 1, 1, 557_088)?;
    for (i, v) in layout.viewports.iter().enumerate() {
        let status = if v.on {
            i32::try_from(i + 2).unwrap_or(2)
        } else {
            0
        };
        let id = i32::try_from(i + 2).unwrap_or(2);
        viewport(
            out,
            h.next(),
            v,
            status,
            id,
            if v.locked { 0x4000 } else { 0 },
        )?;
    }
    Ok(())
}

/// The start of an entity: its kind, handle, owner, layer and properties.
/// Returns the handle (an `INSERT`'s attributes are owned by it).
#[allow(clippy::too_many_arguments)]
pub(super) fn entity_head(
    out: &mut Out,
    h: &mut Handles,
    ids: &Ids,
    document: &Document,
    (id, attrs): (EntityId, &Attributes),
    kind: &str,
    owner: u32,
    paper: bool,
) -> u32 {
    let layer = document
        .layer(attrs.layer)
        .map_or_else(|| "0".to_owned(), |l| dxf_name(&l.name));
    out.pair(0, kind);
    let handle = ids
        .annotations
        .get(&id)
        .copied()
        .unwrap_or_else(|| h.next());
    out.pair(5, hex(handle));
    out.pair(330, hex(owner));
    out.pair(100, "AcDbEntity");
    if paper {
        out.pair(67, 1);
    }
    out.pair(8, &layer);
    match &attrs.linetype {
        Attr::Set(id) => out.pair(6, crate::dxf::linetypes::name_for(document, *id)),
        Attr::ByBlock => out.pair(6, "ByBlock"),
        Attr::ByLayer => {}
    }
    if attrs.linetype_scale != 1.0 {
        out.pair(48, attrs.linetype_scale);
    }
    match attrs.color {
        Attr::Set(color) => out.pair(62, aci_for(color).0),
        Attr::ByBlock => out.pair(62, 0),
        Attr::ByLayer => {}
    }
    match attrs.lineweight {
        Attr::Set(weight) => out.pair(370, weight_code(weight)),
        Attr::ByBlock => out.pair(370, -2),
        Attr::ByLayer => {}
    }
    if let Attr::Set(color) = attrs.color {
        if let Some(rgb) = true_color(color) {
            out.pair(420, rgb);
        }
    }
    handle
}

/// One drawing object as an entity owned by `owner`; `paper` marks sheet entities (group 67).
pub(super) fn write_object(
    out: &mut Out,
    h: &mut Handles,
    ids: &Ids,
    document: &Document,
    object: &Object,
    owner: u32,
    paper: bool,
) -> Result<(), FileError> {
    let head = |out: &mut Out, h: &mut Handles, kind: &str| {
        entity_head(
            out,
            h,
            ids,
            document,
            (object.id, &object.attrs),
            kind,
            owner,
            paper,
        )
    };
    match &object.entity {
        Entity::Dimension(d) => {
            head(out, h, "DIMENSION");
            super::dimension::entity(out, ids, document, object, d)?;
        }
        Entity::Insert(i) => {
            super::insert::insert(out, h, ids, document, (object, i), owner, paper)?
        }
        Entity::AttributeDefinition(d) => {
            head(out, h, "ATTDEF");
            super::insert::attribute_definition(out, document, d)?;
        }
        Entity::Leader(l) => {
            head(out, h, "LEADER");
            super::leader::entity(out, ids, document, l)?;
        }
        Entity::MLeader(m) => {
            head(out, h, "MULTILEADER");
            super::mleader::entity(out, ids, document, m)?;
        }
        Entity::MText(text) => {
            head(out, h, "MTEXT");
            super::text::mtext(out, document, text)?;
        }
        Entity::Text(text) => {
            head(out, h, "TEXT");
            super::text::single_line(out, document, text)?;
        }
        Entity::Hatch(hatch) => {
            head(out, h, "HATCH");
            super::hatch::hatch(out, hatch)?;
        }
        Entity::Xline { origin, direction } | Entity::Ray { origin, direction } => {
            let ray = matches!(object.entity, Entity::Ray { .. });
            head(out, h, if ray { "RAY" } else { "XLINE" });
            out.pair(100, if ray { "AcDbRay" } else { "AcDbXline" });
            out.point3(10, finite(origin.x)?, finite(origin.y)?, 0.0);
            out.point3(11, finite(direction.x)?, finite(direction.y)?, 0.0);
        }
        Entity::Point { at } => {
            head(out, h, "POINT");
            out.pair(100, "AcDbPoint");
            out.point3(10, finite(at.x)?, finite(at.y)?, 0.0);
        }
        Entity::Line { start, end } => {
            head(out, h, "LINE");
            out.pair(100, "AcDbLine");
            out.point3(10, finite(start.x)?, finite(start.y)?, 0.0);
            out.point3(11, finite(end.x)?, finite(end.y)?, 0.0);
        }
        Entity::Circle { center, radius } => {
            head(out, h, "CIRCLE");
            out.pair(100, "AcDbCircle");
            out.point3(10, finite(center.x)?, finite(center.y)?, 0.0);
            out.pair(40, finite(*radius)?);
        }
        Entity::Arc {
            center,
            radius,
            start,
            end,
        } => {
            head(out, h, "ARC");
            out.pair(100, "AcDbCircle");
            out.point3(10, finite(center.x)?, finite(center.y)?, 0.0);
            out.pair(40, finite(*radius)?);
            out.pair(100, "AcDbArc");
            out.pair(50, finite(arc::normalize(*start).to_degrees())?);
            out.pair(51, finite(arc::normalize(*end).to_degrees())?);
        }
        Entity::Ellipse(e) => {
            if let Err(why) = e.validate() {
                return Err(FileError::Invalid(format!(
                    "an ellipse is not valid: {why}"
                )));
            }
            head(out, h, "ELLIPSE");
            out.pair(100, "AcDbEllipse");
            out.point3(10, finite(e.center.x)?, finite(e.center.y)?, 0.0);
            out.point3(11, finite(e.major_axis.x)?, finite(e.major_axis.y)?, 0.0);
            out.point3(210, 0.0, 0.0, 1.0);
            out.pair(40, finite(e.radius_ratio)?);
            out.pair(41, finite(e.start)?);
            out.pair(42, finite(e.end)?);
        }
        Entity::Spline(s) => {
            if let Err(why) = s.validate() {
                return Err(FileError::Invalid(format!("a spline is not valid: {why}")));
            }
            head(out, h, "SPLINE");
            spline(out, s)?;
        }
        Entity::Rectangle { min, max } => {
            let corners = [
                od_core::Point::new(min.x, min.y),
                od_core::Point::new(max.x, min.y),
                od_core::Point::new(max.x, max.y),
                od_core::Point::new(min.x, max.y),
            ];
            head(out, h, "LWPOLYLINE");
            lwpolyline(
                out,
                &corners,
                &[],
                &[],
                true,
                object.attrs.continuous_generation,
            )?;
        }
        Entity::Polyline {
            points,
            closed,
            bulges,
            widths,
        } => {
            head(out, h, "LWPOLYLINE");
            lwpolyline(
                out,
                points,
                bulges,
                widths,
                *closed,
                object.attrs.continuous_generation,
            )?;
        }
    }
    Ok(())
}

/// A spline's `AcDbSpline` part: flags, counts, tolerances, then every knot,
/// weight (rational splines only), control point and fit point.
fn spline(out: &mut Out, s: &od_core::Spline) -> Result<(), FileError> {
    let fit = s.fit.as_ref();
    let flags = i32::from(s.closed)
        | if s.periodic { 2 } else { 0 }
        | if s.is_rational() { 4 } else { 0 }
        | 8
        | if s.degree == 1 { 16 } else { 0 };
    out.pair(100, "AcDbSpline");
    out.point3(210, 0.0, 0.0, 1.0);
    out.pair(70, flags);
    out.pair(71, s.degree);
    out.pair(72, s.knots.len());
    out.pair(73, s.control_points.len());
    out.pair(74, fit.map_or(0, |f| f.points.len()));
    out.pair(42, finite(s.knot_tolerance)?);
    out.pair(43, finite(s.control_point_tolerance)?);
    let fit_tolerance = fit.map_or(od_core::spline::DEFAULT_FIT_TOLERANCE, |f| f.tolerance);
    out.pair(44, finite(fit_tolerance)?);
    if let Some(t) = fit.and_then(|f| f.start_tangent) {
        out.point3(12, finite(t.x)?, finite(t.y)?, 0.0);
    }
    if let Some(t) = fit.and_then(|f| f.end_tangent) {
        out.point3(13, finite(t.x)?, finite(t.y)?, 0.0);
    }
    for k in &s.knots {
        out.pair(40, finite(*k)?);
    }
    for w in s.weights.iter().flatten() {
        out.pair(41, finite(*w)?);
    }
    for q in &s.control_points {
        out.point3(10, finite(q.x)?, finite(q.y)?, 0.0);
    }
    for q in fit.map_or(&[][..], |f| &f.points) {
        out.point3(11, finite(q.x)?, finite(q.y)?, 0.0);
    }
    Ok(())
}

fn lwpolyline(
    out: &mut Out,
    points: &[od_core::Point],
    bulges: &[f64],
    widths: &[(f64, f64)],
    closed: bool,
    continuous: bool,
) -> Result<(), FileError> {
    out.pair(100, "AcDbPolyline");
    out.pair(90, points.len());
    out.pair(70, i32::from(closed) | if continuous { 128 } else { 0 });
    for (i, p) in points.iter().enumerate() {
        out.pair(10, finite(p.x)?);
        out.pair(20, finite(p.y)?);
        let (start, end) = od_core::width::at(widths, i);
        if start != 0.0 || end != 0.0 {
            out.pair(40, finite(start)?);
            out.pair(41, finite(end)?);
        }
        let bulge = od_core::bulge::at(bulges, i);
        if bulge != 0.0 {
            out.pair(42, finite(bulge)?);
        }
    }
    Ok(())
}
