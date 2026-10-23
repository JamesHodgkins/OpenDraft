//! Snaps that follow a path through something the pointer has paused over:
//! Extension (the continuation of a line, arc or elliptical arc past one of
//! its ends) and Parallel (the direction of a straight segment, through the
//! command's last point). The pause "acquires" the object; the acquired
//! objects live in the view, and this is the pure part: what can be
//! acquired where, the path an acquired object gives, and the snap points on
//! those paths.

use std::f64::consts::TAU;

use od_core::{arc, Curve, Document, Ellipse, Entity, EntityId, Object, Point};

use super::{Class, Guide, Ranked, SnapId, SnapKind, SnapModes, SnapPoint};

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

/// What the pointer has paused over, and so keeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Acquired {
    /// An end of a line, arc or elliptical arc: the start, or the end
    /// (`last`).
    End { object: EntityId, last: bool },
    /// A straight segment, the `curve`th of the object's outline.
    Along { object: EntityId, curve: u16 },
    /// A snap point (an endpoint, a midpoint...): the start of tracking paths.
    Origin(SnapId),
}

impl Acquired {
    /// The object an end or a segment belongs to.
    pub fn object(self) -> Option<EntityId> {
        match self {
            Acquired::End { object, .. } | Acquired::Along { object, .. } => Some(object),
            Acquired::Origin(_) => None,
        }
    }
}

/// The path an acquired object gives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Geom {
    /// Past `from`, along the unit vector `dir`.
    Ray { from: Point, dir: Point },
    /// Round the circle from angle `from`, counter-clockwise or not, for
    /// `room` radians at most (what the arc itself does not cover).
    Arc {
        center: Point,
        radius: f64,
        from: f64,
        ccw: bool,
        room: f64,
    },
    /// The same for the parameter of a full ellipse.
    Ellipse {
        ellipse: Ellipse,
        from: f64,
        ccw: bool,
        room: f64,
    },
    /// The line of this unit direction through the command's last point.
    Along { dir: Point },
}

/// An acquired object, resolved against the drawing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Track {
    pub key: Acquired,
    pub geom: Geom,
    /// Where the "+" of an acquired point is drawn.
    pub mark: Point,
}

fn unit(from: Point, to: Point) -> Option<Point> {
    let len = distance(from, to);
    (len > 1e-9).then(|| Point::new((to.x - from.x) / len, (to.y - from.y) / len))
}

/// The ends of an entity that Extension can follow past, start first.
fn ends(entity: &Entity) -> Vec<(bool, Point)> {
    match entity {
        Entity::Line { start, end } => vec![(false, *start), (true, *end)],
        Entity::Arc {
            center,
            radius,
            start,
            end,
        } => vec![
            (false, arc::point_at(*center, radius.abs(), *start)),
            (true, arc::point_at(*center, radius.abs(), *end)),
        ],
        Entity::Ellipse(e) if !e.is_full() => {
            vec![(false, e.start_point()), (true, e.end_point())]
        }
        _ => Vec::new(),
    }
}

/// The path past one end of `entity`.
fn end_geom(entity: &Entity, last: bool) -> Option<Geom> {
    match entity {
        Entity::Line { start, end } => {
            let (from, other) = if last { (*end, *start) } else { (*start, *end) };
            Some(Geom::Ray {
                from,
                dir: unit(other, from)?,
            })
        }
        Entity::Arc {
            center,
            radius,
            start,
            end,
        } => Some(Geom::Arc {
            center: *center,
            radius: radius.abs(),
            from: if last { *end } else { *start },
            ccw: last,
            room: TAU - arc::sweep(*start, *end),
        }),
        Entity::Ellipse(e) if !e.is_full() => Some(Geom::Ellipse {
            ellipse: Ellipse {
                start: 0.0,
                end: TAU,
                ..*e
            },
            from: if last { e.end } else { e.start },
            ccw: last,
            room: TAU - e.sweep(),
        }),
        _ => None,
    }
}

