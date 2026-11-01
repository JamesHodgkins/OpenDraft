//! What a command can ask the user, and what comes back. The command line,
//! the canvas and (later) plugins all answer these same requests.

use std::fmt;
use std::sync::Arc;

use od_core::block::attribute_draft::{DefinitionDraft, EditorDraft};
use od_core::block::block_draft::BlockDraft;
use od_core::{overkill, Entity, EntityId, MText, Point};

/// The user cancelled (Esc), or the command was abandoned. Commands pass it up
/// with `?`; a cancelled transaction leaves no trace in the drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cancelled;

impl fmt::Display for Cancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("cancelled")
    }
}

impl std::error::Error for Cancelled {}

/// An option offered at a prompt, like `Undo` in `[Undo]`. Typing `key` (or
/// the start of the label) picks it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keyword {
    pub label: String,
    pub key: String,
}

impl Keyword {
    pub fn new(label: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            key: key.into(),
        }
    }
}

/// Draws the objects a command is about to make while the user chooses a
/// point: given the pointer position, returns the entities to show.
pub type Ghost = Arc<dyn Fn(Point) -> Vec<Entity> + Send + Sync>;

/// Draws marks that keep one size on the screen: called with how many
/// drawing units a screen point spans.
pub type Marker = Arc<dyn Fn(f64) -> Vec<Entity> + Send + Sync>;

/// How the ghost is drawn: the usual preview, or the colours of a selection
/// region (blue for Window, green and dashed for Crossing and Fence).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GhostStyle {
    #[default]
    Normal,
    Window,
    Crossing,
}

/// Cursor field layout declared by the command, independent of its UI.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DynamicDimensions {
    #[default]
    None,
    Polar,
    Cartesian,
}

/// How a prompt is worded and what it accepts. Build with
/// `Options::new("Specify base point")`, or pass a plain `&str`.
#[derive(Clone)]
pub struct Options<T> {
    pub dynamic_dimensions: DynamicDimensions,
    pub message: String,
    pub keywords: Vec<Keyword>,
    /// Used when the user presses Enter with nothing typed.
    pub default: Option<T>,
    /// Origin for relative input, the rubber band, and distances and angles.
    pub base: Option<Point>,
    /// Live preview shown while choosing a point.
    pub ghost: Option<Ghost>,
    pub ghost_style: GhostStyle,
    /// Objects not yet in the drawing that the pointer snaps to as well (the
    /// parts of a polyline drawn so far).
    pub snap_geometry: Arc<[Entity]>,
    /// Draw a line from `base` to the pointer (the default).
    pub rubber_band: bool,
    /// Marks drawn at a fixed size on the screen, whatever the zoom.
    pub marker: Option<Marker>,
    /// For a selection prompt: Enter with nothing selected is an answer (an
    /// empty selection), not a request to select something first.
    pub allow_empty: bool,
    /// For an object prompt: objects on locked layers may be picked too, as
    /// LAYULK needs.
    pub pick_locked: bool,
    /// Esc steps back out of this prompt (it answers [`Reply::Back`], and the
    /// ask fails with `Cancelled`) without ending the command, for a command
    /// that returns to a dialog.
    pub esc_backs: bool,
}

pub type PointOptions = Options<Point>;
pub type NumberOptions = Options<f64>;
pub type TextOptions = Options<String>;

