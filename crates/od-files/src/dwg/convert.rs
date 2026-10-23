//! From the flat drawing `od-dwg` reads to an `od_core::Document`.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};

use od_core::aci::aci_rgb;
use od_core::{
    arc, AngleFormat, Attr, Attributes, Color, Document, Ellipse, Entity, InsertUnits,
    KnotParameterization, Layer, LayerId, LengthFormat, Lineweight, Point, Spline, StrokeElement,
    Units, LINETYPES,
};
use od_dwg::{
    Drawing, DwgColor, DwgEntity, DwgHandle, DwgHeader, DwgLinetype, DwgLineweight, Shape,
};

use super::text::{self, Styles};
use crate::block_import::{ImportedBlocks, Item, PendingBlock, PendingInsert};
use crate::native::spline::MAX_FILE_SPLINE_POINTS;
use crate::FileError;

/// The unit settings of a drawing's header.
fn units(header: &DwgHeader) -> Units {
    let defaults = Units::default();
    let precision = |v: u16| u8::try_from(v.min(8)).unwrap_or(8);
    Units {
        length: LengthFormat::from_code(i64::from(header.linear_units)).unwrap_or(defaults.length),
        length_precision: precision(header.linear_precision),
        angle: AngleFormat::from_code(i64::from(header.angular_units)).unwrap_or(defaults.angle),
        angle_precision: precision(header.angular_precision),
        clockwise: header.angles_clockwise,
        base_angle: if header.angle_base.is_finite() {
            header.angle_base
        } else {
            0.0
        },
        insert: InsertUnits::from_code(i64::from(header.insert_units))
            .unwrap_or(InsertUnits::UNITLESS),
    }
    .sanitized()
}

pub(super) fn color(c: DwgColor) -> Color {
    match c {
        // Colour 7 is the foreground: white on a dark canvas, black on a light one.
        DwgColor::Aci(7) | DwgColor::ByLayer => Color::Foreground,
        DwgColor::Aci(i) => {
            let (r, g, b) = aci_rgb(i);
            Color::Rgb(r, g, b)
        }
        DwgColor::True(r, g, b) => Color::Rgb(r, g, b),
    }
}

/// The app's spelling of a linetype name, when it has one of that name.
fn linetype(name: &str) -> String {
    LINETYPES
        .iter()
        .find(|known| known.eq_ignore_ascii_case(name))
        .map_or_else(|| name.to_owned(), |known| (*known).to_owned())
}

fn lineweight(w: DwgLineweight) -> Lineweight {
    match w {
        DwgLineweight::Hundredths(w) => Lineweight(w),
        DwgLineweight::ByLayer | DwgLineweight::Default => Lineweight::DEFAULT,
    }
}

fn point(p: [f64; 2]) -> Point {
    Point::new(p[0], p[1])
}

/// Why a shape the reader passed does not become an object.
enum Refused {
    /// An ellipse the drawing cannot hold (one that turns more than once).
    Ellipse,
    /// A spline whose knots, weights or flags make none, or with more points
    /// than a file may bring.
    Spline,
    /// A spline stored by its fit points alone that is fitted other than as
    /// a cubic without end tangents (end tangents, another degree).
    FitSpline,
    /// Text whose values make none (a box or height the text model refuses).
    Text,
}

/// A spline from its control frame, checked as a file's would be.
fn spline(
    degree: u32,
    control_points: &[[f64; 2]],
    knots: &[f64],
    weights: &[f64],
    (closed, periodic): (bool, bool),
    (knot_tolerance, control_point_tolerance): (f64, f64),
) -> Result<Spline, Refused> {
    if control_points.len() > MAX_FILE_SPLINE_POINTS {
        return Err(Refused::Spline);
    }
    let spline = Spline {
        degree: u8::try_from(degree).map_err(|_| Refused::Spline)?,
        control_points: control_points.iter().map(|&p| point(p)).collect(),
        knots: knots.to_vec(),
        // Weights that are all 1 make an ordinary spline.
        weights: weights.iter().any(|w| *w != 1.0).then(|| weights.to_vec()),
        closed: closed || periodic,
        periodic,
        knot_tolerance,
        control_point_tolerance,
        fit: None,
    };
    spline.validate().map_err(|_| Refused::Spline)?;
    Ok(spline)
}