/// `key` as a track in `document`, if the object is still there and still
/// the kind it was.
pub fn resolve(document: &Document, key: Acquired) -> Option<Track> {
    let entity = document.get(key.object()?)?;
    match key {
        Acquired::End { last, .. } => {
            let geom = end_geom(entity, last)?;
            let mark = ends(entity).into_iter().find(|(l, _)| *l == last)?.1;
            Some(Track { key, geom, mark })
        }
        Acquired::Origin(_) => None,
        Acquired::Along { curve, .. } => match entity.curves().get(usize::from(curve))? {
            Curve::Segment { a, b } => Some(Track {
                key,
                geom: Geom::Along { dir: unit(*a, *b)? },
                mark: Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0),
            }),
            _ => None,
        },
    }
}

impl Track {
    /// The points of the dotted path from where it starts to `to`, which is
    /// on it: a Parallel path starts at the command's last point `base`.
    pub fn path_to(&self, to: Point, base: Option<Point>) -> Vec<Point> {
        let steps =
            |delta: f64, per_radian: f64| ((delta * per_radian).ceil() as usize).clamp(2, 400);
        match self.geom {
            Geom::Ray { from, .. } => vec![from, to],
            Geom::Along { .. } => base.into_iter().chain([to]).collect(),
            Geom::Arc {
                center,
                radius,
                from,
                ccw,
                ..
            } => {
                let a = arc::angle_of(center, to);
                let delta = if ccw { a - from } else { from - a }.rem_euclid(TAU);
                let n = steps(delta, 20.0);
                (0..=n)
                    .map(|i| {
                        let part = delta * i as f64 / n as f64;
                        arc::point_at(center, radius, if ccw { from + part } else { from - part })
                    })
                    .collect()
            }
            Geom::Ellipse {
                ellipse, from, ccw, ..
            } => {
                let t = ellipse.parameter_of(to);
                let delta = if ccw { t - from } else { from - t }.rem_euclid(TAU);
                let n = steps(delta, 20.0);
                (0..=n)
                    .map(|i| {
                        let part = delta * i as f64 / n as f64;
                        ellipse.point_at(if ccw { from + part } else { from - part })
                    })
                    .collect()
            }
        }
    }
}

