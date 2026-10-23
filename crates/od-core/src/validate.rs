//! Checking that a drawing is internally consistent.
//!
//! The mutating methods on `Document` are the ones undo and redo need, so
//! they restore what they are given without question. This is the net under
//! them: the session checks it after every edit in debug builds and tests,
//! and a file is never written from a drawing that fails it.

use std::collections::HashSet;
use std::fmt;

use crate::{
    Document, EllipseError, Entity, EntityId, LayerId, MLeaderStyleId, PolylineError, SplineError,
    TextStyleId,
};

/// What is wrong with a drawing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentError {
    NoDefaultLayer,
    DuplicateLayerId(LayerId),
    DuplicateLayerName(String),
    CurrentLayerMissing(LayerId),
    DuplicateObjectId(EntityId),
    /// An object sits on a layer that does not exist.
    MissingLayer(EntityId, LayerId),
    /// A counter would hand out an id that is already in use.
    CounterBehind,
    /// Two layouts share an id or a name, or one has a bad sheet size or repeats a viewport id.
    BadLayout(String),
    /// An object belongs to a layout that does not exist.
    MissingLayoutOf(EntityId),
    /// The text-style table lacks `Standard` first, or has a repeated id or
    /// name, or a style that could not exist.
    BadTextStyle(String),
    CurrentTextStyleMissing(TextStyleId),
    /// The multileader-style table lacks `Standard` first, has a repeated id
    /// or name, a style that could not exist or one using a missing text
    /// style, or a counter behind its ids.
    BadMLeaderStyle(String),
    CurrentMLeaderStyleMissing(MLeaderStyleId),
    /// The dimension-style table lacks `Standard` first, repeats a name, or
    /// leaves out the current style.
    BadDimStyle(String),
    /// Multiline text uses a style that does not exist.
    MissingTextStyle(EntityId, TextStyleId),
    /// Multiline text that could not exist, with the reason.
    BadMText(EntityId, String),
    /// A multileader that could not exist or names a missing style, with the reason.
    BadMLeader(EntityId, String),
    /// A leader that could not exist, with the reason.
    BadLeader(EntityId, String),
    /// An ellipse that could not exist, or is not in canonical form.
    BadEllipse(EntityId, EllipseError),
    /// A hatch that could not exist, with the reason.
    BadHatch(EntityId, String),
    /// A spline that could not exist.
    BadSpline(EntityId, SplineError),
    /// A polyline that could not exist.
    BadPolyline(EntityId, PolylineError),
    /// A block definition has a repeated id or name, a bad name, an object
    /// on a missing layer, an insert of a block not in the table, or an
    /// ellipse or spline that could not exist.
    BadBlock(String),
    /// An insert refers to a block not in the table, or has a zero or
    /// non-finite scale.
    BadInsert(EntityId),
}

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoDefaultLayer => write!(f, "the drawing has no layer 0"),
            Self::DuplicateLayerId(id) => write!(f, "layer id {} is used twice", id.0),
            Self::DuplicateLayerName(name) => write!(f, "two layers are called \"{name}\""),
            Self::CurrentLayerMissing(id) => write!(f, "the current layer {} does not exist", id.0),
            Self::DuplicateObjectId(id) => write!(f, "object id {} is used twice", id.0),
            Self::MissingLayer(object, layer) => {
                write!(f, "object {} is on missing layer {}", object.0, layer.0)
            }
            Self::CounterBehind => write!(f, "an id counter is behind the ids in use"),
            Self::BadLayout(name) => write!(f, "layout \"{name}\" is not valid"),
            Self::MissingLayoutOf(id) => {
                write!(f, "object {} is on a layout that does not exist", id.0)
            }
            Self::BadTextStyle(name) => write!(f, "text style \"{name}\" is not valid"),
            Self::BadDimStyle(name) => write!(f, "dimension style \"{name}\" is not valid"),
            Self::BadMLeaderStyle(name) => {
                write!(f, "multileader style \"{name}\" is not valid")
            }
            Self::CurrentMLeaderStyleMissing(id) => {
                write!(f, "the current multileader style {} does not exist", id.0)
            }
            Self::CurrentTextStyleMissing(id) => {
                write!(f, "the current text style {} does not exist", id.0)
            }
            Self::MissingTextStyle(object, style) => {
                write!(f, "object {} uses missing text style {}", object.0, style.0)
            }
            Self::BadLeader(object, why) => {
                write!(f, "object {} is not a valid leader: {why}", object.0)
            }
            Self::BadMLeader(object, why) => {
                write!(f, "object {} is not a valid multileader: {why}", object.0)
            }
            Self::BadMText(object, why) => {
                write!(f, "object {} is not valid text: {why}", object.0)
            }
            Self::BadEllipse(object, why) => {
                write!(f, "object {} is not a valid ellipse: {why}", object.0)
            }
            Self::BadHatch(object, why) => {
                write!(f, "object {} is not a valid hatch: {why}", object.0)
            }
            Self::BadSpline(object, why) => {
                write!(f, "object {} is not a valid spline: {why}", object.0)
            }
            Self::BadPolyline(object, why) => {
                write!(f, "object {} is not a valid polyline: {why}", object.0)
            }
            Self::BadBlock(name) => write!(f, "block \"{name}\" is not valid"),
            Self::BadInsert(object) => {
                write!(f, "object {} is not a valid block insert", object.0)
            }
        }
    }
}

