//! TEXT, MTEXT and the text style records they name (Open Design
//! Specification §20.4.3, §20.4.46 and §20.4.56), and the flat shapes they
//! make.
//!
//! A TEXT keeps what AutoCAD stores: an insertion point, an optional
//! alignment point, and the two alignment codes. Its shape is where the text
//! hangs and the codes that say how, with the "aligned" and "fit" kinds (which
//! stretch the words between two points) read as left-aligned text along the
//! line between them. An MTEXT keeps its contents as written, format codes
//! included.

use crate::bit::BitReader;
use crate::drawing::{table_color, DwgColor};
use crate::entities::Point3;
use crate::error::DwgError;
use crate::geometry::Orientation;
use crate::handles::DwgHandle;
use crate::limits::DwgLimits;
use crate::object::ObjectStreams;
use crate::report::{Degradation, ImportReport, Skipped};
use crate::shapes::{all_sane, flatten, Shape};
use crate::version::DwgVersion;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TextData {
    pub elevation: f64,
    pub insertion: [f64; 2],
    /// Present when the text is aligned other than left on the baseline.
    pub alignment: Option<[f64; 2]>,
    pub extrusion: Point3,
    /// Radians.
    pub oblique: f64,
    pub rotation: f64,
    pub height: f64,
    pub width_factor: f64,
    pub value: String,
    pub generation: u16,
    pub horizontal: u16,
    pub vertical: u16,
    pub style: Option<DwgHandle>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MTextData {
    pub insertion: Point3,
    pub extrusion: Point3,
    pub direction: Point3,
    pub width: f64,
    pub height: f64,
    /// 1 to 9: top left, top centre, ... bottom right.
    pub attachment: u16,
    /// 1 left to right, 3 top to bottom, 5 by style.
    pub flow: u16,
    pub text: String,
    pub spacing_style: u16,
    pub spacing: f64,
    /// The mask's scale and colour (none for the window's own colour).
    pub background: Option<(f64, Option<DwgColor>)>,
    pub columns: bool,
    pub style: Option<DwgHandle>,
}

/// A text style record: what the font and the shape of the letters are.
#[derive(Clone, Debug, PartialEq)]
pub struct DwgTextStyle {
    pub handle: DwgHandle,
    pub name: String,
    /// The font file (`txt.shx`, `arial.ttf`); empty when the record has none.
    pub font: String,
    pub big_font: String,
    pub vertical: bool,
    /// A shape file rather than a font.
    pub shape_file: bool,
    /// Zero when the style leaves the height to each text.
    pub fixed_height: f64,
    pub width_factor: f64,
    /// Radians.
    pub oblique: f64,
    /// Bit 2 backward, bit 4 upside down.
    pub generation: u8,
}

/// STYLE: after the name and the xref index, the vertical and shape-file
/// bits, then the values and the font names (the strings).
pub(crate) fn read_style(
    s: &mut ObjectStreams<'_>,
    handle: DwgHandle,
    limits: &DwgLimits,
) -> Result<DwgTextStyle, DwgError> {
    let max = limits.max_string_code_units;
    let name = s.strings.text(max)?;
    let d = &mut s.data;
    d.bs()?; // xref index + 1
    let vertical = d.bit()?;
    let shape_file = d.bit()?;
    let fixed_height = d.bd()?;
    let width_factor = d.bd()?;
    let oblique = d.bd()?;
    let generation = d.rc()?;
    d.bd()?; // last height used
    let font = s.strings.text(max)?;
    let big_font = s.strings.text(max)?;
    Ok(DwgTextStyle {
        handle,
        name,
        font,
        big_font,
        vertical,
        shape_file,
        fixed_height,
        width_factor,
        oblique,
        generation,
    })
}

fn style_handle(
    s: &mut ObjectStreams<'_>,
    base: DwgHandle,
    limits: &DwgLimits,
) -> Result<Option<DwgHandle>, DwgError> {
    let h = &mut s.handles;
    h.handle(limits.max_handle_bytes)?
        .resolve(base)
        .map_err(|e| e.at(h.position()))
}

/// TEXT (R2000 and later): a flag byte says which of the values are the
/// defaults and so not stored.
pub(crate) fn read_text(
    s: &mut ObjectStreams<'_>,
    base: DwgHandle,
    limits: &DwgLimits,
) -> Result<TextData, DwgError> {
    let d = &mut s.data;
    let flags = d.rc()?;
    let elevation = if flags & 0x01 == 0 { d.rd()? } else { 0.0 };
    let insertion = d.rd2()?;
    let alignment = if flags & 0x02 == 0 {
        Some([d.dd(insertion[0])?, d.dd(insertion[1])?])
    } else {
        None
    };
    let extrusion = d.extrusion()?;
    d.thickness()?;
    let oblique = if flags & 0x04 == 0 { d.rd()? } else { 0.0 };
    let rotation = if flags & 0x08 == 0 { d.rd()? } else { 0.0 };
    let height = d.rd()?;
    let width_factor = if flags & 0x10 == 0 { d.rd()? } else { 1.0 };
    let value = s.strings.text(limits.max_string_code_units)?;
    let d = &mut s.data;
    let generation = if flags & 0x20 == 0 { d.bs()? } else { 0 };
    let horizontal = if flags & 0x40 == 0 { d.bs()? } else { 0 };
    let vertical = if flags & 0x80 == 0 { d.bs()? } else { 0 };
    let style = style_handle(s, base, limits)?;
    Ok(TextData {
        elevation,
        insertion,
        alignment,
        extrusion,
        oblique,
        rotation,
        height,
        width_factor,
        value,
        generation,
        horizontal,
        vertical,
        style,
    })
}

/// An ATTRIB or ATTDEF: a text with a tag and flags. Only the single-line kind
/// is read; the multiline kind carries an embedded MTEXT and is noted as such.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AttribData {
    pub text: TextData,
    pub multiline: bool,
    pub invisible: bool,
    pub constant: bool,
}

