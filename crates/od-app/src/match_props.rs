//! MATCHPROP: copy one object's properties onto others, as AutoCAD's Match
//! Properties. What a target becomes is [`matched`], kept pure so it is
//! tested without the command.

use od_commands::{Answer, Cancelled, Editor, ModifyAttributes, ModifyEntities, Options, Picked};
use od_core::{Attributes, Document, Entity, EntityId, MText, Object};

use crate::layers::has_outline;

/// The properties matched, worded as AutoCAD's "Current active settings".
pub const SETTINGS: &str =
    "Color Layer Ltype Ltscale Lineweight Dim Text Hatch Polyline Multileader";

/// `target` as it would be after matching `source`: its attributes, and its
/// entity when one of the special properties changes it. `None` when nothing
/// would change.
///
/// Every object takes the colour, layer, linetype, linetype scale and
/// lineweight. Between objects of the same kind: a dimension takes the
/// dimension style (with its overrides), text the text style and height, a
/// hatch the fill and island style, a multileader the style, and a polyline
/// or rectangle the linetype generation and a polyline the global width.
pub fn matched(
    document: &Document,
    source: &Object,
    target: &Object,
) -> Option<(Attributes, Option<Entity>)> {
    let mut attrs = Attributes {
        layer: source.attrs.layer,
        color: source.attrs.color.clone(),
        linetype: source.attrs.linetype.clone(),
        lineweight: source.attrs.lineweight.clone(),
        linetype_scale: source.attrs.linetype_scale,
        ..target.attrs.clone()
    };
    if has_outline(&source.entity) && has_outline(&target.entity) {
        attrs.continuous_generation = source.attrs.continuous_generation;
    }
    let entity = special(document, &source.entity, &target.entity);
    (attrs != target.attrs || entity.is_some()).then_some((attrs, entity))
}

/// The target changed by the source's special properties, if they apply and
/// change it.
fn special(document: &Document, source: &Entity, target: &Entity) -> Option<Entity> {
    let next = match (source, target) {
        (Entity::Dimension(s), Entity::Dimension(t)) => {
            let mut d = (**t).clone();
            d.style = s.style.clone();
            Entity::Dimension(Box::new(d))
        }
        (Entity::MText(s), Entity::MText(t)) => Entity::MText(Box::new(MText {
            style: s.style,
            nominal_height: s.nominal_height,
            ..(**t).clone()
        })),
        (Entity::Hatch(s), Entity::Hatch(t)) => {
            let mut h = (**t).clone();
            h.fill = s.fill.clone();
            h.style = s.style;
            Entity::Hatch(Box::new(h))
        }
        (Entity::MLeader(s), Entity::MLeader(t)) => {
            let style = document.mleader_style_named(&s.style_name)?;
            Entity::MLeader(Box::new(t.restyled(style)))
        }
        (Entity::Polyline { widths: from, .. }, Entity::Polyline { .. }) => {
            let width = global_width(from)?;
            let mut p = target.clone();
            if let Entity::Polyline { points, widths, .. } = &mut p {
                *widths = match width {
                    0.0 => Vec::new(),
                    w => vec![(w, w); points.len()],
                };
            }
            p
        }
        _ => return None,
    };
    (next != *target).then_some(next)
}

/// A polyline's one width (0 for none), or `None` when its segments differ.
fn global_width(widths: &[(f64, f64)]) -> Option<f64> {
    let first = widths.first().map_or(0.0, |w| w.0);
    widths
        .iter()
        .all(|&(a, b)| a == first && b == first)
        .then_some(first)
}

/// MATCHPROP: pick the source object (or have one selected), then each
/// object picked takes its properties, one undo step each, until Enter.
pub fn match_properties(ed: &mut Editor) -> Result<(), Cancelled> {
    let source = match ed.pickset() {
        [one] => *one,
        _ => {
            let options = Options::<Picked>::new("Select source object")
                .pick_locked()
                .allow_empty();
            match ed.ask_object(options)? {
                Answer::Value(Picked { id, .. }) => id,
                _ => return Ok(()),
            }
        }
    };
    ed.say(format!("Current active settings: {SETTINGS}"));
    loop {
        let options = Options::<Picked>::new("Select destination object(s)").allow_empty();
        let Answer::Value(Picked { id, .. }) = ed.ask_object(options)? else {
            return Ok(());
        };
        if id != source {
            apply(ed, source, id)?;
        }
    }
}