impl std::error::Error for DocumentError {}

impl Document {
    /// Whether the drawing could exist: unique ids, layer 0 present, every
    /// object on a real layer, and counters ahead of every id in use.
    pub fn validate(&self) -> Result<(), DocumentError> {
        let layers = self.layers();
        if layers.first().map(|l| l.id) != Some(LayerId::DEFAULT) {
            return Err(DocumentError::NoDefaultLayer);
        }
        let mut layer_ids = HashSet::new();
        let mut names = HashSet::new();
        for layer in layers {
            if !layer_ids.insert(layer.id) {
                return Err(DocumentError::DuplicateLayerId(layer.id));
            }
            if !names.insert(layer.name.to_lowercase()) {
                return Err(DocumentError::DuplicateLayerName(layer.name.clone()));
            }
        }
        if !layer_ids.contains(&self.current_layer()) {
            return Err(DocumentError::CurrentLayerMissing(self.current_layer()));
        }
        let mut object_ids = HashSet::new();
        for object in self.objects() {
            if !object_ids.insert(object.id) {
                return Err(DocumentError::DuplicateObjectId(object.id));
            }
            if !layer_ids.contains(&object.attrs.layer) {
                return Err(DocumentError::MissingLayer(object.id, object.attrs.layer));
            }
            match &object.entity {
                Entity::Ellipse(e) => e
                    .validate()
                    .map_err(|why| DocumentError::BadEllipse(object.id, why))?,
                Entity::Spline(s) => s
                    .validate()
                    .map_err(|why| DocumentError::BadSpline(object.id, why))?,
                Entity::Polyline {
                    points,
                    bulges,
                    widths,
                    ..
                } => crate::polyline::check(points, bulges, widths)
                    .map_err(|why| DocumentError::BadPolyline(object.id, why))?,
                Entity::Hatch(h) => h
                    .validate()
                    .map_err(|why| DocumentError::BadHatch(object.id, why.to_string()))?,
                _ => {}
            }
        }
        let (next_object, next_layer) = self.next_ids();
        let behind = self.objects().iter().any(|o| o.id.0 >= next_object)
            || layers.iter().any(|l| l.id.0 >= next_layer);
        let linetypes = self.linetypes();
        if behind
            || linetypes
                .records()
                .iter()
                .any(|r| r.id.0 >= linetypes.next_id())
        {
            return Err(DocumentError::CounterBehind);
        }
        self.validate_text()?;
        self.validate_mleader_styles()?;
        self.validate_mleaders()?;
        self.validate_leaders()?;
        self.validate_dim_styles()?;
        self.validate_blocks()?;
        self.validate_layouts()
    }

