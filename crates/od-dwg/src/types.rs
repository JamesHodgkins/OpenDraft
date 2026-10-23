//! Object type codes (Open Design Specification §20.3). Fixed codes name a
//! type directly; codes from 500 up index the class table.
//!
//! The specification's printed table is shifted for the table entries
//! (LTYPE_CONTROL to VPORT): the supplied drawings have the linetype control
//! object (handle 5) at 0x38, view 0x3C, UCS 0x3E, VPORT 0x40 and APPID 0x42,
//! and the records follow each control, so the codes here are those.

macro_rules! fixed_types {
    ($($variant:ident = $code:literal, $name:literal, $entity:literal;)*) => {
        /// An object type whose code never varies between files.
        #[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
        pub enum FixedType {
            $($variant,)*
        }

        impl FixedType {
            pub(crate) fn from_code(code: u32) -> Option<FixedType> {
                match code {
                    $($code => Some(FixedType::$variant),)*
                    _ => None,
                }
            }

            /// The DXF name of the type.
            pub fn name(self) -> &'static str {
                match self {
                    $(FixedType::$variant => $name,)*
                }
            }

            /// Whether objects of this type are drawing entities, which carry
            /// the extra common entity fields, rather than table entries or
            /// other objects.
            pub fn is_entity(self) -> bool {
                match self {
                    $(FixedType::$variant => $entity,)*
                }
            }
        }
    };
}

fixed_types! {
    Text = 0x01, "TEXT", true;
    Attrib = 0x02, "ATTRIB", true;
    Attdef = 0x03, "ATTDEF", true;
    Block = 0x04, "BLOCK", true;
    Endblk = 0x05, "ENDBLK", true;
    Seqend = 0x06, "SEQEND", true;
    Insert = 0x07, "INSERT", true;
    Minsert = 0x08, "MINSERT", true;
    Vertex2d = 0x0A, "VERTEX", true;
    Vertex3d = 0x0B, "VERTEX", true;
    VertexMesh = 0x0C, "VERTEX", true;
    VertexPface = 0x0D, "VERTEX", true;
    VertexPfaceFace = 0x0E, "VERTEX", true;
    Polyline2d = 0x0F, "POLYLINE", true;
    Polyline3d = 0x10, "POLYLINE", true;
    Arc = 0x11, "ARC", true;
    Circle = 0x12, "CIRCLE", true;
    Line = 0x13, "LINE", true;
    DimensionOrdinate = 0x14, "DIMENSION", true;
    DimensionLinear = 0x15, "DIMENSION", true;
    DimensionAligned = 0x16, "DIMENSION", true;
    DimensionAngular3Pt = 0x17, "DIMENSION", true;
    DimensionAngular2Ln = 0x18, "DIMENSION", true;
    DimensionRadius = 0x19, "DIMENSION", true;
    DimensionDiameter = 0x1A, "DIMENSION", true;
    Point = 0x1B, "POINT", true;
    Face3d = 0x1C, "3DFACE", true;
    PolylinePface = 0x1D, "POLYLINE", true;
    PolylineMesh = 0x1E, "POLYLINE", true;
    Solid = 0x1F, "SOLID", true;
    Trace = 0x20, "TRACE", true;
    Shape = 0x21, "SHAPE", true;
    Viewport = 0x22, "VIEWPORT", true;
    Ellipse = 0x23, "ELLIPSE", true;
    Spline = 0x24, "SPLINE", true;
    Region = 0x25, "REGION", true;
    Solid3d = 0x26, "3DSOLID", true;
    Body = 0x27, "BODY", true;
    Ray = 0x28, "RAY", true;
    Xline = 0x29, "XLINE", true;
    Dictionary = 0x2A, "DICTIONARY", false;
    OleFrame = 0x2B, "OLEFRAME", true;
    Mtext = 0x2C, "MTEXT", true;
    Leader = 0x2D, "LEADER", true;
    Tolerance = 0x2E, "TOLERANCE", true;
    Mline = 0x2F, "MLINE", true;
    BlockControl = 0x30, "BLOCK_CONTROL", false;
    BlockHeader = 0x31, "BLOCK_RECORD", false;
    LayerControl = 0x32, "LAYER_CONTROL", false;
    Layer = 0x33, "LAYER", false;
    StyleControl = 0x34, "STYLE_CONTROL", false;
    Style = 0x35, "STYLE", false;
    LinetypeControl = 0x38, "LTYPE_CONTROL", false;
    Linetype = 0x39, "LTYPE", false;
    ViewControl = 0x3C, "VIEW_CONTROL", false;
    View = 0x3D, "VIEW", false;
    UcsControl = 0x3E, "UCS_CONTROL", false;
    Ucs = 0x3F, "UCS", false;
    VportControl = 0x40, "VPORT_CONTROL", false;
    Vport = 0x41, "VPORT", false;
    AppidControl = 0x42, "APPID_CONTROL", false;
    Appid = 0x43, "APPID", false;
    DimstyleControl = 0x44, "DIMSTYLE_CONTROL", false;
    Dimstyle = 0x45, "DIMSTYLE", false;
    VpEntHdrControl = 0x46, "VP_ENT_HDR_CONTROL", false;
    VpEntHdr = 0x47, "VP_ENT_HDR", false;
    Group = 0x48, "GROUP", false;
    Mlinestyle = 0x49, "MLINESTYLE", false;
    Ole2Frame = 0x4A, "OLE2FRAME", true;
    LongTransaction = 0x4C, "LONG_TRANSACTION", false;
    Lwpolyline = 0x4D, "LWPOLYLINE", true;
    Hatch = 0x4E, "HATCH", true;
    Xrecord = 0x4F, "XRECORD", false;
    Placeholder = 0x50, "ACDBPLACEHOLDER", false;
    VbaProject = 0x51, "VBA_PROJECT", false;
    Layout = 0x52, "LAYOUT", false;
    ProxyEntity = 0x1F2, "ACAD_PROXY_ENTITY", true;
    ProxyObject = 0x1F3, "ACAD_PROXY_OBJECT", false;
}

