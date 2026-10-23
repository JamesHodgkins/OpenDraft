use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// An unsigned 64-bit id (entity, layer, revision...) that crosses the wire
/// as a decimal string, because JSON numbers lose precision above 2^53.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WireId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdError(pub String);

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "not an unsigned decimal id: {:?}", self.0)
    }
}

impl std::error::Error for IdError {}

impl fmt::Display for WireId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for WireId {
    type Err = IdError;

    /// Digits only: no sign, no whitespace, no leading zeros, so one id has
    /// exactly one spelling.
    fn from_str(s: &str) -> Result<Self, IdError> {
        let canonical = !s.is_empty()
            && s.bytes().all(|b| b.is_ascii_digit())
            && (s == "0" || !s.starts_with('0'));
        if !canonical {
            return Err(IdError(s.to_owned()));
        }
        s.parse().map(WireId).map_err(|_| IdError(s.to_owned()))
    }
}

impl Serialize for WireId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for WireId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_above_two_to_the_53_round_trip_exactly() {
        for value in [0, 1, (1 << 53) + 1, u64::MAX] {
            let json = serde_json::to_string(&WireId(value)).unwrap();
            assert_eq!(json, format!("\"{value}\""));
            assert_eq!(
                serde_json::from_str::<WireId>(&json).unwrap(),
                WireId(value)
            );
        }
    }

    #[test]
    fn only_the_canonical_spelling_is_accepted() {
        for bad in [
            "",
            "-1",
            "+1",
            " 1",
            "1 ",
            "01",
            "1.0",
            "1e3",
            "18446744073709551616",
            "x",
        ] {
            assert!(bad.parse::<WireId>().is_err(), "{bad:?}");
        }
        assert!(
            serde_json::from_str::<WireId>("12").is_err(),
            "a bare number"
        );
    }
}