    fn validate_dim_styles(&self) -> Result<(), DocumentError> {
        let styles = self.dim_styles();
        if styles
            .first()
            .is_none_or(|s| !s.name.eq_ignore_ascii_case("Standard"))
        {
            return Err(DocumentError::BadDimStyle("Standard".into()));
        }
        let mut names = HashSet::new();
        for style in styles {
            if style.name.trim().is_empty() || !names.insert(style.name.to_ascii_lowercase()) {
                return Err(DocumentError::BadDimStyle(style.name.clone()));
            }
        }
        if !names.contains(&self.dim_style().name.to_ascii_lowercase()) {
            return Err(DocumentError::BadDimStyle(self.dim_style().name.clone()));
        }
        Ok(())
    }

    fn validate_text(&self) -> Result<(), DocumentError> {
        let styles = self.text_styles();
        if styles.first().map(|s| s.id) != Some(TextStyleId::STANDARD) {
            return Err(DocumentError::BadTextStyle("Standard".into()));
        }
        let mut ids = HashSet::new();
        let mut names = HashSet::new();
        for style in styles {
            let unique = ids.insert(style.id) && names.insert(style.name.to_ascii_lowercase());
            if !unique || !style.is_valid() {
                return Err(DocumentError::BadTextStyle(style.name.clone()));
            }
        }
        if !ids.contains(&self.current_text_style()) {
            return Err(DocumentError::CurrentTextStyleMissing(
                self.current_text_style(),
            ));
        }
        if styles.iter().any(|s| s.id.0 >= self.next_text_style_id()) {
            return Err(DocumentError::CounterBehind);
        }
        for object in self.objects() {
            let Entity::MText(text) = &object.entity else {
                continue;
            };
            if !ids.contains(&text.style) {
                return Err(DocumentError::MissingTextStyle(object.id, text.style));
            }
            if let Err(e) = text.validate() {
                return Err(DocumentError::BadMText(object.id, e.to_string()));
            }
        }
        Ok(())
    }

