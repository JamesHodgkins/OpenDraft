//! The native format's description of a hatch's fill: a pattern with its
//! final lines, a solid fill, or a gradient.

use od_core::hatch::{HatchFill, HatchGradient, HatchPattern, PatternLine, PatternType};
use serde::{Deserialize, Serialize};

use super::entity::{from_point, invalid, point, PointDto};
use crate::FileError;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum FillDto {
    Pattern {
        /// `user_defined`, `predefined` or `custom`.
        kind: String,
        name: String,
        angle: f64,
        scale: f64,
        #[serde(default)]
        double: bool,
        origin: PointDto,
        lines: Vec<LineDto>,
    },
    Solid,
    Gradient {
        name: String,
        angle: f64,
        shift: f64,
        one_color: bool,
        tint: f64,
        colors: Vec<(f64, [u8; 3])>,
    },
}

#[derive(Serialize, Deserialize)]
pub(super) struct LineDto {
    angle: f64,
    base: PointDto,
    offset: [f64; 2],
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    dashes: Vec<f64>,
}

fn kind_name(k: PatternType) -> &'static str {
    match k {
        PatternType::UserDefined => "user_defined",
        PatternType::Predefined => "predefined",
        PatternType::Custom => "custom",
    }
}

impl FillDto {
    pub(super) fn from_fill(f: &HatchFill) -> Self {
        match f {
            HatchFill::Solid => FillDto::Solid,
            HatchFill::Pattern(p) => FillDto::Pattern {
                kind: kind_name(p.pattern_type).to_owned(),
                name: p.name.clone(),
                angle: p.angle,
                scale: p.scale_or_spacing,
                double: p.double,
                origin: point(p.origin),
                lines: p
                    .lines
                    .iter()
                    .map(|l| LineDto {
                        angle: l.angle,
                        base: point(l.base),
                        offset: l.offset,
                        dashes: l.dashes.clone(),
                    })
                    .collect(),
            },
            HatchFill::Gradient(g) => FillDto::Gradient {
                name: g.name.clone(),
                angle: g.angle,
                shift: g.shift,
                one_color: g.one_color,
                tint: g.tint,
                colors: g.colors.clone(),
            },
        }
    }

    pub(super) fn to_fill(&self) -> Result<HatchFill, FileError> {
        Ok(match self {
            FillDto::Solid => HatchFill::Solid,
            FillDto::Pattern {
                kind,
                name,
                angle,
                scale,
                double,
                origin,
                lines,
            } => {
                let pattern_type = match kind.as_str() {
                    "user_defined" => PatternType::UserDefined,
                    "predefined" => PatternType::Predefined,
                    "custom" => PatternType::Custom,
                    other => {
                        return invalid(format!("a hatch has unknown pattern kind \"{other}\""))
                    }
                };
                HatchFill::Pattern(HatchPattern {
                    pattern_type,
                    name: name.clone(),
                    angle: *angle,
                    scale_or_spacing: *scale,
                    double: *double,
                    lines: lines
                        .iter()
                        .map(|l| {
                            Ok(PatternLine {
                                angle: l.angle,
                                base: from_point(l.base)?,
                                offset: l.offset,
                                dashes: l.dashes.clone(),
                            })
                        })
                        .collect::<Result<_, FileError>>()?,
                    origin: from_point(*origin)?,
                })
            }
            FillDto::Gradient {
                name,
                angle,
                shift,
                one_color,
                tint,
                colors,
            } => HatchFill::Gradient(HatchGradient {
                name: name.clone(),
                angle: *angle,
                shift: *shift,
                one_color: *one_color,
                tint: *tint,
                colors: colors.clone(),
            }),
        })
    }
}