/// A spline stored by its fit points alone, fitted as AutoCAD does when no
/// end tangents are given: a cubic, natural at its ends or smooth where it
/// closes.
fn fit_spline(shape: &Shape) -> Result<Spline, Refused> {
    let Shape::FitSpline {
        degree,
        points,
        tolerance,
        start_tangent,
        end_tangent,
        knots,
        closed,
    } = shape
    else {
        return Err(Refused::FitSpline);
    };
    let parameterization = match knots {
        None | Some(0) => KnotParameterization::Chord,
        Some(1) => KnotParameterization::SqrtChord,
        Some(2) => KnotParameterization::Uniform,
        Some(_) => return Err(Refused::FitSpline),
    };
    if *degree != 3 || start_tangent.is_some() || end_tangent.is_some() {
        return Err(Refused::FitSpline);
    }
    if points.len() > MAX_FILE_SPLINE_POINTS {
        return Err(Refused::Spline);
    }
    let through: Vec<Point> = points.iter().map(|&p| point(p)).collect();
    let made = if *closed {
        Spline::from_closed_fit_points(through, parameterization)
    } else {
        Spline::from_fit_points(through, parameterization)
    };
    let mut spline = made.map_err(|_| Refused::Spline)?;
    if let Some(fit) = spline.fit.as_mut() {
        fit.tolerance = *tolerance;
    }
    Ok(spline)
}

/// The drawing's object for a shape, or why there is none.
fn item(shape: &Shape, styles: &Styles) -> Result<Item<DwgHandle>, Refused> {
    Ok(Item::Entity(match shape {
        Shape::Line { start, end } => Entity::Line {
            start: point(*start),
            end: point(*end),
        },
        Shape::Point { at } => Entity::Point { at: point(*at) },
        Shape::Circle { center, radius } => Entity::Circle {
            center: point(*center),
            radius: *radius,
        },
        Shape::Arc {
            center,
            radius,
            start,
            end,
        } => Entity::Arc {
            center: point(*center),
            radius: *radius,
            start: arc::normalize(*start),
            end: arc::normalize(*end),
        },
        Shape::Ellipse {
            center,
            major_axis,
            ratio,
            start,
            end,
        } => Entity::Ellipse(
            Ellipse::new(point(*center), point(*major_axis), *ratio, *start, *end)
                .map_err(|_| Refused::Ellipse)?,
        ),
        Shape::Spline {
            degree,
            control_points,
            knots,
            weights,
            closed,
            periodic,
            knot_tolerance,
            control_tolerance,
        } => Entity::Spline(Box::new(spline(
            *degree,
            control_points,
            knots,
            weights,
            (*closed, *periodic),
            (*knot_tolerance, *control_tolerance),
        )?)),
        Shape::FitSpline { .. } => Entity::Spline(Box::new(fit_spline(shape)?)),
        Shape::Polyline {
            points,
            closed,
            bulges,
            widths,
        } => od_core::closed_pair::expanded(Entity::Polyline {
            points: points.iter().map(|&p| point(p)).collect(),
            closed: *closed,
            bulges: bulges.clone(),
            widths: widths.iter().map(|w| (w[0], w[1])).collect(),
        }),
        Shape::Text { .. } | Shape::MText { .. } => {
            text::entity(shape, styles).ok_or(Refused::Text)?
        }
        Shape::Insert {
            block,
            insertion,
            scale,
            rotation,
        } => {
            return Ok(Item::Insert(PendingInsert {
                block: *block,
                insertion: point(*insertion),
                scale_x: scale[0],
                scale_y: scale[1],
                rotation: *rotation,
            }));
        }
    }))
}

/// What the conversion noted that the reader's report does not cover.
#[derive(Default)]
pub(super) struct Notes {
    /// Layers renamed because another had the same name (compared ignoring case).
    pub renamed_layers: usize,
    /// What reading the linetype definitions has to report.
    pub linetypes: crate::linetype_import::Imported,
    /// What building the blocks left out, worded.
    pub blocks: Vec<String>,
    /// Ellipses whose values make none (an arc turning more than once).
    pub bad_ellipses: usize,
    /// Splines whose values make none.
    pub bad_splines: usize,
    /// Splines stored by their fit points alone that could not be fitted.
    pub fit_splines: usize,
    /// Text whose values make none.
    pub bad_text: usize,
}