/// What the pointer at `cursor` is over that could be acquired, with the
/// point of it the pointer is on (an end, or the nearest point of a
/// segment): the ends within `tolerance` (for Extension), nearest first, or,
/// if there are none and Parallel is wanted, the straight segments within it.
pub fn targets<'a>(
    objects: impl IntoIterator<Item = &'a Object>,
    cursor: Point,
    tolerance: f64,
    modes: &SnapModes,
) -> Vec<(Acquired, Point)> {
    let objects: Vec<&Object> = objects.into_iter().collect();
    let mut found: Vec<(f64, Acquired, Point)> = Vec::new();
    if modes.extension {
        for o in &objects {
            for (last, at) in ends(&o.entity) {
                let dist = distance(at, cursor);
                if dist <= tolerance && end_geom(&o.entity, last).is_some() {
                    found.push((dist, Acquired::End { object: o.id, last }, at));
                }
            }
        }
    }
    if found.is_empty() && modes.parallel {
        for o in &objects {
            for (i, curve) in o.entity.curves().iter().enumerate() {
                let Curve::Segment { a, b } = curve else {
                    continue;
                };
                let near = curve.nearest_point(cursor);
                let dist = distance(near, cursor);
                if dist <= tolerance && unit(*a, *b).is_some() {
                    if let Ok(curve) = u16::try_from(i) {
                        let key = Acquired::Along {
                            object: o.id,
                            curve,
                        };
                        found.push((dist, key, near));
                    }
                }
            }
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.into_iter().map(|(_, key, at)| (key, at)).collect()
}

/// Where on the path the pointer is, if it is within `tolerance` of it, and
/// beyond `tolerance` of where the path starts (the end's own snap is there).
fn on_path(track: &Track, cursor: Point, tolerance: f64, base: Option<Point>) -> Option<Point> {
    let (point, start) = match track.geom {
        Geom::Ray { from, dir } => {
            let t = (cursor.x - from.x) * dir.x + (cursor.y - from.y) * dir.y;
            if t <= 0.0 {
                return None;
            }
            (Point::new(from.x + dir.x * t, from.y + dir.y * t), from)
        }
        Geom::Arc {
            center,
            radius,
            from,
            ccw,
            room,
        } => {
            let a = arc::angle_of(center, cursor);
            let delta = if ccw { a - from } else { from - a }.rem_euclid(TAU);
            if delta > room || radius < 1e-9 {
                return None;
            }
            (
                arc::point_at(center, radius, a),
                arc::point_at(center, radius, from),
            )
        }
        Geom::Ellipse {
            ellipse,
            from,
            ccw,
            room,
        } => {
            let (point, t) = ellipse.nearest(cursor);
            let delta = if ccw { t - from } else { from - t }.rem_euclid(TAU);
            if delta > room {
                return None;
            }
            (point, ellipse.point_at(from))
        }
        Geom::Along { dir } => {
            let base = base?;
            let s = (cursor.x - base.x) * dir.x + (cursor.y - base.y) * dir.y;
            (Point::new(base.x + dir.x * s, base.y + dir.y * s), base)
        }
    };
    (distance(point, cursor) <= tolerance && distance(point, start) >= tolerance).then_some(point)
}

/// The Extension and Parallel snap points on the acquired `tracks`.
pub(super) fn candidates(
    tracks: &[Track],
    cursor: Point,
    tolerance: f64,
    base: Option<Point>,
    modes: &SnapModes,
) -> Vec<Ranked> {
    tracks
        .iter()
        .filter_map(|track| {
            let kind = match track.key {
                Acquired::End { .. } if modes.extension => SnapKind::Extension,
                Acquired::Along { .. } if modes.parallel => SnapKind::Parallel,
                _ => return None,
            };
            let point = on_path(track, cursor, tolerance, base)?;
            let mut snap = SnapPoint::new(point, kind).from(track.key.object()?);
            // A straight path is remembered, so a distance can be typed along
            // it and the tooltip can say how far and in which direction.
            snap.guides[0] = match track.geom {
                Geom::Ray { from, dir } => Some(Guide {
                    origin: from,
                    dir,
                    kind: Some(kind),
                }),
                Geom::Along { dir } => base.map(|origin| Guide {
                    origin,
                    dir,
                    kind: Some(kind),
                }),
                _ => None,
            };
            Some(Ranked {
                snap,
                dist: distance(point, cursor),
                // A path, like a tracking path: where two meet, that wins.
                class: Class::Track,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::f64::consts::FRAC_PI_2;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn doc(entities: &[Entity]) -> Document {
        Document::from_entities(entities.iter().cloned())
    }

    fn line(a: Point, b: Point) -> Entity {
        Entity::Line { start: a, end: b }
    }

    fn modes() -> SnapModes {
        SnapModes {
            extension: true,
            parallel: true,
            ..SnapModes::default()
        }
    }

    fn track(document: &Document, key: Acquired) -> Track {
        resolve(document, key).unwrap()
    }

    #[test]
    fn a_line_extends_past_either_end_and_only_past_it() {
        let d = doc(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        let id = EntityId(0);
        let last = track(
            &d,
            Acquired::End {
                object: id,
                last: true,
            },
        );
        let first = track(
            &d,
            Acquired::End {
                object: id,
                last: false,
            },
        );
        let at = |t: &Track, x: f64, y: f64| candidates(&[*t], p(x, y), 1.0, None, &modes());
        let found = at(&last, 15.0, 0.4);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].snap.kind, SnapKind::Extension);
        assert_eq!(found[0].snap.point, p(15.0, 0.0));
        // The path is kept for typing a distance along it, and for the tooltip.
        let guide = found[0].snap.guides[0].expect("a straight path");
        assert_eq!((guide.origin, guide.dir), (p(10.0, 0.0), p(1.0, 0.0)));
        assert_eq!(found[0].snap.label(), "Extension: 5.0000 < 0\u{b0}");
        assert!(at(&last, 5.0, 0.4).is_empty(), "over the line itself");
        assert!(at(&last, 15.0, 3.0).is_empty(), "too far from the path");
        assert!(
            at(&last, 10.5, 0.2).is_empty(),
            "the end's own snap is there"
        );
        assert_eq!(at(&first, -7.0, -0.3)[0].snap.point, p(-7.0, 0.0));
    }

    #[test]
    fn an_arc_continues_round_its_circle_not_along_its_tangent() {
        // A quarter arc from east to north: past the north end it carries on
        // toward the west, and past the east end toward the south.
        let d = doc(&[Entity::Arc {
            center: p(0.0, 0.0),
            radius: 10.0,
            start: 0.0,
            end: FRAC_PI_2,
        }]);
        let id = EntityId(0);
        let last = track(
            &d,
            Acquired::End {
                object: id,
                last: true,
            },
        );
        let first = track(
            &d,
            Acquired::End {
                object: id,
                last: false,
            },
        );
        let here = |t: &Track, x: f64, y: f64| candidates(&[*t], p(x, y), 1.0, None, &modes());
        let west = here(&last, -7.0, 7.3);
        assert_eq!(west.len(), 1);
        let q = west[0].snap.point;
        assert!((q.x.hypot(q.y) - 10.0).abs() < 1e-9, "on the circle: {q:?}");
        assert!(here(&last, 7.0, 7.3).is_empty(), "that is the arc itself");
        assert!(!here(&first, 7.3, -7.0).is_empty());
        // Either end carries on round the rest of the circle, never over the arc.
        assert!(!here(&first, -7.0, 7.3).is_empty());
        assert!(here(&first, 7.0, 7.3).is_empty(), "that is the arc itself");
    }

    #[test]
    fn an_elliptical_arc_continues_round_its_ellipse() {
        let e = Ellipse::new(p(0.0, 0.0), p(10.0, 0.0), 0.5, 0.0, FRAC_PI_2).unwrap();
        let d = doc(&[Entity::Ellipse(e)]);
        let last = track(
            &d,
            Acquired::End {
                object: EntityId(0),
                last: true,
            },
        );
        // Past the end at (0, 5), the ellipse goes on to the west.
        let found = candidates(&[last], p(-9.9, 0.4), 1.0, None, &modes());
        assert_eq!(found.len(), 1, "{found:?}");
        let q = found[0].snap.point;
        assert!((q.x * q.x / 100.0 + q.y * q.y / 25.0 - 1.0).abs() < 1e-6);
        assert!(candidates(&[last], p(9.9, 0.4), 1.0, None, &modes()).is_empty());
    }

    #[test]
    fn a_full_circle_and_a_whole_ellipse_have_no_ends_to_acquire() {
        let d = doc(&[
            Entity::Circle {
                center: p(0.0, 0.0),
                radius: 5.0,
            },
            Entity::Ellipse(Ellipse::full(p(0.0, 0.0), p(10.0, 0.0), 0.5).unwrap()),
        ]);
        let objects = d.objects();
        assert!(targets(objects, p(5.0, 0.0), 1.0, &modes()).is_empty());
        assert!(targets(objects, p(10.0, 0.0), 1.0, &modes()).is_empty());
    }

    #[test]
    fn pausing_over_an_end_acquires_it_and_over_a_segment_acquires_its_direction() {
        let d = doc(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        let id = EntityId(0);
        let near_end = targets(d.objects(), p(9.5, 0.5), 1.0, &modes());
        assert_eq!(
            near_end,
            [(
                Acquired::End {
                    object: id,
                    last: true
                },
                p(10.0, 0.0)
            )]
        );
        // Away from the ends it is the segment's direction.
        let mid = targets(d.objects(), p(5.0, 0.5), 1.0, &modes());
        // ... with the point of it the pointer is on.
        assert_eq!(
            mid,
            [(
                Acquired::Along {
                    object: id,
                    curve: 0
                },
                p(5.0, 0.0)
            )]
        );
        // With the modes off, nothing.
        let off = SnapModes::default();
        let off = SnapModes {
            extension: false,
            ..off
        };
        assert!(targets(d.objects(), p(5.0, 0.5), 1.0, &off).is_empty());
        assert!(targets(d.objects(), p(9.5, 0.5), 1.0, &off).is_empty());
    }

    #[test]
    fn polyline_segments_and_rectangle_edges_give_their_direction_but_arcs_do_not() {
        let d = doc(&[
            Entity::Polyline {
                points: vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)],
                closed: false,
                bulges: Vec::new(),
                widths: Vec::new(),
            },
            Entity::Rectangle {
                min: p(20.0, 0.0),
                max: p(30.0, 6.0),
            },
            Entity::Arc {
                center: p(50.0, 0.0),
                radius: 5.0,
                start: 0.0,
                end: FRAC_PI_2,
            },
        ]);
        // The second segment of the polyline, away from its ends.
        let up = targets(d.objects(), p(10.3, 5.0), 1.0, &modes());
        assert_eq!(up.len(), 1);
        assert_eq!(
            up[0].0,
            Acquired::Along {
                object: EntityId(0),
                curve: 1
            }
        );
        // The rectangle's top edge (its third).
        let top = targets(d.objects(), p(25.0, 6.2), 1.0, &modes());
        assert_eq!(top.len(), 1);
        assert!(matches!(top[0].0, Acquired::Along { curve: 2, .. }));
        let t = track(&d, top[0].0);
        assert!(matches!(t.geom, Geom::Along { dir } if dir.y.abs() < 1e-12 && dir.x.abs() > 0.99));
        // Mid-arc: nothing to take a straight direction from.
        let r = 5.0 / 2.0_f64.sqrt();
        assert!(targets(d.objects(), p(50.0 + r, r), 1.0, &modes()).is_empty());
    }

    #[test]
    fn parallel_runs_through_the_last_point_in_the_acquired_direction() {
        let d = doc(&[line(p(0.0, 0.0), p(10.0, 10.0))]);
        let t = track(
            &d,
            Acquired::Along {
                object: EntityId(0),
                curve: 0,
            },
        );
        let base = Some(p(20.0, 0.0));
        let found = candidates(&[t], p(30.3, 10.0), 1.0, base, &modes());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].snap.kind, SnapKind::Parallel);
        assert_eq!(found[0].snap.guides[0].map(|g| g.origin), base);
        let q = found[0].snap.point;
        assert!(
            (q.x - 30.0).abs() < 0.2 && (q.y - 10.0).abs() < 0.2,
            "{q:?}"
        );
        // Backwards along the line works too.
        assert!(!candidates(&[t], p(10.0, -10.1), 1.0, base, &modes()).is_empty());
        // Not near the line, or with no last point: nothing.
        assert!(candidates(&[t], p(30.0, 14.0), 1.0, base, &modes()).is_empty());
        assert!(candidates(&[t], p(30.3, 10.0), 1.0, None, &modes()).is_empty());
        // Right at the last point it has no direction to follow.
        assert!(candidates(&[t], p(20.2, 0.1), 1.0, base, &modes()).is_empty());
    }

    #[test]
    fn the_dotted_path_runs_from_where_the_path_starts_to_the_snap_point() {
        let d = doc(&[
            line(p(0.0, 0.0), p(10.0, 0.0)),
            Entity::Arc {
                center: p(0.0, 0.0),
                radius: 10.0,
                start: 0.0,
                end: FRAC_PI_2,
            },
            line(p(0.0, 0.0), p(10.0, 10.0)),
        ]);
        let ray = track(
            &d,
            Acquired::End {
                object: EntityId(0),
                last: true,
            },
        );
        assert_eq!(
            ray.path_to(p(15.0, 0.0), None),
            [p(10.0, 0.0), p(15.0, 0.0)]
        );
        let arc = track(
            &d,
            Acquired::End {
                object: EntityId(1),
                last: true,
            },
        );
        let to = p(-10.0, 0.0);
        let path = arc.path_to(to, None);
        assert!(path.len() > 3);
        assert!((path[0].x).abs() < 1e-9 && (path[0].y - 10.0).abs() < 1e-9);
        let last = path[path.len() - 1];
        assert!((last.x + 10.0).abs() < 1e-9 && last.y.abs() < 1e-9);
        assert!(path.iter().all(|q| (q.x.hypot(q.y) - 10.0).abs() < 1e-9));
        let along = track(
            &d,
            Acquired::Along {
                object: EntityId(2),
                curve: 0,
            },
        );
        assert_eq!(
            along.path_to(p(5.0, 5.0), Some(p(1.0, 1.0))),
            [p(1.0, 1.0), p(5.0, 5.0)]
        );
    }

    #[test]
    fn a_track_goes_when_its_object_does_or_changes_kind() {
        let mut d = doc(&[line(p(0.0, 0.0), p(10.0, 0.0))]);
        let key = Acquired::End {
            object: EntityId(0),
            last: true,
        };
        assert!(resolve(&d, key).is_some());
        d.remove(EntityId(0));
        assert!(resolve(&d, key).is_none());
    }
}
