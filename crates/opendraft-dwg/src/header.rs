//! `AcDb:Header`: the drawing's system variables (Open Design Specification
//! chapter 9), read for R2010 and later.
//!
//! The variables are a long fixed sequence, so the only way to reach a late
//! one such as `INSUNITS` is to read every one before it. Everything is read
//! and the data and string streams must end exactly where the last field
//! does, which is the check that no field was misread. Three places differ
//! from the specification's list: the four trailing unknown shorts ("type 5/6
//! files") are absent in R2010 and later, the handle stream ends in random
//! fill, and the string stream holds one more (empty) text than the list has
//! TV fields, somewhere before the GUIDs.

use crate::bit::BitReader;
use crate::common::read_table_color;
use crate::error::{DwgError, DwgErrorKind, DwgStage};
use crate::framed::{frame, HEADER_END, HEADER_START};
use crate::handles::DwgHandle;
use crate::limits::DwgLimits;
use crate::object::split_streams;
use crate::version::DwgVersion;

/// The system variables the importer uses.
#[derive(Clone, Debug, PartialEq)]
pub struct DwgHeader {
    /// `LUNITS`: 1 scientific, 2 decimal, 3 engineering, 4 architectural, 5 fractional.
    pub linear_units: u16,
    /// `LUPREC`: decimal places of linear units.
    pub linear_precision: u16,
    /// `AUNITS`: 0 decimal degrees, 1 degrees/minutes/seconds, 2 gradians, 3 radians, 4 surveyor's.
    pub angular_units: u16,
    pub angular_precision: u16,
    /// `ANGBASE`: the zero direction, in radians.
    pub angle_base: f64,
    /// `ANGDIR`: angles run clockwise.
    pub angles_clockwise: bool,
    /// `INSUNITS`: the units of inserted drawings (0 unitless, 1 inches, 4 millimetres, ...).
    pub insert_units: u16,
    pub linetype_scale: f64,
    /// `TILEMODE`: model space is current (the paper-space tab is not).
    pub model_space_current: bool,
    /// `CLAYER`, `*MODEL_SPACE` and `*PAPER_SPACE`.
    pub current_layer: Option<DwgHandle>,
    pub model_space: Option<DwgHandle>,
    pub paper_space: Option<DwgHandle>,
    /// `HANDSEED`: the next handle AutoCAD would assign.
    pub handle_seed: DwgHandle,
}

struct Streams<'a, 'b> {
    data: &'b mut BitReader<'a>,
    strings: &'b mut BitReader<'a>,
    handles: &'b mut BitReader<'a>,
    limits: &'b DwgLimits,
}

impl Streams<'_, '_> {
    fn bds(&mut self, n: usize) -> Result<(), DwgError> {
        (0..n).try_for_each(|_| self.data.bd().map(drop))
    }

    fn bools(&mut self, n: usize) -> Result<(), DwgError> {
        (0..n).try_for_each(|_| self.data.bit().map(drop))
    }

    fn shorts(&mut self, n: usize) -> Result<(), DwgError> {
        (0..n).try_for_each(|_| self.data.bs().map(drop))
    }

    fn longs(&mut self, n: usize) -> Result<(), DwgError> {
        (0..n).try_for_each(|_| self.data.bl().map(drop))
    }

    fn bytes(&mut self, n: usize) -> Result<(), DwgError> {
        (0..n).try_for_each(|_| self.data.rc().map(drop))
    }

    fn texts(&mut self, n: usize) -> Result<(), DwgError> {
        let max = self.limits.max_string_code_units;
        (0..n).try_for_each(|_| self.strings.text(max).map(drop))
    }

    fn colors(&mut self, n: usize) -> Result<(), DwgError> {
        for _ in 0..n {
            read_table_color(self.data, self.strings, self.limits)?;
        }
        Ok(())
    }

    /// A handle from the handle stream; null is `None`.
    fn handle(&mut self) -> Result<Option<DwgHandle>, DwgError> {
        let reference = self.handles.handle(self.limits.max_handle_bytes)?;
        reference
            .resolve(DwgHandle(0))
            .map_err(|e| e.at(self.handles.position()))
    }

    fn handles(&mut self, n: usize) -> Result<(), DwgError> {
        (0..n).try_for_each(|_| self.handle().map(drop))
    }