    fn validate_layouts(&self) -> Result<(), DocumentError> {
        let orphan = self
            .objects()
            .iter()
            .find(|o| o.layout.is_some_and(|l| self.layout(l).is_none()));
        if let Some(object) = orphan {
            return Err(DocumentError::MissingLayoutOf(object.id));
        }
        let (next_layout, next_viewport) = self.next_layout_ids();
        let mut ids = HashSet::new();
        let mut names = HashSet::new();
        for layout in self.layouts() {
            let unique = ids.insert(layout.id) && names.insert(layout.name.to_lowercase());
            if !unique || layout.name.trim().is_empty() || !layout.is_valid() {
                return Err(DocumentError::BadLayout(layout.name.clone()));
            }
            let behind = layout.id.0 >= next_layout
                || layout.viewports.iter().any(|v| v.id.0 >= next_viewport);
            if behind {
                return Err(DocumentError::CounterBehind);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Attributes, Entity, Layer, Object, Point};

    fn line() -> Entity {
        Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(1.0, 1.0),
        }
    }

    #[test]
    fn a_normal_drawing_is_valid() {
        let mut document = Document::default();
        document.add(line());
        let id = document.allocate_layer_id();
        document.push_layer(Layer::new(id, "Walls"));
        document.add_with(line(), Attributes::on_layer(id));
        assert_eq!(document.validate(), Ok(()));
    }

    #[test]
    fn duplicate_object_ids_are_found() {
        let mut document = Document::default();
        let id = document.add(line());
        document.push(Object::new(
            id,
            line(),
            Attributes::on_layer(LayerId::DEFAULT),
            None,
        ));
        assert_eq!(
            document.validate(),
            Err(DocumentError::DuplicateObjectId(id))
        );
    }

    #[test]
    fn a_spline_that_could_not_exist_is_found() {
        let mut document = Document::default();
        let points = vec![
            Point::new(0.0, 0.0),
            Point::new(1.0, 1.0),
            Point::new(2.0, 0.0),
        ];
        let good = crate::Spline::from_control_points(2, points).unwrap();
        document.add(Entity::Spline(Box::new(good.clone())));
        assert_eq!(document.validate(), Ok(()));
        let mut bad = good;
        bad.knots.reverse();
        let id = document.add(Entity::Spline(Box::new(bad)));
        assert_eq!(
            document.validate(),
            Err(DocumentError::BadSpline(
                id,
                crate::SplineError::KnotsDecrease
            ))
        );
    }

    #[test]
    fn a_polyline_that_could_not_exist_is_found() {
        let mut document = Document::default();
        let polyline = |bulges: Vec<f64>| Entity::Polyline {
            points: vec![Point::new(0.0, 0.0), Point::new(1.0, 0.0)],
            closed: false,
            bulges,
            widths: Vec::new(),
        };
        document.add(polyline(vec![0.0, 0.5]));
        assert_eq!(document.validate(), Ok(()));
        let id = document.add(polyline(vec![f64::NAN, 0.0]));
        assert_eq!(
            document.validate(),
            Err(DocumentError::BadPolyline(id, PolylineError::BadBulge(0)))
        );
    }

    #[test]
    fn an_ellipse_that_could_not_exist_is_found() {
        let mut document = Document::default();
        let good = crate::Ellipse::full(Point::new(0.0, 0.0), Point::new(2.0, 0.0), 0.5).unwrap();
        document.add(Entity::Ellipse(good));
        assert_eq!(document.validate(), Ok(()));
        let mut flat = good;
        flat.radius_ratio = 0.0;
        let id = document.add(Entity::Ellipse(flat));
        assert_eq!(
            document.validate(),
            Err(DocumentError::BadEllipse(
                id,
                crate::EllipseError::RatioOutOfRange
            ))
        );
    }

    #[test]
    fn an_object_on_a_missing_layer_is_found() {
        let mut document = Document::default();
        let id = document.add_with(line(), Attributes::on_layer(LayerId(9)));
        assert_eq!(
            document.validate(),
            Err(DocumentError::MissingLayer(id, LayerId(9)))
        );
    }

    #[test]
    fn duplicate_layers_are_found() {
        let mut document = Document::default();
        let id = document.allocate_layer_id();
        document.push_layer(Layer::new(id, "A"));
        document.push_layer(Layer::new(id, "B"));
        assert_eq!(
            document.validate(),
            Err(DocumentError::DuplicateLayerId(id))
        );

        let mut document = Document::default();
        let (a, b) = (document.allocate_layer_id(), document.allocate_layer_id());
        document.push_layer(Layer::new(a, "Walls"));
        document.push_layer(Layer::new(b, "WALLS"));
        assert!(matches!(
            document.validate(),
            Err(DocumentError::DuplicateLayerName(_))
        ));
    }

    #[test]
    fn text_styles_and_text_are_checked() {
        use crate::{MText, TextStyle};

        let mut document = Document::default();
        document.add(Entity::MText(Box::new(MText::new(
            Point::new(0.0, 0.0),
            2.5,
            "Hello",
            TextStyleId::STANDARD,
        ))));
        assert_eq!(document.validate(), Ok(()));

        let notes = document.allocate_text_style_id();
        document.push_text_style(TextStyle::new(notes, "Notes"));
        assert_eq!(document.validate(), Ok(()));
        document.push_text_style(TextStyle::new(TextStyleId(9), "NOTES"));
        assert!(matches!(
            document.validate(),
            Err(DocumentError::BadTextStyle(_))
        ));

        let mut document = Document::default();
        let id = document.add(Entity::MText(Box::new(MText::new(
            Point::new(0.0, 0.0),
            2.5,
            "Hello",
            TextStyleId(4),
        ))));
        assert_eq!(
            document.validate(),
            Err(DocumentError::MissingTextStyle(id, TextStyleId(4)))
        );

        let mut document = Document::default();
        let id = document.add(Entity::MText(Box::new(MText::new(
            Point::new(0.0, 0.0),
            -1.0,
            "Hello",
            TextStyleId::STANDARD,
        ))));
        assert!(matches!(
            document.validate(),
            Err(DocumentError::BadMText(bad, _)) if bad == id
        ));
    }
}
