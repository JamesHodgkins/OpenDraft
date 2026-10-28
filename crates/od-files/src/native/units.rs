//! How a drawing's unit settings are written to the file. The numbers are
//! AutoCAD's own (`LUNITS`, `AUNITS`, `INSUNITS`), so the file reads like the
//! variables it comes from.

use od_core::{AngleFormat, InsertUnits, LengthFormat, Units};
use serde::{Deserialize, Serialize};

use super::entity::invalid;
use crate::FileError;

#[derive(Serialize, Deserialize)]
pub(super) struct UnitsDto {
    /// 1 scientific, 2 decimal, 3 engineering, 4 architectural, 5 fractional.
    length: i64,
    length_precision: u8,
    /// 0 decimal degrees, 1 degrees/minutes/seconds, 2 grads, 3 radians,
    /// 4 surveyor's units.
    angle: i64,
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
            length: units.length.code(),
            length_precision: units.length_precision,
            angle: units.angle.code(),
            angle_precision: units.angle_precision,
            clockwise: units.clockwise,
            base_angle: units.base_angle,
            insert: units.insert.code(),
        }
    }
}

impl UnitsDto {
    pub(super) fn into_units(self) -> Result<Units, FileError> {
        let Some(length) = LengthFormat::from_code(self.length) else {
            return invalid(format!("length units {} do not exist", self.length));
        };
        let Some(angle) = AngleFormat::from_code(self.angle) else {
            return invalid(format!("angle units {} do not exist", self.angle));
        };
        let Some(insert) = InsertUnits::from_code(self.insert) else {
            return invalid(format!("insertion units {} do not exist", self.insert));
        };
        Ok(Units {
            length,
            length_precision: self.length_precision,
            angle,
            angle_precision: self.angle_precision,
            clockwise: self.clockwise,
            base_angle: self.base_angle,
            insert,
        }
        .sanitized())
    }
}
