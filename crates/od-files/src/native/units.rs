//! How a drawing's unit settings are written to the file. The numbers are
//! AutoCAD's own (`LUNITS`, `AUNITS`, `INSUNITS`), so the file reads like the
//! variables it comes from.

use od_core::{InsertUnits, Units};
use serde::{Deserialize, Serialize};

use super::entity::invalid;
use super::names::{AngleFormatDto, LengthFormatDto};
use crate::FileError;

#[derive(Serialize, Deserialize)]
pub(super) struct UnitsDto {
    length: LengthFormatDto,
    length_precision: u8,
    angle: AngleFormatDto,
    angle_precision: u8,
    clockwise: bool,
    /// Radians counter-clockwise from east.
    base_angle: f64,
    /// What a drawing unit is, as AutoCAD's `INSUNITS` code (0 unitless).
    insert: i64,
}

impl From<&Units> for UnitsDto {
    fn from(units: &Units) -> Self {
        Self {
            length: units.length.into(),
            length_precision: units.length_precision,
            angle: units.angle.into(),
            angle_precision: units.angle_precision,
            clockwise: units.clockwise,
            base_angle: units.base_angle,
            insert: units.insert.code(),
        }
    }
}

impl UnitsDto {
    pub(super) fn into_units(self) -> Result<Units, FileError> {
        let Some(insert) = InsertUnits::from_code(self.insert) else {
            return invalid(format!("insertion units {} do not exist", self.insert));
        };
        Ok(Units {
            length: self.length.into(),
            length_precision: self.length_precision,
            angle: self.angle.into(),
            angle_precision: self.angle_precision,
            clockwise: self.clockwise,
            base_angle: self.base_angle,
            insert,
        }
        .sanitized())
    }
}