impl<T> Options<T> {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            dynamic_dimensions: DynamicDimensions::None,
            message: message.into(),
            keywords: Vec::new(),
            default: None,
            base: None,
            ghost: None,
            ghost_style: GhostStyle::Normal,
            snap_geometry: Arc::default(),
            rubber_band: true,
            marker: None,
            allow_empty: false,
            pick_locked: false,
            esc_backs: false,
        }
    }

    /// Offer a keyword; answer it with `ask_*`.
    pub fn keyword(mut self, label: impl Into<String>, key: impl Into<String>) -> Self {
        self.keywords.push(Keyword::new(label, key));
        self
    }

    pub fn default(mut self, value: T) -> Self {
        self.default = Some(value);
        self
    }

    /// Measure from `base`: shows a rubber band and lets `@x,y` be relative.
    pub fn base(mut self, base: Point) -> Self {
        self.base = Some(base);
        self
    }

    /// Take the corner of a box: the base still anchors relative input and
    /// the dimensions, but no line is drawn to the pointer.
    pub fn corner(mut self) -> Self {
        self.rubber_band = false;
        self
    }

    pub fn dimensions(mut self, dimensions: DynamicDimensions) -> Self {
        self.dynamic_dimensions = dimensions;
        self
    }

    /// Show marks that stay the same size on the screen (a vertex cross,
    /// say): `marker` is given how many drawing units a screen point spans.
    pub fn marker(mut self, marker: impl Fn(f64) -> Vec<Entity> + Send + Sync + 'static) -> Self {
        self.marker = Some(Arc::new(marker));
        self
    }

    /// Preview what the command will make at each pointer position.
    pub fn ghost(mut self, ghost: impl Fn(Point) -> Vec<Entity> + Send + Sync + 'static) -> Self {
        self.ghost = Some(Arc::new(ghost));
        self
    }
}

impl<T> Options<T> {
    /// Let a selection prompt be answered with nothing selected.
    pub fn allow_empty(mut self) -> Self {
        self.allow_empty = true;
        self
    }

    /// Let Esc step back out of the prompt instead of ending the command
    /// (see [`Options::esc_backs`]).
    pub fn esc_backs(mut self) -> Self {
        self.esc_backs = true;
        self
    }

    /// Let an object prompt pick objects on locked layers.
    pub fn pick_locked(mut self) -> Self {
        self.pick_locked = true;
        self
    }

    /// Let the pointer snap to `entities` too, as if they were in the drawing.
    pub fn snap_to(mut self, entities: Vec<Entity>) -> Self {
        self.snap_geometry = entities.into();
        self
    }

    /// Draw the ghost in the colours of a selection region.
    pub fn ghost_style(mut self, style: GhostStyle) -> Self {
        self.ghost_style = style;
        self
    }
}

impl<T> From<&str> for Options<T> {
    fn from(message: &str) -> Self {
        Self::new(message)
    }
}

impl<T> From<String> for Options<T> {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

/// What kind of answer a prompt wants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wants {
    Point,
    /// A length: typed, or picked as the distance from the base point.
    Distance,
    /// An angle in radians, counter-clockwise: typed in degrees, or picked as
    /// the direction from the base point.
    Angle,
    /// An angle as a typed value, or a first point (the caller then asks for
    /// a second to measure the direction between them).
    AngleOrPoint,
    /// Likewise for a length.
    DistanceOrPoint,
    Number,
    Text,
    /// One of the keywords, nothing else.
    Keyword,
    /// Objects chosen on the canvas; Enter accepts the current selection.
    Selection,
    /// One object, clicked: the answer is the object and where it was clicked.
    Object,
    /// The text in [`Prompt::mtext`] edited in place; the answer is the
    /// edited text.
    MTextEditor,
    /// The OVERKILL settings dialog, opened on [`Prompt::overkill`]; the
    /// answer is the settings it was closed with.
    OverkillDialog,
    /// The Attribute Definition dialog, opened on
    /// [`Prompt::attribute_definition`].
    AttributeDefinitionDialog,
    /// The Attribute Editor, opened on [`Prompt::attribute_editor`].
    AttributeEditorDialog,
    /// The Block Definition dialog, opened on [`Prompt::block_dialog`].
    BlockDialog,
}

/// The default a prompt offers, left as a value so the UI can write it in the
/// drawing's units (`<0'-5">`, `<45d30'>`) once it knows which kind it is.
#[derive(Clone, Debug, PartialEq)]
pub enum Hint {
    Text(String),
    /// A distance, an angle (radians) or a plain number, by the prompt's
    /// [`Wants`].
    Number(f64),
    Point(Point),
}

