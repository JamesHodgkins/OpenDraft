//! Lineweights as DWG stores them.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DwgLineweight {
    ByLayer,
    Default,
    /// In hundredths of a millimetre.
    Hundredths(u16),
}

/// The lineweight codes that are not table entries: ByLayer, ByBlock and the
/// default. (Read off the supplied drawings, whose model-space entities are
/// ByLayer with code 29.)
pub(crate) const BY_LAYER: u8 = 29;
pub(crate) const BY_BLOCK: u8 = 30;

/// The standard lineweights in hundredths of a millimetre, by DWG code.
const LINEWEIGHTS: [u16; 24] = [
    0, 5, 9, 13, 15, 18, 20, 25, 30, 35, 40, 50, 53, 60, 70, 80, 90, 100, 106, 120, 140, 158, 200,
    211,
];

pub(crate) fn lineweight(code: u8) -> DwgLineweight {
    match code {
        BY_LAYER => DwgLineweight::ByLayer,
        c => LINEWEIGHTS
            .get(usize::from(c))
            .map_or(DwgLineweight::Default, |&w| DwgLineweight::Hundredths(w)),
    }
}