impl Notes {
    fn refused(&mut self, why: Refused) {
        match why {
            Refused::Ellipse => self.bad_ellipses += 1,
            Refused::Spline => self.bad_splines += 1,
            Refused::FitSpline => self.fit_splines += 1,
            Refused::Text => self.bad_text += 1,
        }
    }
}

/// An entity's layer and properties in the document.
fn attributes(e: &DwgEntity, ids: &HashMap<DwgHandle, LayerId>) -> Result<Attributes, FileError> {
    let layer = *ids
        .get(&e.layer)
        .ok_or_else(|| FileError::Invalid("an object is on a layer that is not there".into()))?;
    let linetype = match &e.linetype {
        DwgLinetype::ByLayer => Attr::ByLayer,
        DwgLinetype::Named(name) => Attr::Set(linetype(name)),
    };
    Ok(Attributes {
        layer,
        color: match e.color {
            DwgColor::ByLayer => Attr::ByLayer,
            c => Attr::Set(color(c)),
        },
        linetype,
        lineweight: match e.lineweight {
            DwgLineweight::ByLayer => Attr::ByLayer,
            w => Attr::Set(lineweight(w)),
        },
        linetype_scale: Some(e.linetype_scale)
            .filter(|s| s.is_finite() && *s > 0.0)
            .unwrap_or(1.0),
        // The generation flag of a polyline is not decoded yet.
        continuous_generation: false,
    })
}