/// ATTRIB and ATTDEF (§20.4.4 and §20.4.5): the TEXT fields, a version byte,
/// from 2018 the kind, then (single line) the tag, field length, flags and
/// the position lock. An ATTDEF's prompt follows and is not needed.
pub(crate) fn read_attrib(
    s: &mut ObjectStreams<'_>,
    base: DwgHandle,
    version: DwgVersion,
    limits: &DwgLimits,
) -> Result<AttribData, DwgError> {
    let text = read_text(s, base, limits)?;
    s.data.rc()?; // version
    let multiline = version >= DwgVersion::R2018 && s.data.rc()? != 1;
    if multiline {
        return Ok(AttribData {
            text,
            multiline,
            invisible: false,
            constant: false,
        });
    }
    s.strings.text(limits.max_string_code_units)?; // tag
    s.data.bs()?; // field length
    let flags = s.data.rc()?;
    Ok(AttribData {
        text,
        multiline,
        invisible: flags & 1 != 0,
        constant: flags & 2 != 0,
    })
}

/// The background of an MTEXT (R2004 and later): flags, then for a fill the
/// scale, the colour and a transparency.
fn read_background(
    s: &mut ObjectStreams<'_>,
    limits: &DwgLimits,
) -> Result<Option<(f64, Option<DwgColor>)>, DwgError> {
    let flags = s.data.bl()?;
    // The scale, colour and transparency follow only for a fill in its own
    // colour (1) or a text frame (0x10); a mask in the window's colour (2)
    // stores nothing more.
    let (scale, own) = if flags & 0x11 != 0 {
        let scale = s.data.bd()?;
        let color = crate::common::read_table_color(&mut s.data, &mut s.strings, limits)?;
        s.data.bl()?; // transparency
        (scale, (flags & 3 == 1).then(|| table_color(&color)))
    } else {
        (1.5, None)
    };
    Ok((flags & 3 != 0).then_some((scale, own)))
}