/// Make `target` match `source`, as one undo step.
fn apply(ed: &Editor, source: EntityId, target: EntityId) -> Result<(), Cancelled> {
    ed.transaction("Match Properties", move |tx| {
        let doc = tx.document();
        let (Some(from), Some(to)) = (doc.object(source), doc.object(target)) else {
            return Ok(());
        };
        let Some((attrs, entity)) = matched(doc, from, to) else {
            return Ok(());
        };
        tx.perform(ModifyAttributes::new(
            "Match Properties",
            vec![(target, attrs)],
        ));
        if let Some(entity) = entity {
            tx.perform(ModifyEntities::new(
                "Match Properties",
                vec![(target, entity)],
            ));
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use od_commands::ModifyAttributes;
    use od_core::{Attr, Color, DimStyle, Dimension, LayerId, Lineweight, Point};

    use super::*;
    use crate::state::AppState;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn line(y: f64) -> Entity {
        Entity::Line {
            start: p(0.0, y),
            end: p(10.0, y),
        }
    }

    fn object(entity: Entity, attrs: Attributes) -> Object {
        Object::new(EntityId(1), entity, attrs, None)
    }

    fn red_dashed(layer: LayerId) -> Attributes {
        Attributes {
            color: Attr::Set(Color::Rgb(255, 0, 0)),
            linetype: Attr::Set("DASHED".to_owned()),
            lineweight: Attr::Set(Lineweight::STANDARD[3]),
            linetype_scale: 2.0,
            ..Attributes::on_layer(layer)
        }
    }

    #[test]
    fn every_object_takes_the_general_properties() {
        let doc = Document::default();
        let source = object(line(0.0), red_dashed(LayerId(7)));
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 1.0,
        };
        let target = object(circle, Attributes::on_layer(LayerId::DEFAULT));
        let (attrs, entity) = matched(&doc, &source, &target).unwrap();
        assert_eq!(attrs, red_dashed(LayerId(7)));
        assert!(
            entity.is_none(),
            "a line has no special properties for a circle"
        );
        let same = object(line(5.0), red_dashed(LayerId(7)));
        assert!(matched(&doc, &source, &same).is_none(), "nothing to change");
    }

    #[test]
    fn same_kinds_take_their_special_properties() {
        let doc = Document::default();
        let dim = |height: f64| {
            let style = DimStyle {
                text_height: height,
                ..DimStyle::default()
            };
            Entity::Dimension(Box::new(Dimension::aligned(
                p(0.0, 0.0),
                p(10.0, 0.0),
                p(0.0, 5.0),
                &style,
            )))
        };
        let on_zero = Attributes::on_layer(LayerId::DEFAULT);
        let source = object(dim(5.0), on_zero.clone());
        let target = object(dim(2.5), on_zero.clone());
        let (_, entity) = matched(&doc, &source, &target).unwrap();
        let Some(Entity::Dimension(d)) = entity else {
            panic!("the dimension changes");
        };
        assert_eq!(d.style.text_height, 5.0);

        let wide = |widths: Vec<(f64, f64)>| Entity::Polyline {
            points: vec![p(0.0, 0.0), p(5.0, 0.0), p(5.0, 5.0)],
            bulges: Vec::new(),
            widths,
            closed: false,
        };
        let source = object(wide(vec![(0.5, 0.5); 3]), on_zero.clone());
        let target = object(wide(Vec::new()), on_zero.clone());
        let (_, entity) = matched(&doc, &source, &target).unwrap();
        assert_eq!(entity, Some(wide(vec![(0.5, 0.5); 3])));
        let tapered = object(wide(vec![(0.0, 1.0); 3]), on_zero);
        assert!(matched(&doc, &tapered, &target).is_none(), "no one width");
    }

    #[test]
    fn the_command_matches_each_picked_object_in_its_own_step() {
        let mut state = AppState::new();
        for y in [0.0, 5.0, 10.0] {
            state.add_entity(line(y));
        }
        let ids: Vec<EntityId> = state
            .session()
            .document()
            .objects()
            .iter()
            .map(|o| o.id)
            .collect();
        state.perform(ModifyAttributes::new(
            "Red",
            vec![(ids[0], red_dashed(LayerId::DEFAULT))],
        ));
        let attrs =
            |state: &AppState, id| state.session().document().object(id).unwrap().attrs.clone();

        state.click_select(Some(ids[0]), false);
        state.start_command("MATCHPROP", match_properties);
        state.settle_command();
        let line = state.cmdline.log.last().cloned().unwrap_or_default();
        assert!(
            line.starts_with("Current active settings: Color Layer"),
            "{line}"
        );
        for &id in &ids[1..] {
            state.answer_object(p(1.0, 0.0), Some(id), false);
            state.settle_command();
        }
        state.answer_enter();
        state.settle_command();
        assert!(!state.command_running());
        assert_eq!(attrs(&state, ids[2]), red_dashed(LayerId::DEFAULT));
        state.undo();
        assert_eq!(
            attrs(&state, ids[2]),
            Attributes::on_layer(LayerId::DEFAULT)
        );
        assert_eq!(attrs(&state, ids[1]), red_dashed(LayerId::DEFAULT));
    }
}
