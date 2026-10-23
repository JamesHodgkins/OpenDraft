//! Pausing the pointer over a snap point, an end or a segment keeps it: the
//! snap point as the start of tracking paths, an end for Extension, a segment
//! for Parallel. Pause again over something kept and it is let go. What is
//! kept lasts until a point is placed or the prompt ends.

use egui::{vec2, Painter, Rect, Shape, Stroke};
use od_core::{Document, Point};

use super::{CanvasStyle, View};
use crate::snap::{
    candidates, geometric_center, resolve, Acquired, Geom, Guide, Origin, SnapId, SnapKind,
    SnapPoint, Track,
};

/// How long the pointer has to rest on something to keep it, in seconds.
const DWELL: f64 = 0.3;
/// Most things kept at once (the spec sets no limit; this is a sanity bound).
const KEEP_LIMIT: usize = 32;

/// What a frame of the pointer resting did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Pulse {
    /// A pause is still counting: ask for another frame.
    pub counting: bool,
    /// Something was kept or let go: the paths changed.
    pub changed: bool,
}

#[derive(Default)]
pub(super) struct Acquire {
    /// What is kept, with the point of it the pause was on.
    kept: Vec<(Acquired, Point)>,
    /// What the pointer is resting on, and since when.
    hover: Option<(Acquired, f64)>,
    /// The pause that was already acted on: leaving ends it.
    settled: Option<Acquired>,
    /// The paths of what is kept, as of this frame.
    shown: Vec<Track>,
    /// The kept snap points, as of this frame.
    starts: Vec<Origin>,
}

/// Whether the snap point `id` is still at `at` in `document`.
fn still_there(document: &Document, id: SnapId, at: Point) -> bool {
    let near = |p: &Point| (p.x - at.x).hypot(p.y - at.y) < 1e-6 * (1.0 + at.x.abs() + at.y.abs());
    match id.sources() {
        [Some(a), Some(b)] => match (document.get(a), document.get(b)) {
            (Some(a), Some(b)) => a.intersections(b).iter().any(near),
            _ => false,
        },
        [Some(a), None] => document.get(a).is_some_and(|e| match id.kind() {
            SnapKind::GeometricCenter => geometric_center(e).is_some_and(|p| near(&p)),
            // Where one object crosses itself: it is there while it is.
            SnapKind::Intersection | SnapKind::ApparentIntersection => true,
            kind => candidates(e)
                .iter()
                .any(|c| c.kind == kind && near(&c.point)),
        }),
        _ => false,
    }
}

impl Acquire {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Forget what is kept if `placed` (a point was placed).
    pub fn clear_if(&mut self, placed: bool) {
        if placed {
            self.clear();
        }
    }

    /// One frame: the pointer rests over `targets`, the first being what it
    /// is on. A pause on something kept lets it (and what shares its place)
    /// go; otherwise it keeps them.
    pub fn update(&mut self, now: f64, targets: &[(Acquired, Point)]) -> Pulse {
        let Some(&(lead, _)) = targets.first() else {
            self.hover = None;
            self.settled = None;
            return Pulse::default();
        };
        match self.hover {
            Some((on, since)) if on == lead => {
                if now - since < DWELL {
                    return Pulse {
                        counting: true,
                        changed: false,
                    };
                }
                if self.settled == Some(lead) {
                    return Pulse::default();
                }
                self.settled = Some(lead);
                let adding = !self.kept.iter().any(|(k, _)| *k == lead);
                for &(key, at) in targets {
                    let held = self.kept.iter().position(|(k, _)| *k == key);
                    match (adding, held) {
                        (true, None) if self.kept.len() < KEEP_LIMIT => self.kept.push((key, at)),
                        (false, Some(i)) => {
                            self.kept.remove(i);
                        }
                        _ => {}
                    }
                }
                Pulse {
                    counting: false,
                    changed: true,
                }
            }
            _ => {
                self.hover = Some((lead, now));
                self.settled = None;
                Pulse {
                    counting: true,
                    changed: false,
                }
            }
        }
    }

    /// The kept snap point the pointer at `cursor` is within `tolerance` of:
    /// resting there lets it go even if no snap is running to find it.
    pub fn kept_near(&self, cursor: Point, tolerance: f64) -> Option<(Acquired, Point)> {
        self.kept
            .iter()
            .filter(|(k, _)| matches!(k, Acquired::Origin(_)))
            .find(|(_, at)| (at.x - cursor.x).hypot(at.y - cursor.y) <= tolerance)
            .copied()
    }