/// The end of an MTEXT that only the 2018 layout has: whether it is set in
/// more than one column. Anything that does not read leaves the text as one column.
fn has_columns(d: &mut BitReader<'_>) -> Option<bool> {
    if d.bit().ok()? {
        d.bs().ok()?; // version
        d.bit().ok()?; // default flag
        d.bl().ok()?; // attachment again
        for _ in 0..2 {
            d.bd3().ok()?; // direction and insertion again
        }
        for _ in 0..4 {
            d.bd().ok()?; // the box and the extents again
        }
        // Dynamic columns of count 1 are what AutoCAD gives any MTEXT with a
        // width: one column, which is no columns.
        if d.bs().ok()? == 0 {
            return Some(false);
        }
        return Some(d.bl().ok()? > 1);
    }
    None
}

/// MTEXT. The fields after the text are read for the background and, in a
/// 2018 file, the column type; a failure there is not a failure of the text.
pub(crate) fn read_mtext(
    s: &mut ObjectStreams<'_>,
    base: DwgHandle,
    limits: &DwgLimits,
) -> Result<MTextData, DwgError> {
    let d = &mut s.data;
    let insertion = d.bd3()?;
    let extrusion = d.bd3()?;
    let direction = d.bd3()?;
    let width = d.bd()?;
    d.bd()?; // box height
    let height = d.bd()?;
    let attachment = d.bs()?;
    let flow = d.bs()?;
    d.bd()?; // extents height
    d.bd()?; // extents width
    let text = s.strings.text(limits.max_string_code_units)?;
    let style = style_handle(s, base, limits)?;
    let spacing_style = s.data.bs()?;
    let spacing = s.data.bd()?;
    s.data.bit()?; // unknown
    let background = read_background(s, limits)?;
    let columns = has_columns(&mut s.data).unwrap_or(false);
    Ok(MTextData {
        insertion,
        extrusion,
        direction,
        width,
        height,
        attachment,
        flow,
        text,
        spacing_style,
        spacing,
        background,
        columns,
        style,
    })
}

/// The shape of a TEXT. A text hung from its alignment point, or "aligned" or
/// "fit" between two points, is placed by `horizontal` and `vertical` as
/// AutoCAD's codes give them (0 left, 1 centre, 2 right; 0 baseline, 1 bottom,
/// 2 middle, 3 top); "middle" (4) is centre and middle.
pub(crate) fn text(d: &TextData, report: &mut ImportReport) -> Result<Shape, Skipped> {
    let o = Orientation::from_extrusion(d.extrusion).map_err(|_| Skipped::InvalidGeometry)?;
    let other = d.alignment.unwrap_or(d.insertion);
    let numbers = [
        d.insertion[0],
        d.insertion[1],
        other[0],
        other[1],
        d.elevation,
        d.rotation,
        d.oblique,
        d.width_factor,
    ];
    if o != Orientation::Upright || !all_sane(&numbers) || !all_sane(&[d.height]) || d.height <= 0.0
    {
        return Err(Skipped::InvalidGeometry);
    }
    if d.width_factor <= 0.0 || d.value.trim().is_empty() {
        return Err(Skipped::InvalidGeometry);
    }
    flatten(
        d.elevation,
        d.insertion[0].abs().max(d.insertion[1].abs()),
        report,
    );
    let (at, rotation, horizontal, vertical) = match (d.horizontal, d.vertical) {
        (3 | 5, _) => {
            // Between the two points: left-aligned along the line they make.
            report.degrade(Degradation::TextFitApproximated);
            let (dx, dy) = (other[0] - d.insertion[0], other[1] - d.insertion[1]);
            let turn = if dx.hypot(dy) > 1e-12 {
                dy.atan2(dx)
            } else {
                d.rotation
            };
            (d.insertion, turn, 0, 0)
        }
        (4, _) => (other, d.rotation, 1, 2),
        (0, 0) => (d.insertion, d.rotation, 0, 0),
        (h, v) => (other, d.rotation, h.min(2), v.min(3)),
    };
    Ok(Shape::Text {
        at,
        rotation,
        height: d.height,
        width_factor: d.width_factor,
        oblique: d.oblique,
        value: d.value.clone(),
        horizontal: u8::try_from(horizontal).unwrap_or(0),
        vertical: u8::try_from(vertical).unwrap_or(0),
        backward: d.generation & 2 != 0,
        upside_down: d.generation & 4 != 0,
        style: d.style,
    })
}

