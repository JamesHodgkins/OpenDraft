//! Layers and the display attributes objects take from them.

use crate::linetype::LinetypeId;

/// Permanent identity of a layer; never reused within a drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LayerId(pub u64);

impl LayerId {
    /// The layer every drawing starts with. It cannot be deleted.
    pub const DEFAULT: LayerId = LayerId(0);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    /// The drawing foreground: light on a dark canvas, dark on a light one
    /// (colour 7 in AutoCAD).
    Foreground,
    Rgb(u8, u8, u8),
}

/// Line width in hundredths of a millimetre.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Lineweight(pub u16);

impl Lineweight {
    pub const DEFAULT: Lineweight = Lineweight(25);

    /// The widths offered in the UI (the standard AutoCAD set).
    pub const STANDARD: &'static [Lineweight] = &[
        Lineweight(0),
        Lineweight(5),
        Lineweight(9),
        Lineweight(13),
        Lineweight(15),
        Lineweight(18),
        Lineweight(20),
        Lineweight(25),
        Lineweight(30),
        Lineweight(35),
        Lineweight(40),
        Lineweight(50),
        Lineweight(53),
        Lineweight(60),
        Lineweight(70),
        Lineweight(80),
        Lineweight(90),
        Lineweight(100),
        Lineweight(106),
        Lineweight(120),
        Lineweight(140),
        Lineweight(158),
        Lineweight(200),
        Lineweight(211),
    ];

    pub fn mm(self) -> f32 {
        f32::from(self.0) / 100.0
    }
}

/// An object attribute that either follows its layer or is set on the object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Attr<T> {
    ByLayer,
    Set(T),
}

impl<T: Clone> Attr<T> {
    /// The effective value, given what the layer says.
    pub fn resolve(&self, layer_value: &T) -> T {
        match self {
            Attr::ByLayer => layer_value.clone(),
            Attr::Set(v) => v.clone(),
        }
    }
}

/// How an object is displayed: its layer, and any overrides of the layer's
/// colour, linetype and lineweight, plus how its linetype pattern is laid.
#[derive(Clone, Debug, PartialEq)]
pub struct Attributes {
    pub layer: LayerId,
    pub color: Attr<Color>,
    pub linetype: Attr<LinetypeId>,
    pub lineweight: Attr<Lineweight>,
    /// The object's own linetype scale (DXF group 48), above zero; multiplied
    /// by the drawing's `LTSCALE`.
    pub linetype_scale: f64,
    /// Polylines and rectangles only: lay the pattern along the whole outline
    /// (AutoCAD's Linetype Generation, DXF group 70 bit 128) instead of
    /// starting it afresh on every segment.
    pub continuous_generation: bool,
}

impl Attributes {
    /// Everything ByLayer, on `layer`.
    pub fn on_layer(layer: LayerId) -> Self {
        Self {
            layer,
            color: Attr::ByLayer,
            linetype: Attr::ByLayer,
            lineweight: Attr::ByLayer,
            linetype_scale: 1.0,
            continuous_generation: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    pub color: Color,
    pub linetype: LinetypeId,
    pub lineweight: Lineweight,
    /// Off layers are hidden.
    pub on: bool,
    /// Frozen layers are hidden too (and, later, skipped by regeneration).
    pub frozen: bool,
    /// Locked layers are shown but their objects cannot be selected.
    pub locked: bool,
}

impl Layer {
    pub fn new(id: LayerId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            color: Color::Foreground,
            linetype: LinetypeId::CONTINUOUS,
            lineweight: Lineweight::DEFAULT,
            on: true,
            frozen: false,
            locked: false,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.on && !self.frozen
    }
}
