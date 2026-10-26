//! The opposite corner of a box, like AutoCAD's `getcorner`: measured from a
//! base point, with no line to the pointer, X and Y in dynamic input and, unless
//! the command draws its own, an outline of the box.

use od_core::{Entity, Point};

use crate::prompt::{Answer, Cancelled, DynamicDimensions, Options};
use crate::Editor;

impl Editor {
    /// Ask for the corner opposite `base`. Keywords and a bare Enter come back
    /// as such. A ghost already on `options` (a rotated box, say) is kept.
    pub fn ask_corner(
        &self,
        base: Point,
        options: impl Into<Options<Point>>,
    ) -> Result<Answer<Point>, Cancelled> {
        let mut options = options
            .into()
            .base(base)
            .corner()
            .dimensions(DynamicDimensions::Cartesian);
        if options.ghost.is_none() {
            options = options.ghost(move |at| vec![Entity::Rectangle { min: base, max: at }]);
        }
        self.ask_point(options)
    }

    /// Like [`Editor::ask_corner`], for prompts with no keywords.
    pub fn get_corner(
        &self,
        base: Point,
        options: impl Into<Options<Point>>,
    ) -> Result<Point, Cancelled> {
        match self.ask_corner(base, options)? {
            Answer::Value(corner) => Ok(corner),
            _ => Err(Cancelled),
        }
    }
}
