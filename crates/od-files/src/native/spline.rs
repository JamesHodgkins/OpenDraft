//! The native format's description of a spline: its control frame exactly,
//! and its fit data when it has some. Nothing derived is stored.

use od_core::{KnotParameterization, Point, Spline, SplineFitData};
use serde::{Deserialize, Serialize};

use super::entity::{from_point, invalid, point, PointDto};
use crate::FileError;

/// Most control points or fit points a spline read from a file may have.
pub(crate) const MAX_FILE_SPLINE_POINTS: usize = 100_000;

#[derive(Serialize, Deserialize)]
pub(super) struct SplineDto {
    degree: u8,
    control_points: Vec<PointDto>,
    knots: Vec<f64>,
    /// One per control point; left out when every weight is 1.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    weights: Vec<f64>,
    #[serde(default)]
    closed: bool,
    #[serde(default)]
    periodic: bool,
    #[serde(default = "default_knot_tolerance")]
    knot_tolerance: f64,
    #[serde(default = "default_control_tolerance")]
    control_point_tolerance: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fit: Option<FitDataDto>,
}

#[derive(Serialize, Deserialize)]
struct FitDataDto {
    points: Vec<PointDto>,
    #[serde(default)]
    tolerance: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    start_tangent: Option<PointDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    end_tangent: Option<PointDto>,
    /// `chord`, `sqrt_chord` or `uniform`; left out when not known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    knots: Option<String>,
}

fn default_knot_tolerance() -> f64 {
    od_core::spline::DEFAULT_KNOT_TOLERANCE
}

fn default_control_tolerance() -> f64 {
    od_core::spline::DEFAULT_CONTROL_TOLERANCE
}

fn knots_name(how: KnotParameterization) -> &'static str {
    match how {
        KnotParameterization::Chord => "chord",
        KnotParameterization::SqrtChord => "sqrt_chord",
        KnotParameterization::Uniform => "uniform",
    }
}

fn knots_from(name: &str) -> Result<KnotParameterization, FileError> {
    match name {
        "chord" => Ok(KnotParameterization::Chord),
        "sqrt_chord" => Ok(KnotParameterization::SqrtChord),
        "uniform" => Ok(KnotParameterization::Uniform),
        other => invalid(format!("a spline has unknown knot spacing \"{other}\"")),
    }
}

fn points(dtos: &[PointDto]) -> Result<Vec<Point>, FileError> {
    if dtos.len() > MAX_FILE_SPLINE_POINTS {
        return invalid("a spline has more points than this build reads");
    }
    dtos.iter().map(|p| from_point(*p)).collect()
}

impl SplineDto {
    pub(super) fn from_spline(s: &Spline) -> Result<Self, FileError> {
        if let Err(why) = s.validate() {
            return invalid(format!("a spline is not valid: {why}"));
        }
        Ok(Self {
            degree: s.degree,
            control_points: s.control_points.iter().map(|p| point(*p)).collect(),
            knots: s.knots.clone(),
            weights: s.weights.clone().unwrap_or_default(),
            closed: s.closed,
            periodic: s.periodic,
            knot_tolerance: s.knot_tolerance,
            control_point_tolerance: s.control_point_tolerance,
            fit: s.fit.as_ref().map(|f| FitDataDto {
                points: f.points.iter().map(|p| point(*p)).collect(),
                tolerance: f.tolerance,
                start_tangent: f.start_tangent.map(point),
                end_tangent: f.end_tangent.map(point),
                knots: f.parameterization.map(|k| knots_name(k).to_owned()),
            }),
        })
    }