use crate::classes::ClassRegistry;
use crate::error::DwgErrorKind;

/// The first code that indexes the class table.
pub(crate) const FIRST_CLASS_CODE: u32 = 500;

/// What an object is, from its type code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObjectKind {
    Fixed(FixedType),
    /// A type defined by a class in the file.
    Class {
        number: u16,
        dxf_name: String,
    },
    /// A code below 500 this reader has no name for. The object is counted but
    /// its contents are not read.
    Unknown(u32),
}

impl ObjectKind {
    /// The DXF name, or `UNKNOWN_<code>`.
    pub fn name(&self) -> String {
        match self {
            ObjectKind::Fixed(t) => t.name().to_owned(),
            ObjectKind::Class { dxf_name, .. } => dxf_name.clone(),
            ObjectKind::Unknown(code) => format!("UNKNOWN_{code}"),
        }
    }
}

/// Names a type code. A class code outside the table is corruption; a fixed
/// code with no name is merely unknown.
pub(crate) fn classify(
    code: u32,
    classes: &ClassRegistry,
) -> Result<(ObjectKind, bool), DwgErrorKind> {
    if code >= FIRST_CLASS_CODE {
        let class = classes.get(code).ok_or(DwgErrorKind::InvalidObject)?;
        let kind = ObjectKind::Class {
            number: class.number,
            dxf_name: class.dxf_name.clone(),
        };
        return Ok((kind, class.is_entity));
    }
    Ok(match FixedType::from_code(code) {
        Some(t) => (ObjectKind::Fixed(t), t.is_entity()),
        None => (ObjectKind::Unknown(code), false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_geometry_types_have_their_documented_codes() {
        assert_eq!(FixedType::from_code(0x13), Some(FixedType::Line));
        assert_eq!(FixedType::from_code(0x12), Some(FixedType::Circle));
        assert_eq!(FixedType::from_code(0x11), Some(FixedType::Arc));
        assert_eq!(FixedType::from_code(0x4D), Some(FixedType::Lwpolyline));
        assert_eq!(FixedType::from_code(0x0F), Some(FixedType::Polyline2d));
        assert_eq!(FixedType::from_code(0x2C), Some(FixedType::Mtext));
        assert_eq!(FixedType::from_code(0x33), Some(FixedType::Layer));
        assert_eq!(FixedType::from_code(0x52), Some(FixedType::Layout));
        assert_eq!(FixedType::from_code(0x38), Some(FixedType::LinetypeControl));
        assert_eq!(FixedType::from_code(0x41), Some(FixedType::Vport));
        assert_eq!(FixedType::from_code(0x1F2), Some(FixedType::ProxyEntity));
    }

    #[test]
    fn unlisted_codes_are_unknown_and_codes_are_unique() {
        assert_eq!(FixedType::from_code(0), None);
        assert_eq!(FixedType::from_code(9), None);
        assert_eq!(FixedType::from_code(0x4B), None);
        assert_eq!(FixedType::from_code(500), None);
        // A table with a repeated code would hide the later variant.
        for code in 0..0x200 {
            if let Some(t) = FixedType::from_code(code) {
                assert!(!t.name().is_empty());
            }
        }
    }

    #[test]
    fn entities_are_told_from_table_entries() {
        assert!(FixedType::Line.is_entity());
        assert!(FixedType::Viewport.is_entity());
        assert!(!FixedType::Layer.is_entity());
        assert!(!FixedType::Dictionary.is_entity());
        assert!(!FixedType::BlockHeader.is_entity());
    }
}
