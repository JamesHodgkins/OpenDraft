use crate::error::{DwgError, DwgErrorKind, DwgStage};

/// The file-format generations this reader handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DwgVersion {
    /// AC1024: AutoCAD 2010 to 2012.
    R2010,
    /// AC1027: AutoCAD 2013 to 2017.
    R2013,
    /// AC1032: AutoCAD 2018 and later.
    R2018,
}

impl DwgVersion {
    /// Reads the six-byte marker at the start of a file.
    pub fn probe(bytes: &[u8]) -> Result<DwgVersion, DwgError> {
        let marker: [u8; 6] = bytes
            .get(..6)
            .and_then(|m| m.try_into().ok())
            .ok_or_else(|| DwgError::new(DwgErrorKind::Truncated, DwgStage::Probe))?;
        match &marker {
            b"AC1024" => Ok(DwgVersion::R2010),
            b"AC1027" => Ok(DwgVersion::R2013),
            b"AC1032" => Ok(DwgVersion::R2018),
            [b'A', b'C', rest @ ..] if rest.iter().all(u8::is_ascii_digit) => Err(DwgError::new(
                DwgErrorKind::UnsupportedVersion(marker),
                DwgStage::Probe,
            )),
            _ => Err(DwgError::new(DwgErrorKind::InvalidHeader, DwgStage::Probe)),
        }
    }

    pub fn marker(self) -> &'static str {
        match self {
            DwgVersion::R2010 => "AC1024",
            DwgVersion::R2013 => "AC1027",
            DwgVersion::R2018 => "AC1032",
        }
    }

    /// For messages. AC1032 is a generation, not one release.
    pub fn description(self) -> &'static str {
        match self {
            DwgVersion::R2010 => "2010 to 2012 (AC1024)",
            DwgVersion::R2013 => "2013 to 2017 (AC1027)",
            DwgVersion::R2018 => "2018 or later (AC1032)",
        }
    }

    /// Objects carry the R2013 "has binary data" flag.
    pub fn has_r2013_object_binary_flag(self) -> bool {
        self >= DwgVersion::R2013
    }

    /// The auxiliary header ends with three extra zero shorts.
    pub fn has_r2018_aux_tail(self) -> bool {
        self >= DwgVersion::R2018
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_markers() {
        assert_eq!(DwgVersion::probe(b"AC1024xx"), Ok(DwgVersion::R2010));
        assert_eq!(DwgVersion::probe(b"AC1027"), Ok(DwgVersion::R2013));
        assert_eq!(DwgVersion::probe(b"AC1032\0\0"), Ok(DwgVersion::R2018));
    }

    #[test]
    fn older_versions_are_unsupported_not_invalid() {
        let err = DwgVersion::probe(b"AC1018").unwrap_err();
        assert_eq!(err.kind, DwgErrorKind::UnsupportedVersion(*b"AC1018"));
        assert!(err.user_message().contains("AC1018"));
    }

    #[test]
    fn other_files_are_invalid_and_short_ones_truncated() {
        assert_eq!(
            DwgVersion::probe(b"PK\x03\x04..").unwrap_err().kind,
            DwgErrorKind::InvalidHeader
        );
        assert_eq!(
            DwgVersion::probe(b"AC10x4").unwrap_err().kind,
            DwgErrorKind::InvalidHeader
        );
        assert_eq!(
            DwgVersion::probe(b"AC102").unwrap_err().kind,
            DwgErrorKind::Truncated
        );
    }

    #[test]
    fn feature_predicates() {
        assert!(!DwgVersion::R2010.has_r2013_object_binary_flag());
        assert!(DwgVersion::R2013.has_r2013_object_binary_flag());
        assert!(!DwgVersion::R2013.has_r2018_aux_tail());
        assert!(DwgVersion::R2018.has_r2018_aux_tail());
    }
}
