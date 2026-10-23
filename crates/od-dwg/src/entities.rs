//! The geometry of the entities the importer supports, as stored (Open Design
//! Specification §20.4.x): LINE, CIRCLE, ARC, ELLIPSE, LWPOLYLINE, the old 2D
//! POLYLINE with its VERTEX records, and INSERT. Circles, arcs and polylines are in their
//! own coordinate system; `geometry` says how to read that. An ellipse is not:
//! its centre and axis are world coordinates.

use crate::bit::BitReader;
use crate::error::{DwgError, DwgErrorKind};
use crate::handles::DwgHandle;
use crate::limits::{check, DwgLimits};
use crate::object::ObjectStreams;

pub(crate) type Point3 = [f64; 3];

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LineData {
    pub start: Point3,
    pub end: Point3,
    pub extrusion: Point3,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PointData {
    pub at: Point3,
    pub extrusion: Point3,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CircleData {
    pub center: Point3,
    pub radius: f64,
    pub extrusion: Point3,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ArcData {
    pub center: Point3,
    pub radius: f64,
    /// Counter-clockwise radians, in the entity's own system.
    pub start: f64,
    pub end: f64,
    pub extrusion: Point3,
}

/// An ellipse or elliptical arc. Unlike a circle, its centre and major axis
/// are world coordinates; the extrusion only says which way the parameters
/// run.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct EllipseData {
    pub center: Point3,
    /// From the centre to the end of the major axis.
    pub major_axis: Point3,
    pub extrusion: Point3,
    /// Minor radius over major radius.
    pub ratio: f64,
    /// Counter-clockwise parameters (eccentric anomaly) seen down the
    /// extrusion.
    pub start: f64,
    pub end: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LwPolylineData {
    pub closed: bool,
    pub elevation: f64,
    pub extrusion: Point3,
    pub constant_width: Option<f64>,
    pub points: Vec<[f64; 2]>,
    /// Empty, or one per point.
    pub bulges: Vec<f64>,
    /// Empty, or one (start, end) per point.
    pub widths: Vec<[f64; 2]>,
}

/// The header of an old-style polyline; its vertices are separate objects.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Polyline2dData {
    pub flags: u16,
    /// 0 none, 5 quadratic B-spline, 6 cubic B-spline, 8 Bezier.
    pub curve_type: u16,
    pub elevation: f64,
    pub extrusion: Point3,
    pub vertices: Vec<DwgHandle>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Vertex2dData {
    pub flags: u8,
    pub point: Point3,
    pub start_width: f64,
    pub end_width: f64,
    pub bulge: f64,
}

/// A block reference. Its insertion point and rotation are in the system
/// its extrusion gives, as for a circle.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct InsertData {
    pub insertion: Point3,
    pub scale: Point3,
    /// Counter-clockwise radians.
    pub rotation: f64,
    pub extrusion: Point3,
    /// The block record it places.
    pub block: Option<DwgHandle>,
    /// How many attributes (ATTRIB) it owns.
    pub attributes: u32,
    /// The handles of those attributes, in order.
    pub attribute_handles: Vec<DwgHandle>,
}

fn point3(r: &mut BitReader<'_>) -> Result<Point3, DwgError> {
    r.bd3()
}

/// LINE (§20.4.21). From R2000 the second point is stored as differences from
/// the first in the "default" form, and Z is left out when both are zero.
pub(crate) fn read_line(s: &mut ObjectStreams<'_>) -> Result<LineData, DwgError> {
    let d = &mut s.data;
    let flat = d.bit()?;
    let sx = d.rd()?;
    let ex = d.dd(sx)?;
    let sy = d.rd()?;
    let ey = d.dd(sy)?;
    let (sz, ez) = if flat {
        (0.0, 0.0)
    } else {
        let sz = d.rd()?;
        (sz, d.dd(sz)?)
    };
    d.thickness()?;
    let extrusion = d.extrusion()?;
    Ok(LineData {
        start: [sx, sy, sz],
        end: [ex, ey, ez],
        extrusion,
    })
}

/// POINT (§20.4.28): the point, thickness, extrusion and the angle of the
/// X axis for a point style that turns (not used).
pub(crate) fn read_point(s: &mut ObjectStreams<'_>) -> Result<PointData, DwgError> {
    let d = &mut s.data;
    let at = point3(d)?;
    d.thickness()?;
    let extrusion = d.extrusion()?;
    d.bd()?;
    Ok(PointData { at, extrusion })
}

/// CIRCLE (§20.4.20).
pub(crate) fn read_circle(s: &mut ObjectStreams<'_>) -> Result<CircleData, DwgError> {
    let d = &mut s.data;
    let center = point3(d)?;
    let radius = d.bd()?;
    d.thickness()?;
    let extrusion = d.extrusion()?;
    Ok(CircleData {
        center,
        radius,
        extrusion,
    })
}

/// ARC (§20.4.18).
pub(crate) fn read_arc(s: &mut ObjectStreams<'_>) -> Result<ArcData, DwgError> {
    let d = &mut s.data;
    let center = point3(d)?;
    let radius = d.bd()?;
    d.thickness()?;
    let extrusion = d.extrusion()?;
    let start = d.bd()?;
    let end = d.bd()?;
    Ok(ArcData {
        center,
        radius,
        start,
        end,
        extrusion,
    })
}

/// ELLIPSE (§20.4.39). The extrusion is three plain doubles here, not the
/// compressed form a circle uses, and there is no thickness.
pub(crate) fn read_ellipse(s: &mut ObjectStreams<'_>) -> Result<EllipseData, DwgError> {
    let d = &mut s.data;
    let center = point3(d)?;
    let major_axis = point3(d)?;
    let extrusion = point3(d)?;
    let ratio = d.bd()?;
    let start = d.bd()?;
    let end = d.bd()?;
    Ok(EllipseData {
        center,
        major_axis,
        extrusion,
        ratio,
        start,
        end,
    })
}

/// A count read from the data, checked against the limit and against the bits
/// left to hold that many items of at least `min_bits` each.
pub(crate) fn count(
    d: &BitReader<'_>,
    n: u32,
    min_bits: u64,
    limits: &DwgLimits,
    what: &'static str,
) -> Result<usize, DwgError> {
    check(d.stage(), what, u64::from(n), limits.max_collection_items)?;
    if u64::from(n) * min_bits > d.remaining() {
        return Err(d.error(DwgErrorKind::Truncated));
    }
    Ok(n as usize)
}

/// LWPOLYLINE (§20.4.85, R2010 layout).
pub(crate) fn read_lwpolyline(
    s: &mut ObjectStreams<'_>,
    limits: &DwgLimits,
) -> Result<LwPolylineData, DwgError> {
    let d = &mut s.data;
    let flag = d.bs()?;
    let constant_width = if flag & 4 != 0 { Some(d.bd()?) } else { None };
    let elevation = if flag & 8 != 0 { d.bd()? } else { 0.0 };
    if flag & 2 != 0 {
        d.bd()?; // thickness
    }
    let extrusion = if flag & 1 != 0 {
        d.bd3()?
    } else {
        [0.0, 0.0, 1.0]
    };
    let n_points = d.bl()?;
    let n_bulges = if flag & 16 != 0 { d.bl()? } else { 0 };
    let n_ids = if flag & 1024 != 0 { d.bl()? } else { 0 };
    let n_widths = if flag & 32 != 0 { d.bl()? } else { 0 };
    let n_points = count(d, n_points, 4, limits, "polyline vertices")?;
    let n_bulges = count(d, n_bulges, 2, limits, "polyline vertices")?;
    let n_ids = count(d, n_ids, 2, limits, "polyline vertices")?;
    let n_widths = count(d, n_widths, 4, limits, "polyline vertices")?;

    let mut points: Vec<[f64; 2]> = Vec::with_capacity(n_points);
    if n_points > 0 {
        points.push(d.rd2()?);
    }
    for _ in 1..n_points {
        let prev = points.last().copied().unwrap_or([0.0, 0.0]);
        points.push([d.dd(prev[0])?, d.dd(prev[1])?]);
    }
    let bulges = (0..n_bulges)
        .map(|_| d.bd())
        .collect::<Result<Vec<_>, _>>()?;
    for _ in 0..n_ids {
        d.bl()?;
    }
    let widths = (0..n_widths)
        .map(|_| Ok([d.bd()?, d.bd()?]))
        .collect::<Result<Vec<_>, DwgError>>()?;
    // The closed flag is bit 9 here (it is bit 0 of the DXF group).
    Ok(LwPolylineData {
        closed: flag & 512 != 0,
        elevation,
        extrusion,
        constant_width,
        points,
        bulges,
        widths,
    })
}

/// 2D POLYLINE (§20.4.16): the header, and from the handle stream the handles
/// of its vertices (the SEQEND handle after them is not needed).
pub(crate) fn read_polyline2d(
    s: &mut ObjectStreams<'_>,
    limits: &DwgLimits,
) -> Result<Polyline2dData, DwgError> {
    let d = &mut s.data;
    let flags = d.bs()?;
    let curve_type = d.bs()?;
    d.bd()?; // default start width
    d.bd()?; // default end width
    d.thickness()?;
    let elevation = d.bd()?;
    let extrusion = d.extrusion()?;
    let owned = d.bl()?;
    check(
        d.stage(),
        "polyline vertices",
        u64::from(owned),
        limits.max_collection_items,
    )?;
    let h = &mut s.handles;
    if u64::from(owned) * 8 > h.remaining() {
        return Err(h.error(DwgErrorKind::Truncated));
    }
    let mut vertices = Vec::with_capacity(owned as usize);
    for _ in 0..owned {
        let reference = h.handle(limits.max_handle_bytes)?;
        let handle = reference
            .resolve(DwgHandle(0))
            .map_err(|e| e.at(h.position()))?;
        vertices.push(handle.ok_or_else(|| h.error(DwgErrorKind::InvalidHandle))?);
    }
    Ok(Polyline2dData {
        flags,
        curve_type,
        elevation,
        extrusion,
        vertices,
    })
}

/// INSERT (§20.4.9, R2004 and later). The scale is stored by a two-bit
/// form: 3 is (1, 1, 1); 2 one value for all three; 1 an X of 1 with Y and Z
/// defaulting to it; 0 X then Y and Z defaulting to X. After the common
/// handles come the block record, then each owned ATTRIB and their SEQEND.
pub(crate) fn read_insert(
    s: &mut ObjectStreams<'_>,
    base: DwgHandle,
    limits: &DwgLimits,
) -> Result<InsertData, DwgError> {
    let d = &mut s.data;
    let insertion = d.bd3()?;
    let scale = match d.bb()? {
        3 => [1.0; 3],
        2 => {
            let x = d.rd()?;
            [x; 3]
        }
        1 => [1.0, d.dd(1.0)?, d.dd(1.0)?],
        _ => {
            let x = d.rd()?;
            [x, d.dd(x)?, d.dd(x)?]
        }
    };
    let rotation = d.bd()?;
    let extrusion = d.bd3()?;
    let attributes = if d.bit()? { d.bl()? } else { 0 };
    check(
        d.stage(),
        "block attributes",
        u64::from(attributes),
        limits.max_collection_items,
    )?;
    let h = &mut s.handles;
    let block = h
        .handle(limits.max_handle_bytes)?
        .resolve(base)
        .map_err(|e| e.at(h.position()))?;
    if u64::from(attributes) * 8 > h.remaining() {
        return Err(h.error(DwgErrorKind::Truncated));
    }
    let mut attribute_handles = Vec::with_capacity(attributes as usize);
    for _ in 0..attributes {
        let reference = h.handle(limits.max_handle_bytes)?;
        let handle = reference.resolve(base).map_err(|e| e.at(h.position()))?;
        attribute_handles.extend(handle);
    }
    Ok(InsertData {
        insertion,
        scale,
        rotation,
        extrusion,
        block,
        attributes,
        attribute_handles,
    })
}

/// VERTEX (2D) (§20.4.11). A negative start width stands for both widths.
pub(crate) fn read_vertex2d(s: &mut ObjectStreams<'_>) -> Result<Vertex2dData, DwgError> {
    let d = &mut s.data;
    let flags = d.rc()?;
    let point = point3(d)?;
    let start = d.bd()?;
    let (start_width, end_width) = if start < 0.0 {
        (-start, -start)
    } else {
        (start, d.bd()?)
    };
    let bulge = d.bd()?;
    d.bl()?; // vertex id
    d.bd()?; // tangent direction
    Ok(Vertex2dData {
        flags,
        point,
        start_width,
        end_width,
        bulge,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{BitWriter, Buffers};

    fn with_data(w: &BitWriter) -> Buffers {
        Buffers::new(w, &BitWriter::new(), &BitWriter::new())
    }

    #[test]
    fn a_line_with_defaulted_second_point() {
        let mut w = BitWriter::new();
        w.bit(true)
            .rd(1.0)
            .dd_default()
            .rd(2.0)
            .dd_full(5.5)
            .thickness(0.0)
            .extrusion([0.0, 0.0, 1.0]);
        let b = with_data(&w);
        let line = read_line(&mut b.streams()).unwrap();
        assert_eq!(line.start, [1.0, 2.0, 0.0]);
        assert_eq!(line.end, [1.0, 5.5, 0.0]);
        assert_eq!(line.extrusion, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn a_line_with_z_values() {
        let mut w = BitWriter::new();
        w.bit(false)
            .rd(0.0)
            .dd_full(3.0)
            .rd(0.0)
            .dd_full(4.0)
            .rd(7.0)
            .dd_default();
        w.thickness(0.0).extrusion([0.0, 0.0, -1.0]);
        let b = with_data(&w);
        let line = read_line(&mut b.streams()).unwrap();
        assert_eq!(line.start, [0.0, 0.0, 7.0]);
        assert_eq!(line.end, [3.0, 4.0, 7.0]);
        assert_eq!(line.extrusion, [0.0, 0.0, -1.0]);
    }

    #[test]
    fn a_point() {
        let mut w = BitWriter::new();
        w.bd3([3.0, -4.0, 0.0])
            .thickness(0.0)
            .extrusion([0.0, 0.0, 1.0])
            .bd(0.5);
        let b = with_data(&w);
        let point = read_point(&mut b.streams()).unwrap();
        assert_eq!(point.at, [3.0, -4.0, 0.0]);
        assert_eq!(point.extrusion, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn a_circle_and_an_arc() {
        let mut w = BitWriter::new();
        w.bd3([1.0, 2.0, 0.0])
            .bd(5.0)
            .thickness(0.0)
            .extrusion([0.0, 0.0, 1.0]);
        let b = with_data(&w);
        let c = read_circle(&mut b.streams()).unwrap();
        assert_eq!((c.center, c.radius), ([1.0, 2.0, 0.0], 5.0));

        let mut w = BitWriter::new();
        w.bd3([1.0, 2.0, 3.0])
            .bd(5.0)
            .thickness(2.0)
            .extrusion([0.0, 0.0, -1.0]);
        w.bd(0.5).bd(1.5);
        let b = with_data(&w);
        let a = read_arc(&mut b.streams()).unwrap();
        assert_eq!(
            (a.center, a.radius, a.start, a.end),
            ([1.0, 2.0, 3.0], 5.0, 0.5, 1.5)
        );
        assert_eq!(a.extrusion, [0.0, 0.0, -1.0]);
    }

    #[test]
    fn an_ellipse_is_centre_axis_extrusion_ratio_and_parameters() {
        let mut w = BitWriter::new();
        w.bd3([1.0, 2.0, 3.0])
            .bd3([4.0, 0.0, 0.0])
            .bd3([0.0, 0.0, -1.0]);
        w.bd(0.25).bd(0.5).bd(2.0);
        let b = with_data(&w);
        let e = read_ellipse(&mut b.streams()).unwrap();
        assert_eq!(
            (e.center, e.major_axis, e.extrusion),
            ([1.0, 2.0, 3.0], [4.0, 0.0, 0.0], [0.0, 0.0, -1.0])
        );
        assert_eq!((e.ratio, e.start, e.end), (0.25, 0.5, 2.0));
    }

    #[test]
    fn a_closed_lwpolyline_with_bulges_and_widths() {
        let mut w = BitWriter::new();
        // closed (512) + bulges (16) + widths (32) + elevation (8)
        w.bs(512 | 16 | 32 | 8).bd(2.5).bl(3).bl(3).bl(3);
        w.rd2([0.0, 0.0])
            .dd_full(10.0)
            .dd_default()
            .dd_default()
            .dd_full(5.0);
        w.bd(0.0).bd(1.0).bd(-0.5);
        for _ in 0..3 {
            w.bd(0.1).bd(0.2);
        }
        let b = with_data(&w);
        let p = read_lwpolyline(&mut b.streams(), &DwgLimits::default()).unwrap();
        assert!(p.closed);
        assert_eq!(p.elevation, 2.5);
        assert_eq!(p.points, [[0.0, 0.0], [10.0, 0.0], [10.0, 5.0]]);
        assert_eq!(p.bulges, [0.0, 1.0, -0.5]);
        assert_eq!(p.widths.len(), 3);
        assert_eq!(p.widths[0], [0.1, 0.2]);
        assert_eq!(p.constant_width, None);
        assert_eq!(p.extrusion, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn an_open_lwpolyline_with_constant_width_and_a_normal() {
        let mut w = BitWriter::new();
        w.bs(4 | 1).bd(0.25).bd3([0.0, 0.0, -1.0]).bl(2);
        w.rd2([1.0, 1.0]).dd_full(2.0).dd_full(3.0);
        let b = with_data(&w);
        let p = read_lwpolyline(&mut b.streams(), &DwgLimits::default()).unwrap();
        assert!(!p.closed);
        assert_eq!(p.constant_width, Some(0.25));
        assert_eq!(p.extrusion, [0.0, 0.0, -1.0]);
        assert_eq!(p.points, [[1.0, 1.0], [2.0, 3.0]]);
        assert!(p.bulges.is_empty() && p.widths.is_empty());
    }

    #[test]
    fn lwpolyline_counts_are_checked_before_allocating() {
        let mut w = BitWriter::new();
        w.bs(0).bl(4_000_000_000);
        let b = with_data(&w);
        let err = read_lwpolyline(&mut b.streams(), &DwgLimits::default()).unwrap_err();
        assert!(matches!(err.kind, DwgErrorKind::LimitExceeded { .. }));

        let mut w = BitWriter::new();
        w.bs(0).bl(1000).rd2([0.0, 0.0]);
        let b = with_data(&w);
        let err = read_lwpolyline(&mut b.streams(), &DwgLimits::default()).unwrap_err();
        assert_eq!(err.kind, DwgErrorKind::Truncated);
    }

    #[test]
    fn a_polyline_header_and_its_vertex_handles() {
        let mut data = BitWriter::new();
        data.bs(1)
            .bs(0)
            .bd(0.0)
            .bd(0.0)
            .thickness(0.0)
            .bd(1.5)
            .extrusion([0.0, 0.0, 1.0])
            .bl(3);
        let mut handles = BitWriter::new();
        handles
            .handle(4, 0x51)
            .handle(4, 0x52)
            .handle(4, 0x53)
            .handle(4, 0x54);
        let b = Buffers::new(&data, &BitWriter::new(), &handles);
        let p = read_polyline2d(&mut b.streams(), &DwgLimits::default()).unwrap();
        assert_eq!((p.flags, p.curve_type, p.elevation), (1, 0, 1.5));
        assert_eq!(
            p.vertices,
            [DwgHandle(0x51), DwgHandle(0x52), DwgHandle(0x53)]
        );
    }

    #[test]
    fn a_polyline_cannot_own_more_vertices_than_handles_exist() {
        let mut data = BitWriter::new();
        data.bs(0)
            .bs(0)
            .bd(0.0)
            .bd(0.0)
            .thickness(0.0)
            .bd(0.0)
            .extrusion([0.0, 0.0, 1.0])
            .bl(50);
        let b = Buffers::new(&data, &BitWriter::new(), BitWriter::new().handle(4, 5));
        let err = read_polyline2d(&mut b.streams(), &DwgLimits::default()).unwrap_err();
        assert_eq!(err.kind, DwgErrorKind::Truncated);
    }

    fn insert_handles() -> BitWriter {
        let mut h = BitWriter::new();
        h.handle(5, 0x1F);
        h
    }

    #[test]
    fn an_insert_at_full_size() {
        let mut w = BitWriter::new();
        w.bd3([30.0, 25.0, 0.0])
            .bits(3, 2)
            .bd(0.0)
            .bd3([0.0, 0.0, 1.0])
            .bit(false);
        let b = Buffers::new(&w, &BitWriter::new(), &insert_handles());
        let i = read_insert(&mut b.streams(), DwgHandle(0x40), &DwgLimits::default()).unwrap();
        assert_eq!(i.insertion, [30.0, 25.0, 0.0]);
        assert_eq!(i.scale, [1.0; 3]);
        assert_eq!((i.rotation, i.attributes), (0.0, 0));
        assert_eq!(i.block, Some(DwgHandle(0x1F)));
        assert!(i.attribute_handles.is_empty());
    }

    #[test]
    fn an_insert_lists_the_attributes_it_owns() {
        let mut w = BitWriter::new();
        w.bd3([0.0; 3])
            .bits(3, 2)
            .bd(0.0)
            .bd3([0.0, 0.0, 1.0])
            .bit(true)
            .bl(2);
        let mut h = insert_handles();
        h.handle(4, 0x41).handle(4, 0x42);
        let b = Buffers::new(&w, &BitWriter::new(), &h);
        let i = read_insert(&mut b.streams(), DwgHandle(0x40), &DwgLimits::default()).unwrap();
        assert_eq!(i.attributes, 2);
        assert_eq!(i.attribute_handles, [DwgHandle(0x41), DwgHandle(0x42)]);
    }

    #[test]
    fn insert_scale_forms() {
        let scale = |write: &dyn Fn(&mut BitWriter)| {
            let mut w = BitWriter::new();
            w.bd3([0.0; 3]);
            write(&mut w);
            w.bd(0.5).bd3([0.0, 0.0, 1.0]).bit(true).bl(2);
            let mut h = insert_handles();
            h.handle(4, 0x41).handle(4, 0x42);
            let b = Buffers::new(&w, &BitWriter::new(), &h);
            let i = read_insert(&mut b.streams(), DwgHandle(1), &DwgLimits::default()).unwrap();
            assert_eq!((i.rotation, i.attributes), (0.5, 2));
            i.scale
        };
        assert_eq!(
            scale(&|w| {
                w.bits(2, 2).rd(2.5);
            }),
            [2.5; 3]
        );
        assert_eq!(
            scale(&|w| {
                w.bits(1, 2).dd_full(-3.0).dd_default();
            }),
            [1.0, -3.0, 1.0]
        );
        assert_eq!(
            scale(&|w| {
                w.bits(0, 2).rd(-2.0).dd_default().dd_full(4.0);
            }),
            [-2.0, -2.0, 4.0]
        );
    }

    #[test]
    fn vertex_widths_and_the_negative_width_shortcut() {
        let mut w = BitWriter::new();
        w.rc(0)
            .bd3([1.0, 2.0, 0.0])
            .bd(0.5)
            .bd(0.75)
            .bd(0.25)
            .bl(7)
            .bd(0.0);
        let b = with_data(&w);
        let v = read_vertex2d(&mut b.streams()).unwrap();
        assert_eq!(
            (v.point, v.start_width, v.end_width, v.bulge),
            ([1.0, 2.0, 0.0], 0.5, 0.75, 0.25)
        );

        let mut w = BitWriter::new();
        w.rc(16).bd3([0.0, 0.0, 0.0]).bd(-0.4).bd(0.0).bl(0).bd(0.0);
        let b = with_data(&w);
        let v = read_vertex2d(&mut b.streams()).unwrap();
        assert_eq!((v.flags, v.start_width, v.end_width), (16, 0.4, 0.4));
    }
}
