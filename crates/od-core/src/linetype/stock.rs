//! The stock linetype definitions, as AutoCAD ships them for a metric drawing
//! (`acadiso.lin`) plus the ISO 128 series. Autodesk's own numbers are data,
//! not something to compute: `CENTER2` is not a half-size `CENTER`. The
//! descriptions are sketches in our own words, not copied library text.
//!
//! Lengths are drawing units (millimetres here): positive draws, negative
//! leaves a gap, zero is a dot. See `private_material/autocad_linetype_reference_guide.md`.

/// One definition in the stock library.
#[derive(Clone, Copy, Debug)]
pub struct Stock {
    pub name: &'static str,
    pub description: &'static str,
    pub pattern: &'static [f64],
}

/// Every stock definition: the 8 families in three sizes, then the ISO series.
pub const LIBRARY: &[Stock] = &[
    Stock {
        name: "BORDER",
        description: "Border __ __ . __ __ . __ __ .",
        pattern: &[12.7, -6.35, 12.7, -6.35, 0.0, -6.35],
    },
    Stock {
        name: "BORDER2",
        description: "Border (.5x) _ _ . _ _ . _ _ .",
        pattern: &[6.35, -3.175, 6.35, -3.175, 0.0, -3.175],
    },
    Stock {
        name: "BORDERX2",
        description: "Border (2x) ____ ____ . ____ ____ .",
        pattern: &[25.4, -12.7, 25.4, -12.7, 0.0, -12.7],
    },
    Stock {
        name: "CENTER",
        description: "Center ____ _ ____ _ ____ _",
        pattern: &[31.75, -6.35, 6.35, -6.35],
    },
    Stock {
        name: "CENTER2",
        description: "Center (.5x) ___ _ ___ _ ___ _",
        pattern: &[19.05, -3.175, 3.175, -3.175],
    },
    Stock {
        name: "CENTERX2",
        description: "Center (2x) ________ __ ________ __",
        pattern: &[63.5, -12.7, 12.7, -12.7],
    },
    Stock {
        name: "DASHDOT",
        description: "Dash dot __ . __ . __ . __ .",
        pattern: &[12.7, -6.35, 0.0, -6.35],
    },
    Stock {
        name: "DASHDOT2",
        description: "Dash dot (.5x) _._._._._._._.",
        pattern: &[6.35, -3.175, 0.0, -3.175],
    },
    Stock {
        name: "DASHDOTX2",
        description: "Dash dot (2x) ____ . ____ . ____ .",
        pattern: &[25.4, -12.7, 0.0, -12.7],
    },
    Stock {
        name: "DASHED",
        description: "Dashed __ __ __ __ __ __ __ __",
        pattern: &[12.7, -6.35],
    },
    Stock {
        name: "DASHED2",
        description: "Dashed (.5x) _ _ _ _ _ _ _ _ _ _",
        pattern: &[6.35, -3.175],
    },
    Stock {
        name: "DASHEDX2",
        description: "Dashed (2x) ____ ____ ____ ____",
        pattern: &[25.4, -12.7],
    },
    Stock {
        name: "DIVIDE",
        description: "Divide ____ . . ____ . . ____ . .",
        pattern: &[12.7, -6.35, 0.0, -6.35, 0.0, -6.35],
    },
    Stock {
        name: "DIVIDE2",
        description: "Divide (.5x) __..__..__..__..",
        pattern: &[6.35, -3.175, 0.0, -3.175, 0.0, -3.175],
    },
    Stock {
        name: "DIVIDEX2",
        description: "Divide (2x) ________ . . ________ . .",
        pattern: &[25.4, -12.7, 0.0, -12.7, 0.0, -12.7],
    },
    Stock {
        name: "DOT",
        description: "Dot . . . . . . . . . . . . . .",
        pattern: &[0.0, -6.35],
    },
    Stock {
        name: "DOT2",
        description: "Dot (.5x) ....................",
        pattern: &[0.0, -3.175],
    },
    Stock {
        name: "DOTX2",
        description: "Dot (2x) .  .  .  .  .  .  .  .",
        pattern: &[0.0, -12.7],
    },
    Stock {
        name: "HIDDEN",
        description: "Hidden __ __ __ __ __ __ __ __",
        pattern: &[6.35, -3.175],
    },
    Stock {
        name: "HIDDEN2",
        description: "Hidden (.5x) _ _ _ _ _ _ _ _ _ _",
        pattern: &[3.175, -1.5875],
    },
    Stock {
        name: "HIDDENX2",
        description: "Hidden (2x) ____ ____ ____ ____",
        pattern: &[12.7, -6.35],
    },
    Stock {
        name: "PHANTOM",
        description: "Phantom ______ __ __ ______ __ __",
        pattern: &[31.75, -6.35, 6.35, -6.35, 6.35, -6.35],
    },
    Stock {
        name: "PHANTOM2",
        description: "Phantom (.5x) ___ _ _ ___ _ _",
        pattern: &[15.875, -3.175, 3.175, -3.175, 3.175, -3.175],
    },
    Stock {
        name: "PHANTOMX2",
        description: "Phantom (2x) ____________ ____ ____",
        pattern: &[63.5, -12.7, 12.7, -12.7, 12.7, -12.7],
    },
    Stock {
        name: "ACAD_ISO02W100",
        description: "ISO dash",
        pattern: &[12.0, -3.0],
    },
    Stock {
        name: "ACAD_ISO03W100",
        description: "ISO dash space",
        pattern: &[12.0, -18.0],
    },
    Stock {
        name: "ACAD_ISO04W100",
        description: "ISO long-dash dot",
        pattern: &[24.0, -3.0, 0.0, -3.0],
    },
    Stock {
        name: "ACAD_ISO05W100",
        description: "ISO long-dash double-dot",
        pattern: &[24.0, -3.0, 0.0, -3.0, 0.0, -3.0],
    },
    Stock {
        name: "ACAD_ISO06W100",
        description: "ISO long-dash triple-dot",
        pattern: &[24.0, -3.0, 0.0, -3.0, 0.0, -3.0, 0.0, -3.0],
    },
    Stock {
        name: "ACAD_ISO07W100",
        description: "ISO dot",
        pattern: &[0.0, -3.0],
    },
    Stock {
        name: "ACAD_ISO08W100",
        description: "ISO long-dash short-dash",
        pattern: &[24.0, -3.0, 6.0, -3.0],
    },
    Stock {
        name: "ACAD_ISO09W100",
        description: "ISO long-dash double-short-dash",
        pattern: &[24.0, -3.0, 6.0, -3.0, 6.0, -3.0],
    },
    Stock {
        name: "ACAD_ISO10W100",
        description: "ISO dash dot",
        pattern: &[12.0, -3.0, 0.0, -3.0],
    },
    Stock {
        name: "ACAD_ISO11W100",
        description: "ISO double-dash dot",
        pattern: &[12.0, -3.0, 12.0, -3.0, 0.0, -3.0],
    },
    Stock {
        name: "ACAD_ISO12W100",
        description: "ISO dash double-dot",
        pattern: &[12.0, -3.0, 0.0, -3.0, 0.0, -3.0],
    },
    Stock {
        name: "ACAD_ISO13W100",
        description: "ISO double-dash double-dot",
        pattern: &[12.0, -3.0, 12.0, -3.0, 0.0, -3.0, 0.0, -3.0],
    },
    Stock {
        name: "ACAD_ISO14W100",
        description: "ISO dash triple-dot",
        pattern: &[12.0, -3.0, 0.0, -3.0, 0.0, -3.0, 0.0, -3.0],
    },
    Stock {
        name: "ACAD_ISO15W100",
        description: "ISO double-dash triple-dot",
        pattern: &[12.0, -3.0, 12.0, -3.0, 0.0, -3.0, 0.0, -3.0, 0.0, -3.0],
    },
];

/// The stock definition called `name`, ignoring ASCII case.
pub fn find(name: &str) -> Option<&'static Stock> {
    LIBRARY.iter().find(|s| s.name.eq_ignore_ascii_case(name))
}
