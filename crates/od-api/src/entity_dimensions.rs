//! The read-only summary of a dimension: its kind and defining points.

use od_core::{DimKind, Point};

use crate::entity::{GeometryDto, PointDto};

fn named_points(points: &[(&str, Point)]) -> Vec<(String, PointDto)> {
    points
        .iter()
        .map(|(name, p)| ((*name).to_owned(), (*p).into()))
        .collect()
}

pub(crate) fn geometry(d: &od_core::Dimension) -> GeometryDto {
    let (dimension, points) = match &d.kind {
        DimKind::Linear {
            p1,
            p2,
            line_point,
            rotation,
            ..
        } => (
            if rotation.is_some() {
                "linear"
            } else {
                "aligned"
            },
            named_points(&[("p1", *p1), ("p2", *p2), ("line_point", *line_point)]),
        ),
        DimKind::Radial {
            center,
            chord_point,
            diameter,
            ..
        } => (
            if *diameter { "diameter" } else { "radius" },
            named_points(&[("center", *center), ("chord_point", *chord_point)]),
        ),
        DimKind::Angular {
            vertex,
            p1,
            p2,
            arc_point,
            ..
        } => (
            "angular",
            named_points(&[
                ("vertex", *vertex),
                ("p1", *p1),
                ("p2", *p2),
                ("arc_point", *arc_point),
            ]),
        ),
        DimKind::ArcLength {
            center,
            start,
            end,
            arc_point,
            ..
        } => (
            "arc_length",
            named_points(&[
                ("center", *center),
                ("start", *start),
                ("end", *end),
                ("arc_point", *arc_point),
            ]),
        ),
    };
    GeometryDto::Dimension {
        dimension: dimension.to_owned(),
        points,
        text: match &d.text {
            od_core::DimText::Measured => "measured",
            od_core::DimText::Hidden => "hidden",
            od_core::DimText::Custom(_) => "custom",
        }
        .to_owned(),
    }
}
