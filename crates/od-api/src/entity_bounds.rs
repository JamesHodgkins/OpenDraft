//! The box round an entity, on the wire.

use od_core::{Bounds, Point};
use serde::{Deserialize, Serialize};

use crate::entity::PointDto;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoundsDto {
    pub min: PointDto,
    pub max: PointDto,
}

impl From<Bounds> for BoundsDto {
    /// A box open to infinity (a construction line or ray) is given the
    /// largest numbers JSON can hold, as an infinity is not one.
    fn from(b: Bounds) -> Self {
        let limit =
            |p: Point| Point::new(p.x.clamp(f64::MIN, f64::MAX), p.y.clamp(f64::MIN, f64::MAX));
        Self {
            min: limit(b.min).into(),
            max: limit(b.max).into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_open_box_is_given_numbers_json_can_hold() {
        let open = Bounds {
            min: Point::new(f64::NEG_INFINITY, 2.0),
            max: Point::new(f64::INFINITY, 2.0),
        };
        let dto = BoundsDto::from(open);
        assert_eq!((dto.min.x, dto.max.x), (f64::MIN, f64::MAX));
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("e+308"), "{json}");
    }
}