    /// The nine point and view settings that repeat for model and paper space.
    fn space_settings(&mut self, paper: bool) -> Result<(), DwgError> {
        if paper {
            self.bds(1)?; // PSVPSCALE
        }
        self.bds(3 * 3)?; // INSBASE, EXTMIN, EXTMAX
        self.data.rd2()?; // LIMMIN
        self.data.rd2()?; // LIMMAX
        self.bds(1 + 9)?; // ELEVATION, UCSORG, UCSXDIR, UCSYDIR
        self.handles(1)?; // UCSNAME
        self.handles(1)?; // (P)UCSORTHOREF
        self.shorts(1)?; // (P)UCSORTHOVIEW
        self.handles(1)?; // (P)UCSBASE
        self.bds(6 * 3) // the six orthographic origins
    }
}

/// Reads the decompressed `AcDb:Header` section.
pub(crate) fn parse_header(
    section: &[u8],
    version: DwgVersion,
    limits: &DwgLimits,
) -> Result<DwgHeader, DwgError> {
    let stage = DwgStage::Header;
    let sentinels = (&HEADER_START, &HEADER_END);
    let region = frame(section, version, sentinels, stage, "AcDb:Header")?;
    // The region opens with the bit count where the handle stream starts.
    let invalid = || DwgError::new(DwgErrorKind::InvalidObject, stage);
    let total = region.len() as u64 * 8;
    let first = region.get(..4).ok_or_else(invalid)?;
    let used = u64::from(u32::from_le_bytes([first[0], first[1], first[2], first[3]]));
    let mut split = split_streams(region, total.checked_sub(used).ok_or_else(invalid)?, stage)?;
    split.data.skip(32)?;
    let mut s = Streams {
        data: &mut split.data,
        strings: &mut split.strings,
        handles: &mut split.handles,
        limits,
    };

    if version >= DwgVersion::R2013 {
        s.data.bll()?; // REQUIREDVERSIONS
    }
    s.bds(4)?;
    s.texts(4)?;
    s.longs(2)?;
    // DIMASO .. LIMCHECK (9), an undocumented bit, USRTIMER, SKPOLY.
    s.bools(9 + 1 + 1 + 1)?;
    let angles_clockwise = s.data.bit()?; // ANGDIR
    s.bools(3)?; // SPLFRAME, MIRRTEXT, WORLDVIEW
    let model_space_current = s.data.bit()?; // TILEMODE
    s.bools(2 + 2)?; // PLIMCHECK, VISRETAIN, DISPSILH, PELLIPSE
    s.shorts(2)?; // PROXYGRAPHICS, TREEDEPTH
    let linear_units = s.data.bs()?;
    let linear_precision = s.data.bs()?;
    let angular_units = s.data.bs()?;
    let angular_precision = s.data.bs()?;
    s.shorts(2)?; // ATTMODE, PDMODE
    s.longs(3)?;
    s.shorts(5 + 14)?; // USERI1-5, SPLINESEGS .. TEXTQLTY
    let linetype_scale = s.data.bd()?;
    s.bds(2 + 3)?; // TEXTSIZE, TRACEWID, SKETCHINC, FILLETRAD, THICKNESS
    let angle_base = s.data.bd()?;
    s.bds(2 + 5 + 4 + 3)?; // PDSIZE, PLINEWID, USERR1-5, CHAMFERA-D, FACETRES, CMLSCALE, CELTSCALE
    s.longs(4 + 3 + 4)?; // TDCREATE, TDUPDATE, unknown, TDINDWG, TDUSRTIMER
    s.colors(1)?; // CECOLOR
    let handle_seed = DwgHandle(s.data.handle(limits.max_handle_bytes)?.value);
    let current_layer = s.handle()?;
    s.handles(3)?; // TEXTSTYLE, CELTYPE, CMATERIAL
    s.handles(2)?; // DIMSTYLE, CMLSTYLE
    s.space_settings(true)?;
    s.space_settings(false)?;
    s.texts(2)?; // DIMPOST, DIMAPOST
    s.bds(9)?; // DIMSCALE .. DIMTM
    s.bds(2)?; // DIMFXL, DIMJOGANG
    s.shorts(1)?; // DIMTFILL
    s.colors(1)?; // DIMTFILLCLR
    s.bools(6)?; // DIMTOL .. DIMSE2
    s.shorts(3 + 1)?; // DIMTAD, DIMZIN, DIMAZIN, DIMARCSYM
    s.bds(8)?; // DIMTXT .. DIMGAP
    s.bds(1)?; // DIMALTRND
    s.bools(1)?; // DIMALT
    s.shorts(1)?; // DIMALTD
    s.bools(4)?; // DIMTOFL, DIMSAH, DIMTIX, DIMSOXD
    s.colors(3)?; // DIMCLRD, DIMCLRE, DIMCLRT
    s.shorts(11)?; // DIMADEC .. DIMJUST
    s.bools(2)?; // DIMSD1, DIMSD2
    s.shorts(4)?; // DIMTOLJ, DIMTZIN, DIMALTZ, DIMALTTZ
    s.bools(1)?; // DIMUPT
    s.shorts(1)?; // DIMATFIT
    s.bools(2)?; // DIMFXLON, DIMTXTDIRECTION
    s.bds(1)?; // DIMALTMZF
    s.texts(1)?; // DIMALTMZS
    s.bds(1)?; // DIMMZF
    s.texts(1)?; // DIMMZS
    s.handles(5 + 3)?; // DIMTXSTY, DIMLDRBLK, DIMBLK, DIMBLK1, DIMBLK2, DIMLTYPE, DIMLTEX1, DIMLTEX2
    s.shorts(2)?; // DIMLWD, DIMLWE
    s.handles(9)?; // the nine table control objects
    s.handles(3)?; // ACAD_GROUP, ACAD_MLINESTYLE, named objects
    s.shorts(2)?; // TSTACKALIGN, TSTACKSIZE
    s.texts(2)?; // HYPERLINKBASE, STYLESHEET
    s.handles(3 + 2 + 1)?; // LAYOUTS, PLOTSETTINGS, PLOTSTYLES, MATERIALS, COLORS, VISUALSTYLE
    if version >= DwgVersion::R2013 {
        s.handles(1)?;
    }
    s.data.bl()?; // CELWEIGHT, ENDCAPS, JOINSTYLE and other flags
    let insert_units = s.data.bs()?;
    if s.data.bs()? == 3 {
        s.handles(1)?; // CPSNID
    }
    s.texts(1 + 2)?; // an unlisted empty text, FINGERPRINTGUID, VERSIONGUID
    s.bytes(6)?; // SORTENTS .. HALOGAP
    s.shorts(2)?; // OBSCUREDCOLOR, INTERSECTIONCOLOR
    s.bytes(2)?; // OBSCUREDLTYPE, INTERSECTIONDISPLAY
    s.texts(1)?; // PROJECTNAME
    let paper_space = s.handle()?;
    let model_space = s.handle()?;
    s.handles(3)?; // the BYLAYER, BYBLOCK and CONTINUOUS linetypes
    s.bools(1)?; // CAMERADISPLAY
    s.longs(2)?;
    s.bds(1 + 5)?; // unknown, STEPSPERSEC, STEPSIZE, 3DDWFPREC, LENSLENGTH, CAMERAHEIGHT
    s.bytes(2)?; // SOLIDHIST, SHOWHIST
    s.bds(6)?; // PSOLWIDTH .. LOFTMAG2
    s.shorts(1)?; // LOFTPARAM
    s.bytes(1)?; // LOFTNORMALS
    s.bds(3)?; // LATITUDE, LONGITUDE, NORTHDIRECTION
    s.longs(1)?; // TIMEZONE
    s.bytes(4)?; // LIGHTGLYPHDISPLAY .. DGNFRAME
    s.bools(1)?;
    s.colors(1)?; // INTERFERECOLOR
    s.handles(3)?; // INTERFEREOBJVS, INTERFEREVPVS, DRAGVS
    s.bytes(1)?; // CSHADOW
    s.bds(1)?;

    // The data and strings must be used up exactly. The handle stream ends
    // with random fill, so what is left of it says nothing.
    if s.data.remaining() + s.strings.remaining() != 0 {
        return Err(invalid().at(s.data.position()));
    }
    Ok(DwgHeader {
        linear_units,
        linear_precision,
        angular_units,
        angular_precision,
        angle_base,
        angles_clockwise,
        insert_units,
        linetype_scale,
        model_space_current,
        current_layer,
        model_space,
        paper_space,
        handle_seed,
    })
}