/// A request for input, as the UI sees it.
#[derive(Clone)]
pub struct Prompt {
    pub dynamic_dimensions: DynamicDimensions,
    pub wants: Wants,
    pub message: String,
    pub keywords: Vec<Keyword>,
    pub base: Option<Point>,
    pub ghost: Option<Ghost>,
    pub ghost_style: GhostStyle,
    /// See [`Options::snap_geometry`].
    pub snap_geometry: Arc<[Entity]>,
    pub rubber_band: bool,
    pub marker: Option<Marker>,
    /// Enter alone is accepted (a default exists); the UI may show it.
    pub default_hint: Option<Hint>,
    /// See [`Options::allow_empty`].
    pub allow_empty: bool,
    /// See [`Options::pick_locked`].
    pub pick_locked: bool,
    /// See [`Options::esc_backs`].
    pub esc_backs: bool,
    /// For [`Wants::MTextEditor`]: the text to edit.
    pub mtext: Option<Box<MText>>,
    /// For [`Wants::MTextEditor`]: the text object being edited, if there is
    /// one, so the drawing can leave it out while its draft is shown.
    pub mtext_of: Option<EntityId>,
    /// For [`Wants::OverkillDialog`]: the settings to start from.
    pub overkill: Option<overkill::Settings>,
    /// For [`Wants::AttributeDefinitionDialog`]: what it opens with.
    pub attribute_definition: Option<Box<DefinitionDraft>>,
    /// For [`Wants::AttributeEditorDialog`]: what it opens with.
    pub attribute_editor: Option<Box<EditorDraft>>,
    /// For [`Wants::BlockDialog`]: what it opens with.
    pub block_dialog: Option<Box<BlockDraft>>,
}

impl Prompt {
    pub(crate) fn new<T>(wants: Wants, options: &Options<T>, hint: Option<Hint>) -> Self {
        Self {
            dynamic_dimensions: options.dynamic_dimensions,
            wants,
            message: options.message.clone(),
            keywords: options.keywords.clone(),
            base: options.base,
            ghost: options.ghost.clone(),
            ghost_style: options.ghost_style,
            snap_geometry: options.snap_geometry.clone(),
            rubber_band: options.rubber_band,
            marker: options.marker.clone(),
            default_hint: hint,
            allow_empty: options.allow_empty,
            pick_locked: options.pick_locked,
            esc_backs: options.esc_backs,
            mtext: None,
            mtext_of: None,
            overkill: None,
            attribute_definition: None,
            attribute_editor: None,
            block_dialog: None,
        }
    }
}

/// The button that closed the Block Definition dialog. The pick buttons hide
/// it while the user picks in the drawing, and it opens again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockDialogButton {
    Ok,
    PickPoint,
    SelectObjects,
}

/// What the UI sends back.
#[derive(Clone, Debug, PartialEq)]
pub enum Reply {
    Point(Point),
    Number(f64),
    Text(String),
    /// The text the editor was closed with.
    MText(Box<MText>),
    /// The settings the OVERKILL dialog was closed with.
    Overkill(overkill::Settings),
    /// The Attribute Definition dialog, accepted.
    AttributeDefinition(Box<DefinitionDraft>),
    /// The Attribute Editor: what it holds, and whether it is
    /// closing (OK) or staying open (Apply).
    AttributeEditor(Box<EditorDraft>, bool),
    /// The Attribute Editor, closed to choose another block reference.
    AttributeEditorSelect(Box<EditorDraft>),
    /// The Block Definition dialog, closed by a button.
    BlockDialog(Box<BlockDraft>, BlockDialogButton),
    /// The label of a chosen keyword.
    Keyword(String),
    Selection(Vec<EntityId>),
    /// An object clicked, where, and whether Shift was held.
    Object {
        id: EntityId,
        at: Point,
        shift: bool,
    },
    /// Enter with nothing typed.
    Enter,
    Cancel,
    /// Esc at a prompt that asked for [`Options::esc_backs`]: that ask fails
    /// with `Cancelled`, and the command goes on.
    Back,
}

/// The answer to `Editor::ask_object`: the object that was clicked, the point
/// it was clicked at, and whether Shift was held.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Picked {
    pub id: EntityId,
    pub at: Point,
    pub shift: bool,
}

/// The answer to `Editor::ask_angle_or_point` and `ask_distance_or_point`: a
/// typed number (radians for an angle), or a point that was picked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NumberOrPoint {
    Number(f64),
    Point(Point),
}

/// An answer to an `ask_*` call: a value, a keyword, or a bare Enter.
#[derive(Clone, Debug, PartialEq)]
pub enum Answer<T> {
    Value(T),
    /// The label of the keyword chosen.
    Keyword(String),
    Enter,
}