/// The shape of an ATTRIB, which is its text, or of a constant ATTDEF, which
/// shows its default value. Any other ATTDEF is a template: what a reference
/// shows is its own ATTRIB.
pub(crate) fn attrib(
    d: &AttribData,
    definition: bool,
    report: &mut ImportReport,
) -> Result<Shape, Skipped> {
    if d.multiline {
        return Err(Skipped::Unsupported("multiline attribute".into()));
    }
    if definition && !d.constant {
        return Err(Skipped::AttributeTemplate);
    }
    if d.invisible {
        return Err(Skipped::Invisible);
    }
    text(&d.text, report)
}

pub(crate) fn mtext(d: &MTextData, report: &mut ImportReport) -> Result<Shape, Skipped> {
    let o = Orientation::from_extrusion(d.extrusion).map_err(|_| Skipped::InvalidGeometry)?;
    let numbers = [
        d.insertion[0],
        d.insertion[1],
        d.insertion[2],
        d.direction[0],
        d.direction[1],
        d.width,
        d.spacing,
    ];
    if !all_sane(&numbers) || !all_sane(&[d.height]) || d.height <= 0.0 || d.width < 0.0 {
        return Err(Skipped::InvalidGeometry);
    }
    if o != Orientation::Upright {
        return Err(Skipped::NonPlanar);
    }
    if d.columns {
        return Err(Skipped::Unsupported("MTEXT in columns".into()));
    }
    if d.text.trim().is_empty() {
        return Err(Skipped::InvalidGeometry);
    }
    flatten(
        d.insertion[2],
        d.insertion[0].abs().max(d.insertion[1].abs()),
        report,
    );
    let mut direction = [d.direction[0], d.direction[1]];
    if direction[0].hypot(direction[1]) <= 1e-12 {
        direction = [1.0, 0.0];
    }
    Ok(Shape::MText {
        at: [d.insertion[0], d.insertion[1]],
        direction,
        height: d.height,
        width: d.width,
        attachment: u8::try_from(d.attachment).unwrap_or(1),
        flow: u8::try_from(d.flow).unwrap_or(1),
        contents: d.text.clone(),
        spacing_style: u8::try_from(d.spacing_style).unwrap_or(1),
        spacing: d.spacing,
        background: d.background,
        style: d.style,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{BitWriter, Buffers};
    use crate::version::DwgVersion;

    fn report() -> ImportReport {
        ImportReport::new(DwgVersion::R2018)
    }

    fn handles() -> BitWriter {
        let mut h = BitWriter::new();
        h.handle(5, 0x11);
        h
    }

    #[test]
    fn a_text_with_every_default_left_out() {
        let mut data = BitWriter::new();
        // Everything defaulted: no elevation, alignment, oblique, rotation,
        // width factor, generation, or alignment codes.
        data.rc(0xFF)
            .rd2([5.0, 6.0])
            .extrusion([0.0, 0.0, 1.0])
            .thickness(0.0)
            .rd(2.5);
        let mut strings = BitWriter::new();
        strings.text("Hello");
        let b = Buffers::new(&data, &strings, &handles());
        let t = read_text(&mut b.streams(), DwgHandle(0x40), &DwgLimits::default()).unwrap();
        assert_eq!(
            (t.insertion, t.height, t.value.as_str()),
            ([5.0, 6.0], 2.5, "Hello")
        );
        assert_eq!((t.width_factor, t.rotation, t.horizontal), (1.0, 0.0, 0));
        assert_eq!(t.style, Some(DwgHandle(0x11)));
        let Ok(Shape::Text { at, vertical, .. }) = text(&t, &mut report()) else {
            panic!("a text")
        };
        assert_eq!((at, vertical), ([5.0, 6.0], 0));
    }

    #[test]
    fn a_centred_text_hangs_from_its_alignment_point() {
        let mut data = BitWriter::new();
        // Alignment point, rotation, width factor, horizontal and vertical codes.
        data.rc(0x01 | 0x04 | 0x20)
            .rd2([0.0, 0.0])
            .dd_full(10.0)
            .dd_full(4.0)
            .extrusion([0.0, 0.0, 1.0])
            .thickness(0.0)
            .rd(0.5)
            .rd(3.0)
            .rd(0.8);
        data.bs(1).bs(2);
        let mut strings = BitWriter::new();
        strings.text("Mid");
        let b = Buffers::new(&data, &strings, &handles());
        let t = read_text(&mut b.streams(), DwgHandle(0x40), &DwgLimits::default()).unwrap();
        assert_eq!((t.horizontal, t.vertical), (1, 2));
        let Ok(Shape::Text {
            at,
            rotation,
            width_factor,
            ..
        }) = text(&t, &mut report())
        else {
            panic!("a text")
        };
        assert_eq!((at, rotation, width_factor), ([10.0, 4.0], 0.5, 0.8));
    }

    #[test]
    fn aligned_text_follows_the_line_between_its_points() {
        let t = TextData {
            elevation: 0.0,
            insertion: [0.0, 0.0],
            alignment: Some([10.0, 10.0]),
            extrusion: [0.0, 0.0, 1.0],
            oblique: 0.0,
            rotation: 0.0,
            height: 1.0,
            width_factor: 1.0,
            value: "x".into(),
            generation: 0,
            horizontal: 3,
            vertical: 0,
            style: None,
        };
        let mut r = report();
        let Ok(Shape::Text { rotation, at, .. }) = text(&t, &mut r) else {
            panic!("a text")
        };
        assert!((rotation - std::f64::consts::FRAC_PI_4).abs() < 1e-12);
        assert_eq!(at, [0.0, 0.0]);
        assert_eq!(r.degraded.get(&Degradation::TextFitApproximated), Some(&1));
    }

    #[test]
    fn a_text_that_cannot_exist_is_refused() {
        let mut t = TextData {
            elevation: 0.0,
            insertion: [0.0, 0.0],
            alignment: None,
            extrusion: [0.0, 0.0, 1.0],
            oblique: 0.0,
            rotation: 0.0,
            height: 0.0,
            width_factor: 1.0,
            value: "x".into(),
            generation: 0,
            horizontal: 0,
            vertical: 0,
            style: None,
        };
        assert_eq!(text(&t, &mut report()), Err(Skipped::InvalidGeometry));
        t.height = 1.0;
        t.value = "  ".into();
        assert_eq!(text(&t, &mut report()), Err(Skipped::InvalidGeometry));
    }

    fn mtext_streams(extra: impl Fn(&mut BitWriter)) -> Buffers {
        let mut data = BitWriter::new();
        data.bd3([1.0, 2.0, 0.0])
            .bd3([0.0, 0.0, 1.0])
            .bd3([0.0, 1.0, 0.0])
            .bd(40.0)
            .bd(0.0)
            .bd(2.5)
            .bs(5)
            .bs(1)
            .bd(2.5)
            .bd(30.0);
        data.bs(1).bd(1.0).bit(false);
        extra(&mut data);
        let mut strings = BitWriter::new();
        strings.text("A\\PB");
        Buffers::new(&data, &strings, &handles())
    }

    #[test]
    fn an_mtext_with_no_background_and_a_turned_axis() {
        let b = mtext_streams(|d| {
            d.bl(0);
        });
        let m = read_mtext(&mut b.streams(), DwgHandle(0x40), &DwgLimits::default()).unwrap();
        assert_eq!((m.text.as_str(), m.attachment, m.width), ("A\\PB", 5, 40.0));
        assert_eq!(m.background, None);
        assert!(!m.columns);
        let Ok(Shape::MText {
            direction, height, ..
        }) = mtext(&m, &mut report())
        else {
            panic!("an mtext")
        };
        assert_eq!((direction, height), ([0.0, 1.0], 2.5));
    }

    #[test]
    fn a_mask_in_the_window_colour_stores_nothing_more() {
        // Real drawings write flag 2 alone; reading a scale and a colour
        // after it ran past the end of the object.
        let b = mtext_streams(|d| {
            d.bl(2);
        });
        let m = read_mtext(&mut b.streams(), DwgHandle(0x40), &DwgLimits::default()).unwrap();
        assert_eq!(m.background, Some((1.5, None)));
    }

    #[test]
    fn an_mtext_background_fill_in_its_own_colour() {
        let b = mtext_streams(|d| {
            d.bl(1).bd(2.0).bs(1).bl(0).rc(0).bl(0);
        });
        let m = read_mtext(&mut b.streams(), DwgHandle(0x40), &DwgLimits::default()).unwrap();
        assert_eq!(m.background, Some((2.0, Some(DwgColor::Aci(1)))));
    }

    fn attrib_streams(kind: u8, flags: u8) -> Buffers {
        let mut data = BitWriter::new();
        data.rc(0xFF)
            .rd2([5.0, 6.0])
            .extrusion([0.0, 0.0, 1.0])
            .thickness(0.0)
            .rd(2.5);
        data.rc(0).rc(kind).bs(0).rc(flags);
        let mut strings = BitWriter::new();
        strings.text("Value").text("TAG");
        Buffers::new(&data, &strings, &handles())
    }

    #[test]
    fn an_attribute_is_a_text_with_a_tag_and_flags() {
        let b = attrib_streams(1, 2);
        let a = read_attrib(
            &mut b.streams(),
            DwgHandle(0x40),
            DwgVersion::R2018,
            &DwgLimits::default(),
        )
        .unwrap();
        assert_eq!(a.text.value, "Value");
        assert!(a.constant && !a.invisible && !a.multiline);
        // A reference's attribute shows; a constant definition shows too.
        assert!(matches!(
            attrib(&a, false, &mut report()),
            Ok(Shape::Text { .. })
        ));
        assert!(matches!(
            attrib(&a, true, &mut report()),
            Ok(Shape::Text { .. })
        ));
    }

    #[test]
    fn templates_and_invisible_attributes_do_not_show() {
        let b = attrib_streams(1, 1);
        let a = read_attrib(
            &mut b.streams(),
            DwgHandle(0x40),
            DwgVersion::R2018,
            &DwgLimits::default(),
        )
        .unwrap();
        assert!(a.invisible);
        assert_eq!(attrib(&a, false, &mut report()), Err(Skipped::Invisible));
        let template = AttribData {
            constant: false,
            invisible: false,
            ..a.clone()
        };
        assert_eq!(
            attrib(&template, true, &mut report()),
            Err(Skipped::AttributeTemplate)
        );
        assert!(matches!(
            attrib(&template, false, &mut report()),
            Ok(Shape::Text { .. })
        ));
    }

    #[test]
    fn a_multiline_attribute_is_named_not_read() {
        let b = attrib_streams(2, 0);
        let a = read_attrib(
            &mut b.streams(),
            DwgHandle(0x40),
            DwgVersion::R2018,
            &DwgLimits::default(),
        )
        .unwrap();
        assert!(a.multiline);
        assert!(matches!(
            attrib(&a, false, &mut report()),
            Err(Skipped::Unsupported(_))
        ));
    }

    #[test]
    fn a_style_record() {
        let mut data = BitWriter::new();
        data.bs(0)
            .bit(false)
            .bit(false)
            .bd(0.0)
            .bd(0.8)
            .bd(0.2)
            .rc(2)
            .bd(2.5);
        let mut strings = BitWriter::new();
        strings.text("Notes").text("arial.ttf").text("");
        let b = Buffers::new(&data, &strings, &BitWriter::new());
        let s = read_style(&mut b.streams(), DwgHandle(0x30), &DwgLimits::default()).unwrap();
        assert_eq!((s.name.as_str(), s.font.as_str()), ("Notes", "arial.ttf"));
        assert_eq!((s.width_factor, s.oblique, s.generation), (0.8, 0.2, 2));
        assert!(!s.vertical && !s.shape_file && s.big_font.is_empty());
    }
}