    pub(super) fn to_spline(&self) -> Result<Spline, FileError> {
        let fit = match &self.fit {
            None => None,
            Some(f) => Some(SplineFitData {
                points: points(&f.points)?,
                tolerance: f.tolerance,
                start_tangent: f.start_tangent.map(from_point).transpose()?,
                end_tangent: f.end_tangent.map(from_point).transpose()?,
                parameterization: f.knots.as_deref().map(knots_from).transpose()?,
            }),
        };
        let spline = Spline {
            degree: self.degree,
            control_points: points(&self.control_points)?,
            knots: self.knots.clone(),
            weights: (!self.weights.is_empty()).then(|| self.weights.clone()),
            closed: self.closed,
            periodic: self.periodic,
            knot_tolerance: self.knot_tolerance,
            control_point_tolerance: self.control_point_tolerance,
            fit,
        };
        match spline.validate() {
            Ok(()) => Ok(spline),
            Err(why) => invalid(format!("a spline is not valid: {why}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use od_core::{Document, Entity, Spline};

    use crate::native::NativeFormat;
    use crate::{FileError, FileFormat};

    fn with_spline(fields: &str) -> String {
        format!(
            r#"{{ "format": "opendraft-drawing", "version": 0,
            "drawing": {{ "next_object_id": 1, "next_layer_id": 1, "current_layer": 0,
                "layers": [ {{ "id": 0, "name": "0", "color": "foreground",
                              "linetype": "Continuous", "lineweight": 25 }} ],
                "objects": [ {{ "id": 0, "layer": 0,
                    "entity": {{ "type": "spline", {fields} }} }} ] }} }}"#
        )
    }

    fn read(text: &str) -> Result<Document, FileError> {
        crate::native::tests::read(text)
    }

    const GOOD: &str = r#""degree": 2, "control_points": [[0, 0], [1, 1], [2, 0]],
        "knots": [0, 0, 0, 1, 1, 1]"#;

    #[test]
    fn a_plain_spline_needs_only_its_frame_and_writes_no_weights() {
        let document = read(&with_spline(GOOD)).unwrap();
        let Entity::Spline(s) = &document.objects()[0].entity else {
            panic!("a spline")
        };
        assert_eq!((s.degree, s.weights.clone(), s.closed), (2, None, false));
        assert_eq!(s.knot_tolerance, od_core::spline::DEFAULT_KNOT_TOLERANCE);
        let text = String::from_utf8(NativeFormat.write(&document).unwrap().bytes).unwrap();
        assert!(text.contains("\"type\": \"spline\""));
        let file: serde_json::Value = serde_json::from_str(&text).unwrap();
        let entity = &file["drawing"]["objects"][0]["entity"];
        assert!(entity.get("weights").is_none() && entity.get("fit").is_none());
    }

    #[test]
    fn a_spline_that_could_not_exist_is_refused_whole() {
        for bad in [
            GOOD.replace("[0, 0, 0, 1, 1, 1]", "[0, 0, 1, 1, 1]"),
            GOOD.replace("[0, 0, 0, 1, 1, 1]", "[1, 0, 0, 1, 1, 1]"),
            GOOD.replace("\"degree\": 2", "\"degree\": 3"),
            format!("{GOOD}, \"weights\": [1, -1, 1]"),
            format!("{GOOD}, \"weights\": [1, 2]"),
            format!("{GOOD}, \"periodic\": true"),
            format!("{GOOD}, \"closed\": true"),
            format!("{GOOD}, \"fit\": {{ \"points\": [[0, 0]] }}"),
            format!("{GOOD}, \"fit\": {{ \"points\": [[0, 0], [2, 0]], \"knots\": \"odd\" }}"),
            r#""degree": 2, "control_points": [[0, 0], [1, 1], [2, 0]]"#.to_owned(),
        ] {
            assert!(read(&with_spline(&bad)).is_err(), "{bad}");
        }
        // Past the file limit is refused rather than read.
        let many = vec!["[0, 0]"; super::MAX_FILE_SPLINE_POINTS + 1].join(",");
        let huge = format!("\"degree\": 1, \"control_points\": [{many}], \"knots\": []");
        assert!(matches!(
            read(&with_spline(&huge)),
            Err(FileError::Invalid(_))
        ));
    }

    #[test]
    fn an_invalid_spline_is_not_written() {
        let mut document = Document::default();
        let points = vec![od_core::Point::new(0.0, 0.0), od_core::Point::new(1.0, 1.0)];
        let mut s = Spline::from_control_points(1, points).unwrap();
        s.knots.pop();
        document.add(Entity::Spline(Box::new(s)));
        assert!(NativeFormat.write(&document).is_err());
    }
}
