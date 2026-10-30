//! `entities.construct`: exact drafting operations on shapes, as one undo
//! step. The geometry is the program's own (the same functions MIRROR, ARRAY,
//! OFFSET and POLYGON use), so the caller never works out a coordinate; it
//! says what to do and to what.

use std::collections::HashMap;

use od_api::construct_ops::{ConstructArgs, PolygonFit, Step, STEPS_MAX};
use od_api::convert::{is_writable, unsupported};
use od_api::limits::BATCH_OPERATIONS_MAX;
use od_api::{ApiError, ErrorCode};
use od_commands::{AddCopies, ModifyEntities, Transaction};
use od_core::{array, Attributes, Entity, EntityId, LayoutId, Point};

mod shapes;

use super::construct_support::{name_ok, number, point, refuse, Source};
use super::guard::Caller;
use super::mutate::{run_edit, Edit};
use super::read::parse;
use crate::state::AppState;

pub fn construct(
    state: &mut AppState,
    instance: &str,
    caller: &Caller,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, ApiError> {
    let args: ConstructArgs = parse(arguments)?;
    if args.steps.is_empty() || args.steps.len() > STEPS_MAX {
        return Err(refuse(
            ErrorCode::InvalidArguments,
            format!("A request has from 1 to {STEPS_MAX} steps."),
        ));
    }
    run_edit(
        state,
        instance,
        caller,
        Edit {
            document_id: &args.document_id,
            expected_revision: args.expected_revision.0,
            label: args.label.as_deref(),
        },
        |_| Ok(()),
        |tx| {
            let mut run = Run::default();
            for (index, step) in args.steps.iter().enumerate() {
                run.step(tx, step).map_err(|e| e.detail("step", index))?;
            }
            Ok(run.made)
        },
    )
}

/// What the steps so far have made, by name.
#[derive(Default)]
struct Run {
    made: HashMap<String, EntityId>,
}

impl Run {
    fn step(&mut self, tx: &mut Transaction, step: &Step) -> Result<(), ApiError> {
        match step {
            Step::Polygon {
                client_ref,
                center,
                sides,
                radius,
                fit,
                rotation,
                layer_id,
            } => self.polygon(
                tx,
                client_ref,
                point("The centre", center)?,
                *sides,
                *radius,
                fit.unwrap_or(PolygonFit::Inscribed),
                number("rotation", rotation.unwrap_or(0.0))?,
                layer_id,
            ),
            Step::Line {
                client_ref,
                start,
                end,
                layer_id,
            } => self.line(
                tx,
                client_ref,
                point("start", start)?,
                point("end", end)?,
                layer_id,
            ),
            Step::Move {
                targets,
                dx,
                dy,
                copy_as,
            } => {
                let (dx, dy) = (number("dx", *dx)?, number("dy", *dy)?);
                self.each(tx, targets, copy_as, |e| Ok(e.translated(dx, dy)))
            }
            Step::Rotate {
                targets,
                center,
                angle,
                copy_as,
            } => {
                let center = point("The centre", center)?;
                let angle = number("angle", *angle)?;
                self.each(tx, targets, copy_as, |e| Ok(e.rotated(center, angle)))
            }
            Step::Scale {
                targets,
                base,
                factor,
                copy_as,
            } => {
                let base = point("The base point", base)?;
                let factor = number("factor", *factor)?;
                if factor <= 0.0 {
                    return Err(refuse(
                        ErrorCode::InvalidArguments,
                        "factor must be above 0.",
                    ));
                }
                self.each(tx, targets, copy_as, |e| match e.scale_refusal() {
                    Some(why) => Err(refuse(ErrorCode::InvalidGeometry, why)),
                    None => Ok(e.scaled(base, factor)),
                })
            }
            Step::Mirror {
                targets,
                axis_start,
                axis_end,
                copy_as,
            } => {
                let (a, b) = (
                    point("axis_start", axis_start)?,
                    point("axis_end", axis_end)?,
                );
                if (a.x - b.x).hypot(a.y - b.y) < 1e-9 {
                    return Err(refuse(
                        ErrorCode::InvalidArguments,
                        "The mirror axis needs two different points.",
                    ));
                }
                self.each(tx, targets, copy_as, |e| match e.mirror_refusal() {
                    Some(why) => Err(refuse(ErrorCode::InvalidGeometry, why)),
                    None => Ok(e.mirrored(a, b)),
                })
            }
            Step::RectArray {
                targets,
                columns,
                rows,
                column_spacing,
                row_spacing,
                ref_prefix,
            } => {
                let placements = array::rectangular(
                    *columns as usize,
                    *rows as usize,
                    number("column_spacing", *column_spacing)?,
                    number("row_spacing", *row_spacing)?,
                )
                .ok_or_else(|| {
                    refuse(
                        ErrorCode::InvalidArguments,
                        "columns and rows must be 1 or more, and the array not enormous.",
                    )
                })?;
                self.place(tx, targets, &placements, ref_prefix)
            }
            Step::PolarArray {
                targets,
                center,
                count,
                fill_angle,
                rotate_items,
                ref_prefix,
            } => {
                let center = point("The centre", center)?;
                let sources = self.sources(tx, targets)?;
                let base = sources[0].entity.bounds();
                let base = Point::new(
                    (base.min.x + base.max.x) / 2.0,
                    (base.min.y + base.max.y) / 2.0,
                );
                let placements = array::polar(
                    center,
                    base,
                    *count as usize,
                    number("fill_angle", fill_angle.unwrap_or(std::f64::consts::TAU))?,
                    rotate_items.unwrap_or(true),
                )
                .ok_or_else(|| {
                    refuse(
                        ErrorCode::InvalidArguments,
                        "count must be 1 or more and fill_angle at most a full turn.",
                    )
                })?;
                self.place_sources(tx, sources, &placements, ref_prefix)
            }
            Step::TangentArc {
                target,
                at,
                radius,
                sweep,
                client_ref,
                layer_id,
            } => self.tangent_arc(tx, target, *at, *radius, *sweep, client_ref, layer_id),
            Step::Offset {
                target,
                distance,
                toward,
                client_ref,
                layer_id,
            } => {
                let source = self.sources(tx, std::slice::from_ref(target))?.remove(0);
                let made = source
                    .entity
                    .offset(number("distance", *distance)?, point("toward", toward)?)
                    .ok_or_else(|| {
                        refuse(
                            ErrorCode::InvalidGeometry,
                            "That shape cannot be offset that way: the distance must be above 0 and \
                             the side must not be on the outline, and an inward offset must fit.",
                        )
                    })?;
                let attrs = self.on_layer(tx, source.attrs, layer_id)?;
                self.add(tx, client_ref, made, attrs, source.layout)
            }
        }
    }

    /// The entity a target names: an id in the drawing (digits) or a shape
    /// made earlier in this request.
    fn resolve(&self, target: &str) -> Result<EntityId, ApiError> {
        if target.bytes().all(|b| b.is_ascii_digit()) && !target.is_empty() {
            return target.parse().map(EntityId).map_err(|_| {
                refuse(
                    ErrorCode::InvalidArguments,
                    "That entity id is not a number.",
                )
            });
        }
        self.made.get(target).copied().ok_or_else(|| {
            refuse(
                ErrorCode::InvalidReference,
                format!("{target:?} is not made earlier in this request."),
            )
        })
    }

    /// The objects `targets` name, each of a kind this tool may change.
    fn sources(&self, tx: &Transaction, targets: &[String]) -> Result<Vec<Source>, ApiError> {
        if targets.is_empty() {
            return Err(refuse(ErrorCode::InvalidArguments, "A step needs targets."));
        }
        targets
            .iter()
            .map(|target| {
                let id = self.resolve(target)?;
                let object = tx
                    .document()
                    .object(id)
                    .ok_or_else(|| refuse(ErrorCode::EntityNotFound, "No such entity."))?;
                if !is_writable(&object.entity) {
                    return Err(unsupported(&object.entity));
                }
                Ok(Source {
                    id,
                    entity: object.entity.clone(),
                    attrs: object.attrs.clone(),
                    layout: object.layout,
                })
            })
            .collect()
    }

    /// Change each target with `how`, or, with `copy_as`, add a changed copy.
    fn each(
        &mut self,
        tx: &mut Transaction,
        targets: &[String],
        copy_as: &Option<String>,
        how: impl Fn(&Entity) -> Result<Entity, ApiError>,
    ) -> Result<(), ApiError> {
        let sources = self.sources(tx, targets)?;
        match copy_as {
            None => {
                let changes = sources
                    .iter()
                    .map(|s| Ok((s.id, how(&s.entity)?)))
                    .collect::<Result<Vec<_>, ApiError>>()?;
                tx.perform(ModifyEntities::new("Modify", changes));
                Ok(())
            }
            Some(prefix) => {
                name_ok(prefix)?;
                for (n, source) in sources.into_iter().enumerate() {
                    let copy = how(&source.entity)?;
                    self.add(
                        tx,
                        &format!("{prefix}_{}", n + 1),
                        copy,
                        source.attrs,
                        source.layout,
                    )?;
                }
                Ok(())
            }
        }
    }

    fn place(
        &mut self,
        tx: &mut Transaction,
        targets: &[String],
        placements: &[array::Placement],
        prefix: &str,
    ) -> Result<(), ApiError> {
        let sources = self.sources(tx, targets)?;
        self.place_sources(tx, sources, placements, prefix)
    }

    /// Add the original objects as each placement but the first has them.
    fn place_sources(
        &mut self,
        tx: &mut Transaction,
        sources: Vec<Source>,
        placements: &[array::Placement],
        prefix: &str,
    ) -> Result<(), ApiError> {
        name_ok(prefix)?;
        let several = sources.len() > 1;
        for (k, placement) in placements.iter().enumerate().skip(1) {
            for (n, source) in sources.iter().enumerate() {
                let name = if several {
                    format!("{prefix}_{k}_{}", n + 1)
                } else {
                    format!("{prefix}_{k}")
                };
                self.add(
                    tx,
                    &name,
                    placement.apply(&source.entity),
                    source.attrs.clone(),
                    source.layout,
                )?;
            }
        }
        Ok(())
    }

    /// Add one new entity and remember it by `name`.
    fn add(
        &mut self,
        tx: &mut Transaction,
        name: &str,
        entity: Entity,
        attrs: Attributes,
        layout: Option<LayoutId>,
    ) -> Result<(), ApiError> {
        name_ok(name)?;
        if self.made.contains_key(name) {
            return Err(refuse(
                ErrorCode::InvalidArguments,
                format!("The name {name:?} is used twice."),
            ));
        }
        if self.made.len() >= BATCH_OPERATIONS_MAX {
            return Err(refuse(
                ErrorCode::PayloadTooLarge,
                format!("A request can make at most {BATCH_OPERATIONS_MAX} entities."),
            ));
        }
        tx.perform(AddCopies::new("Add", vec![(entity, attrs)]).in_layout(layout));
        let id = tx
            .document()
            .objects()
            .last()
            .map(|o| o.id)
            .ok_or_else(|| refuse(ErrorCode::InternalError, "The new entity vanished."))?;
        self.made.insert(name.to_owned(), id);
        Ok(())
    }
}