    /// Work out the paths of what is kept in `document`, dropping what has
    /// gone, and leaving out Parallel when `parallel` is false (ortho holds
    /// the point, or there is no last point).
    pub fn paths(&mut self, document: &Document, parallel: bool) -> &[Track] {
        self.kept.retain(|(k, at)| match k {
            Acquired::Origin(id) => still_there(document, *id, *at),
            _ => resolve(document, *k).is_some(),
        });
        self.starts = self
            .kept
            .iter()
            .filter_map(|(k, at)| match k {
                Acquired::Origin(id) => Some(Origin {
                    at: *at,
                    kind: id.kind(),
                }),
                _ => None,
            })
            .collect();
        self.shown = self
            .kept
            .iter()
            .filter(|(k, _)| parallel || matches!(k, Acquired::End { .. }))
            .filter_map(|(k, at)| {
                let track = resolve(document, *k)?;
                // The "+" of a direction sits where the pause was.
                let mark = if matches!(k, Acquired::Along { .. }) {
                    *at
                } else {
                    track.mark
                };
                Some(Track { mark, ..track })
            })
            .collect();
        &self.shown
    }

    /// The kept snap points (the starts of tracking paths).
    pub fn origins(&self) -> &[Origin] {
        &self.starts
    }
}

fn plus(painter: &Painter, c: egui::Pos2, stroke: Stroke) {
    painter.line_segment([c - vec2(4.0, 0.0), c + vec2(4.0, 0.0)], stroke);
    painter.line_segment([c - vec2(0.0, 4.0), c + vec2(0.0, 4.0)], stroke);
}

impl View {
    /// The "+" on everything kept, and the dotted path to the snap point in
    /// force when it is an Extension, a Parallel or a tracking point.
    pub(super) fn paint_acquired(
        &self,
        painter: &Painter,
        rect: Rect,
        style: &CanvasStyle,
        snapped: Option<SnapPoint>,
    ) {
        let screen = |p: Point| self.camera.world_to_screen(p, rect);
        let stroke = Stroke::new(1.5_f32, style.snap);
        let dots = |painter: &Painter, path: Vec<Point>| {
            let path: Vec<_> = path.into_iter().map(screen).collect();
            painter.extend(Shape::dotted_line(
                &path,
                style.snap.gamma_multiply(0.8),
                6.0,
                0.9,
            ));
        };
        let base = self.point_prompt.as_ref().and_then(|p| p.base);
        // As far as a path can usefully reach across the view.
        let reach = rect.size().length() as f64 / self.camera.zoom;
        for origin in self.snap_acquire.origins() {
            plus(painter, screen(origin.at), stroke);
        }
        for at in &self.snap_cycle.temps {
            plus(painter, screen(*at), stroke);
        }
        for track in &self.snap_acquire.shown {
            plus(painter, screen(track.mark), stroke);
            let object = track.key.object();
            let Some(at) = snapped.filter(|s| object.is_some() && s.source[0] == object) else {
                continue;
            };
            // A parallel is shown as the whole line through the last point.
            let world = match (track.geom, base) {
                (Geom::Along { dir }, Some(b)) => vec![
                    Point::new(b.x - dir.x * reach, b.y - dir.y * reach),
                    Point::new(b.x + dir.x * reach, b.y + dir.y * reach),
                ],
                _ => track.path_to(at.point, base),
            };
            dots(painter, world);
        }
        // Extension and Parallel draw their own paths, above.
        let own = |g: &&Guide| !matches!(g.kind, Some(SnapKind::Extension | SnapKind::Parallel));
        for guide in snapped
            .iter()
            .flat_map(|s| s.guides.iter().flatten().filter(own))
        {
            self.paint_guide(painter, guide, snapped, reach, &dots);
        }
    }

    /// A tracking or polar path, per `TRACKPATH`: the whole line, or from
    /// where it starts to the snap point.
    fn paint_guide(
        &self,
        painter: &Painter,
        guide: &Guide,
        snapped: Option<SnapPoint>,
        reach: f64,
        dots: &dyn Fn(&Painter, Vec<Point>),
    ) {
        let polar = guide.kind.is_none();
        let tracking = &self.snap_cycle.tracking;
        let Some(to) = snapped.map(|s| s.point).filter(|_| tracking.shows(polar)) else {
            return;
        };
        let (o, d) = (guide.origin, guide.dir);
        let path = if tracking.full_path(polar) {
            vec![
                Point::new(o.x - d.x * reach, o.y - d.y * reach),
                Point::new(o.x + d.x * reach, o.y + d.y * reach),
            ]
        } else {
            vec![o, to]
        };
        dots(painter, path);
    }
}