/// Builds the document. The layer called "0" becomes the document's own layer
/// 0; the others get new ids, never derived from DWG handles. Blocks are
/// built before the model-space objects that insert them.
pub(super) fn to_document(
    drawing: &Drawing,
    cancelled: &AtomicBool,
) -> Result<(Document, Notes), FileError> {
    let mut document = Document::default();
    document.begin_bulk_load();
    document.set_units(units(&drawing.header));
    let mut notes = Notes::default();
    let mut ids: HashMap<DwgHandle, LayerId> = HashMap::new();
    let mut taken: HashSet<String> = HashSet::new();
    document.set_linetype_scale(drawing.header.linetype_scale);
    for def in &drawing.linetypes {
        // A pattern that did not decode leaves the name for `resolve_used`.
        let Some(pattern) = &def.pattern else {
            continue;
        };
        let elements = pattern
            .iter()
            .filter_map(|v| StrokeElement::from_dxf(*v))
            .collect();
        notes.linetypes.define(
            &mut document,
            &def.name,
            &def.description,
            elements,
            def.complex,
        );
    }

    for l in &drawing.layers {
        let is_default = l.name == "0";
        let id = if is_default {
            LayerId::DEFAULT
        } else {
            document.allocate_layer_id()
        };
        let mut name = l.name.clone();
        let mut suffix = 1;
        while !taken.insert(name.to_lowercase()) {
            suffix += 1;
            name = format!("{}_{suffix}", l.name);
            notes.renamed_layers += usize::from(suffix == 2);
        }
        let linetype_name = l
            .linetype
            .as_deref()
            .map_or_else(|| LINETYPES[0].to_owned(), linetype);
        let layer = Layer {
            id,
            name,
            color: color(l.color),
            linetype: linetype_name,
            lineweight: lineweight(l.lineweight),
            on: l.on,
            frozen: l.frozen,
            locked: l.locked,
        };
        if is_default {
            document.replace_layer(layer);
        } else {
            document.push_layer(layer);
        }
        ids.insert(l.handle, id);
    }
    if let Some(&current) = drawing
        .header
        .current_layer
        .as_ref()
        .and_then(|h| ids.get(h))
    {
        document.set_current_layer(current);
    }

    let styles = text::import_styles(&mut document, &drawing.text_styles);
    let mut pending = Vec::with_capacity(drawing.blocks.len());
    for b in &drawing.blocks {
        let mut items = Vec::with_capacity(b.entities.len());
        for e in &b.entities {
            let attrs = attributes(e, &ids)?;
            match item(&e.shape, &styles) {
                Ok(made) => items.push((made, attrs)),
                Err(why) => notes.refused(why),
            }
        }
        pending.push(PendingBlock {
            key: b.handle,
            name: b.name.clone(),
            base: point(b.base),
            description: b.description.clone(),
            items,
        });
    }
    let mut blocks = ImportedBlocks::build(&mut document, pending);

    for (n, e) in drawing.entities.iter().enumerate() {
        if n % 1024 == 0 && cancelled.load(Ordering::Relaxed) {
            return Err(FileError::Cancelled);
        }
        let attrs = attributes(e, &ids)?;
        let entity = match item(&e.shape, &styles) {
            Ok(Item::Entity(entity)) => Some(entity),
            Ok(Item::Insert(insert)) => blocks.insert(insert),
            Err(why) => {
                notes.refused(why);
                None
            }
        };
        if let Some(entity) = entity {
            document.add_with(entity, attrs);
        }
    }
    notes.linetypes.resolve_used(&mut document);
    notes.blocks = blocks.warnings();
    document
        .validate()
        .map_err(|e| FileError::Invalid(e.to_string()))?;
    document.finish_bulk_load();
    Ok((document, notes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(knots: Vec<f64>, weights: Vec<f64>) -> Shape {
        Shape::Spline {
            degree: 2,
            control_points: vec![[1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            knots,
            weights,
            closed: false,
            periodic: false,
            knot_tolerance: 1e-9,
            control_tolerance: 1e-10,
        }
    }

    #[test]
    fn a_control_frame_becomes_a_spline_with_its_values() {
        let quarter = frame(vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0], vec![1.0, 0.5, 1.0]);
        let Ok(Item::Entity(Entity::Spline(s))) = item(&quarter, &Styles::default()) else {
            panic!("a spline")
        };
        assert_eq!(s.weights, Some(vec![1.0, 0.5, 1.0]));
        assert_eq!((s.knot_tolerance, s.control_point_tolerance), (1e-9, 1e-10));
        assert_eq!(s.end_point(), Point::new(0.0, 1.0));
        // Weights of 1 make an ordinary spline.
        let plain = frame(vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0], vec![1.0; 3]);
        let Ok(Item::Entity(Entity::Spline(s))) = item(&plain, &Styles::default()) else {
            panic!("a spline")
        };
        assert_eq!(s.weights, None);
    }

    #[test]
    fn a_spline_that_cannot_exist_and_a_fit_one_with_tangents_are_refused() {
        for bad in [
            frame(vec![0.0, 0.0, 1.0, 1.0], Vec::new()),
            frame(vec![1.0, 0.0, 0.0, 1.0, 1.0, 1.0], Vec::new()),
            frame(vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0], vec![1.0, -1.0, 1.0]),
        ] {
            assert!(matches!(
                item(&bad, &Styles::default()),
                Err(Refused::Spline)
            ));
        }
        let fitted = |start_tangent, knots| Shape::FitSpline {
            degree: 3,
            points: vec![[0.0, 0.0], [1.0, 1.0], [2.0, 0.0]],
            tolerance: 0.5,
            start_tangent,
            end_tangent: None,
            knots,
            closed: false,
        };
        assert!(matches!(
            item(&fitted(Some([1.0, 0.0]), Some(0)), &Styles::default()),
            Err(Refused::FitSpline)
        ));
        assert!(matches!(
            item(&fitted(None, Some(7)), &Styles::default()),
            Err(Refused::FitSpline)
        ));
    }

    #[test]
    fn a_closed_fit_drops_its_repeated_first_point_and_closes_smoothly() {
        let shape = Shape::FitSpline {
            degree: 3,
            points: vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0], [1e-15, 0.0]],
            tolerance: 0.0,
            start_tangent: None,
            end_tangent: None,
            knots: Some(2),
            closed: true,
        };
        let Ok(Item::Entity(Entity::Spline(s))) = item(&shape, &Styles::default()) else {
            panic!("a spline")
        };
        assert!(s.closed);
        assert_eq!(s.fit.as_ref().map(|f| f.points.len()), Some(4));
        assert_eq!(s.control_points.len(), 7);
    }

    #[test]
    fn a_spline_of_fit_points_alone_is_fitted_through_them() {
        let shape = Shape::FitSpline {
            degree: 3,
            points: vec![[0.0, 0.0], [1.0, 1.0], [2.0, 0.0]],
            tolerance: 0.5,
            start_tangent: None,
            end_tangent: None,
            knots: Some(0),
            closed: false,
        };
        let Ok(Item::Entity(Entity::Spline(s))) = item(&shape, &Styles::default()) else {
            panic!("a spline")
        };
        assert!(s.distance_to(Point::new(1.0, 1.0)) < 1e-9);
        assert_eq!(s.fit.as_ref().map(|f| f.tolerance), Some(0.5));
    }
}
