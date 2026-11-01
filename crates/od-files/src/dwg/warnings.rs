//! The words for what an import left out or changed. The reader's report is
//! typed counts; this turns it into the short sentences the user sees, in a
//! stable order, one per kind and never one per object.

use od_dwg::{Degradation, ImportReport, Skipped};

use super::convert::Notes;
use crate::{block_import, plural};

/// One sentence for a kind of skipped object, or `None` for the unsupported
/// types, which are summed into a single sentence by the caller.
fn skipped(why: &Skipped, count: u64) -> Option<String> {
    Some(match why {
        // Several kinds are summed into one sentence by the caller.
        Skipped::Unsupported(_) => return None,
        Skipped::NonPlanar => format!(
            "Skipped {} lying in a tilted plane or at more than one height.",
            plural(count, "object")
        ),
        Skipped::Invisible => format!("Skipped {} marked invisible.", plural(count, "object")),
        Skipped::PaperSpace => format!(
            "Skipped {} in paper space (layouts are not imported yet).",
            plural(count, "object")
        ),
        Skipped::AnonymousBlockReference => block_import::anonymous_references(count),
        Skipped::ExternalReference => block_import::external_references(count),
        Skipped::InvalidGeometry => {
            format!("Skipped {} with damaged geometry.", plural(count, "object"))
        }
        Skipped::DegeneratePolyline => {
            format!(
                "Skipped {} with fewer than two points.",
                plural(count, "polyline")
            )
        }
    })
}

fn degraded(how: Degradation, count: u64) -> String {
    match how {
        Degradation::ElevationFlattened => format!(
            "Flattened {} that sat above or below the drawing plane.",
            plural(count, "object")
        ),
        Degradation::MissingLinetype => format!(
            "{} named a linetype that is not in the drawing; they follow their layer instead.",
            plural(count, "object")
        ),
        Degradation::SplineFrameVertices => format!(
            "{} lost the control points of a fitted curve; the fitted curve itself was kept.",
            plural(count, "polyline")
        ),
        Degradation::TextFitApproximated => format!(
            "{} set \"aligned\" or \"fit\" between two points was read as ordinary text along that line.",
            plural(count, "text")
        ),
    }
}

/// The sentences for a report: what was skipped, then what was changed.
pub(super) fn words(report: &ImportReport) -> Vec<String> {
    let mut out = Vec::new();
    let unsupported: Vec<(&String, u64)> = report
        .skipped
        .iter()
        .filter_map(|(why, &n)| match why {
            Skipped::Unsupported(name) => Some((name, n)),
            _ => None,
        })
        .collect();
    if !unsupported.is_empty() {
        let total: u64 = unsupported.iter().map(|(_, n)| n).sum();
        let kinds: Vec<String> = unsupported
            .iter()
            .map(|(name, n)| format!("{n} {name}"))
            .collect();
        out.push(format!(
            "Skipped {} that this version cannot import yet: {}.",
            plural(total, "object"),
            kinds.join(", ")
        ));
    }
    out.extend(
        report
            .skipped
            .iter()
            .filter_map(|(why, &n)| skipped(why, n)),
    );
    out.extend(report.degraded.iter().map(|(&how, &n)| degraded(how, n)));
    out
}

/// The sentences for what the conversion itself changed.
pub(super) fn conversion_words(notes: &Notes) -> Vec<String> {
    let mut out = Vec::new();
    if notes.renamed_layers > 0 {
        out.push(format!(
            "Renamed {} whose names repeated another layer's name (ignoring case).",
            plural(notes.renamed_layers as u64, "layer")
        ));
    }
    out.extend(notes.linetypes.warnings());
    out.extend(notes.blocks.iter().cloned());
    if notes.bad_ellipses > 0 {
        out.push(format!(
            "Skipped {} whose values make no ellipse.",
            plural(notes.bad_ellipses as u64, "ellipse")
        ));
    }
    if notes.bad_splines > 0 {
        out.push(format!(
            "Skipped {} whose values make no spline.",
            plural(notes.bad_splines as u64, "spline")
        ));
    }
    if notes.bad_text > 0 {
        out.push(format!(
            "Skipped {} whose values make no text.",
            plural(notes.bad_text as u64, "text object")
        ));
    }
    if notes.fit_splines > 0 {
        out.push(format!(
            "Skipped {} stored by fit points with end tangents or not cubic (only cubic fits without end tangents are rebuilt).",
            plural(notes.fit_splines as u64, "spline")
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use od_dwg::DwgVersion;

    fn report() -> ImportReport {
        let mut r = ImportReport::new(DwgVersion::R2013);
        r.skipped.insert(Skipped::Unsupported("TEXT".into()), 3);
        r.skipped.insert(Skipped::Unsupported("INSERT".into()), 1);
        r.skipped.insert(Skipped::PaperSpace, 2);
        r.skipped.insert(Skipped::NonPlanar, 1);
        r.degraded.insert(Degradation::ElevationFlattened, 4);
        r
    }

    #[test]
    fn one_sentence_per_kind_in_a_fixed_order() {
        let words = words(&report());
        assert_eq!(
            words,
            [
                "Skipped 4 objects that this version cannot import yet: 1 INSERT, 3 TEXT.",
                "Skipped 1 object lying in a tilted plane or at more than one height.",
                "Skipped 2 objects in paper space (layouts are not imported yet).",
                "Flattened 4 objects that sat above or below the drawing plane.",
            ]
        );
    }

    #[test]
    fn conversion_notes() {
        let notes = Notes {
            renamed_layers: 1,
            blocks: Vec::new(),
            bad_ellipses: 2,
            bad_splines: 1,
            fit_splines: 3,
            ..Notes::default()
        };
        assert_eq!(
            conversion_words(&notes),
            [
                "Renamed 1 layer whose names repeated another layer's name (ignoring case).",
                "Skipped 2 ellipses whose values make no ellipse.",
                "Skipped 1 spline whose values make no spline.",
                "Skipped 3 splines stored by fit points with end tangents or not cubic (only cubic fits without end tangents are rebuilt).",
            ]
        );
        assert!(conversion_words(&Notes::default()).is_empty());
    }

    #[test]
    fn nothing_to_say_when_nothing_was_lost() {
        assert!(words(&ImportReport::new(DwgVersion::R2018)).is_empty());
    }
}