#[cfg(test)]
mod tests {
    use od_core::EntityId;

    use super::*;

    fn key(n: u64) -> Acquired {
        Acquired::End {
            object: EntityId(n),
            last: true,
        }
    }

    /// What the pointer is over: object `n`, at the origin.
    fn over(n: u64) -> Vec<(Acquired, Point)> {
        vec![(key(n), Point::new(0.0, 0.0))]
    }

    fn kept(a: &Acquire) -> Vec<Acquired> {
        a.kept.iter().map(|(k, _)| *k).collect()
    }

    #[test]
    fn a_pause_keeps_what_it_rests_on_once() {
        let mut a = Acquire::default();
        assert!(a.update(0.0, &over(1)).counting);
        assert!(a.update(0.2, &over(1)).counting);
        assert!(a.kept.is_empty());
        let kept_it = a.update(0.4, &over(1));
        assert!(kept_it.changed && !kept_it.counting);
        assert_eq!(kept(&a), [key(1)]);
        // Resting longer does not toggle it off again.
        assert_eq!(a.update(5.0, &over(1)), Pulse::default());
        assert_eq!(kept(&a), [key(1)]);
    }

    #[test]
    fn passing_over_something_keeps_nothing() {
        let mut a = Acquire::default();
        a.update(0.0, &over(1));
        a.update(0.1, &over(2));
        a.update(0.2, &[]);
        a.update(0.3, &over(1));
        assert!(a.kept.is_empty());
    }

    #[test]
    fn pausing_again_over_something_kept_lets_it_go() {
        let mut a = Acquire::default();
        a.update(0.0, &over(1));
        a.update(0.4, &over(1));
        a.update(0.5, &[]);
        a.update(1.0, &over(1));
        let gone = a.update(1.4, &over(1));
        assert!(gone.changed);
        assert!(a.kept.is_empty());
    }

    #[test]
    fn what_shares_a_place_is_kept_together_and_let_go_by_the_one_rested_on() {
        // A line's end is a tracking point and an extension at once.
        let mut a = Acquire::default();
        let both = vec![
            (key(1), Point::new(0.0, 0.0)),
            (key(2), Point::new(0.0, 0.0)),
        ];
        a.update(0.0, &both);
        a.update(0.4, &both);
        assert_eq!(kept(&a), [key(1), key(2)]);
        a.update(0.5, &[]);
        // Resting on one of them alone lets that one go, led by it.
        a.update(1.0, &over(1));
        a.update(1.4, &over(1));
        assert_eq!(kept(&a), [key(2)]);
    }

    #[test]
    fn placing_a_point_forgets_what_was_kept() {
        let mut a = Acquire::default();
        a.update(0.0, &over(1));
        a.update(0.4, &over(1));
        a.clear_if(false);
        assert_eq!(a.kept.len(), 1);
        a.clear_if(true);
        assert!(a.kept.is_empty());
    }

    #[test]
    fn a_kept_snap_point_is_found_near_the_pointer_and_survives_only_while_it_exists() {
        use crate::snap::{SnapKind, SnapPoint};
        let mut doc = Document::from_entities([od_core::Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0),
        }]);
        let mut snap = SnapPoint::new(Point::new(10.0, 0.0), SnapKind::Endpoint);
        snap.source[0] = Some(EntityId(0));
        let mut a = Acquire::default();
        let target = [(Acquired::Origin(snap.id()), snap.point)];
        a.update(0.0, &target);
        a.update(0.4, &target);
        a.paths(&doc, false);
        assert_eq!(a.origins().len(), 1);
        assert!(a.kept_near(Point::new(10.5, 0.2), 1.0).is_some());
        assert!(a.kept_near(Point::new(14.0, 0.0), 1.0).is_none());
        // The line goes: so does the point.
        doc.remove(EntityId(0));
        a.paths(&doc, false);
        assert!(a.origins().is_empty() && a.kept.is_empty());
    }
}
