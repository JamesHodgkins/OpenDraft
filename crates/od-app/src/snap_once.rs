//! One-shot object snaps: what applies to the next point only. AutoCAD's
//! typed `END`, `MID`..., `NON`, and the two that take more than one point:
//! `FRO` (a base point, then an offset from it) and `M2P` (the middle of two
//! points). The Shift+right-click menu offers the same things.
//!
//! Pure state, no UI: `interact.rs` feeds it the points a prompt receives.

use od_canvas::{SnapKind, SnapOverride};
use od_core::{Entity, EntityId, Point};

/// The snaps a typed word names, by the full name (three letters or more of it
/// are enough, as in AutoCAD).
const NAMES: [(&str, SnapKind); 16] = [
    ("ENDPOINT", SnapKind::Endpoint),
    ("MIDPOINT", SnapKind::Midpoint),
    ("CENTER", SnapKind::Center),
    ("NODE", SnapKind::Node),
    ("QUADRANT", SnapKind::Quadrant),
    ("INTERSECTION", SnapKind::Intersection),
    ("INSERTION", SnapKind::Insertion),
    ("PERPENDICULAR", SnapKind::Perpendicular),
    ("TANGENT", SnapKind::Tangent),
    ("NEAREST", SnapKind::Nearest),
    ("EXTENSION", SnapKind::Extension),
    ("PARALLEL", SnapKind::Parallel),
    ("GEOMETRICCENTER", SnapKind::GeometricCenter),
    ("APPARENTINTERSECTION", SnapKind::ApparentIntersection),
    ("EXTENDEDINTERSECTION", SnapKind::ExtendedIntersection),
    (
        "EXTENDEDAPPARENTINTERSECTION",
        SnapKind::ExtendedIntersection,
    ),
];

/// Words AutoCAD takes that are not the start of a full name.
const ALIASES: [(&str, SnapKind); 4] = [
    ("GCEN", SnapKind::GeometricCenter),
    ("APPINT", SnapKind::ApparentIntersection),
    ("EXTINT", SnapKind::ExtendedIntersection),
    ("EXTAPPINT", SnapKind::ExtendedIntersection),
];

/// A point built from more than one answer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Chain {
    /// `FRO`: the base point once given, then the offset.
    From(Option<Point>),
    /// `M2P`: the first point once given, then the second.
    Mid(Option<Point>),
    /// `TT`: the next point is a temporary tracking point, not the answer.
    Temp,
    /// `TK`: tracking points chained, each held to an axis from the last, until
    /// Enter; the last is the answer.
    Track(Option<Point>),
}

/// A Perpendicular or Tangent picked on an object before the command had a
/// last point: which point of the object it is waits for the next point (a
/// line drawn perpendicular to, or tangent to, the object).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Deferred {
    pub kind: SnapKind,
    pub object: EntityId,
    /// Where it was picked, to choose between two tangent points.
    pub near: Point,
}

impl Deferred {
    /// The point of `entity` that is the snap seen from `from`: the foot of
    /// the perpendicular or the point of tangency, nearest where it was
    /// picked. `None` when there is none (no foot on a segment's length, no
    /// tangent from inside a circle).
    pub fn point(&self, entity: &Entity, from: Point) -> Option<Point> {
        let found: Vec<Point> = entity
            .curves()
            .iter()
            .flat_map(|c| match self.kind {
                SnapKind::Perpendicular => c.perpendicular_feet(from),
                SnapKind::Tangent => c.tangent_points(from),
                _ => Vec::new(),
            })
            .collect();
        let apart = |p: &Point| (p.x - self.near.x).hypot(p.y - self.near.y);
        found
            .into_iter()
            .min_by(|a, b| apart(a).total_cmp(&apart(b)))
    }
}

/// What typed text asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Wish {
    Snap(SnapOverride),
    Chain(Chain),
}

/// What happened to a point given while a one-shot was waiting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// Not part of a chain: use the point as it is.
    Pass,
    /// Taken as one part of a chain; say what is asked next.
    Consumed(&'static str),
    /// The chain is done: this is the point.
    Done(Point),
}

/// The one-shot override and chain waiting for the next point.
#[derive(Debug, Default)]
pub struct OneShot {
    over: Option<SnapOverride>,
    chain: Option<Chain>,
    /// Temporary tracking points set by `TT`, until the point they help with.
    temps: Vec<Point>,
    deferred: Option<Deferred>,
    /// The first point a deferred snap has settled, to give before the point
    /// that settled it.
    first: Option<Point>,
}

/// What `text` asks for: `END`, `MID,CEN` (several at once), `NON`, `FRO`,
/// `M2P`. `None` when it is anything else, such as a coordinate.
pub fn parse(text: &str) -> Option<Wish> {
    let word = text.trim().to_uppercase();
    match word.as_str() {
        "FRO" | "FROM" => return Some(Wish::Chain(Chain::From(None))),
        "M2P" => return Some(Wish::Chain(Chain::Mid(None))),
        "TT" => return Some(Wish::Chain(Chain::Temp)),
        "TK" => return Some(Wish::Chain(Chain::Track(None))),
        "NON" | "NONE" => return Some(Wish::Snap(SnapOverride::None)),
        _ => {}
    }
    let kinds: Option<Vec<SnapKind>> = word
        .split(',')
        .map(|part| {
            let part = part.trim();
            ALIASES
                .iter()
                .find(|(alias, _)| *alias == part)
                .or_else(|| {
                    NAMES
                        .iter()
                        .find(|(full, _)| part.len() >= 3 && full.starts_with(part))
                })
                .map(|(_, kind)| *kind)
        })
        .collect();
    kinds
        .filter(|k| !k.is_empty())
        .map(|k| Wish::Snap(SnapOverride::Only(k)))
}

impl OneShot {
    /// Start what was asked for. A new override replaces one waiting.
    pub fn begin(&mut self, wish: Wish) {
        match wish {
            Wish::Snap(over) => self.over = Some(over),
            Wish::Chain(chain) => {
                self.over = None;
                self.chain = Some(chain);
            }
        }
    }

    /// Hold a deferred snap until the next point.
    pub fn defer(&mut self, deferred: Deferred) {
        self.over = None;
        self.chain = None;
        self.deferred = Some(deferred);
    }

    pub fn deferred(&self) -> Option<Deferred> {
        self.deferred
    }

    /// The next point has settled the deferred snap at `first`.
    pub fn settle(&mut self, first: Point) {
        self.deferred = None;
        self.first = Some(first);
    }

    /// The point a settled deferred snap gives first, once.
    pub fn take_first(&mut self) -> Option<Point> {
        self.first.take()
    }

    /// Let go of the override (a click found nothing for it); a chain stays.
    pub fn drop_override(&mut self) -> Option<SnapOverride> {
        self.over.take()
    }

    /// Forget all of it (the prompt ended).
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn over(&self) -> Option<&SnapOverride> {
        self.over.as_ref()
    }

    /// The temporary tracking points to follow.
    pub fn temps(&self) -> &[Point] {
        &self.temps
    }

    /// Whether `TK` is collecting points (each is held to an axis).
    pub fn tracking_chain(&self) -> bool {
        matches!(self.chain, Some(Chain::Track(_)))
    }

    /// Enter ends `TK`: the last point it was given is the answer.
    pub fn finish_track(&mut self) -> Option<Point> {
        let Some(Chain::Track(Some(last))) = self.chain else {
            return None;
        };
        self.chain = None;
        self.temps.clear();
        Some(last)
    }

    /// Where a typed relative point measures from instead of the last point.
    pub fn origin(&self) -> Option<Point> {
        match self.chain {
            Some(Chain::From(Some(base)) | Chain::Track(Some(base))) => Some(base),
            _ => None,
        }
    }

    /// Where the rubber band starts while a chain waits for its second point.
    pub fn rubber_base(&self) -> Option<Point> {
        match self.chain {
            Some(Chain::From(Some(p)) | Chain::Mid(Some(p)) | Chain::Track(Some(p))) => Some(p),
            _ => None,
        }
    }

    /// What is asked for now, if a chain is waiting.
    pub fn asking(&self) -> Option<&'static str> {
        if self.deferred.is_some() {
            return Some("Specify the other point:");
        }
        match self.chain? {
            Chain::From(None) => Some("Base point:"),
            Chain::From(Some(_)) => Some("<Offset>:"),
            Chain::Mid(None) => Some("First point of mid:"),
            Chain::Mid(Some(_)) => Some("Second point of mid:"),
            Chain::Temp => Some("Specify temporary tracking point:"),
            Chain::Track(None) => Some("First tracking point:"),
            Chain::Track(Some(_)) => Some("Next point (Enter to end tracking):"),
        }
    }

    /// What the status line adds while a one-shot waits.
    pub fn status(&self) -> Option<String> {
        let deferred = self.deferred.map(|d| format!("[Deferred {:?}]", d.kind));
        match (self.asking(), &self.over) {
            (Some(ask), _) => {
                Some(deferred.map_or_else(|| ask.to_owned(), |d| format!("{d} {ask}")))
            }
            (None, Some(over)) => Some(format!("[{}]", over.label())),
            _ => None,
        }
    }

    /// A point arrived. The override it was given under is spent either way.
    pub fn point(&mut self, at: Point) -> Step {
        self.over = None;
        match self.chain {
            None => {
                self.temps.clear();
                Step::Pass
            }
            Some(Chain::Temp) => {
                self.chain = None;
                self.temps.push(at);
                Step::Consumed("Now specify the point; it tracks from the temporary point.")
            }
            Some(Chain::Track(_)) => {
                self.chain = Some(Chain::Track(Some(at)));
                Step::Consumed("Next point (Enter to end tracking):")
            }
            Some(Chain::From(None)) => {
                self.chain = Some(Chain::From(Some(at)));
                Step::Consumed("<Offset>:")
            }
            Some(Chain::Mid(None)) => {
                self.chain = Some(Chain::Mid(Some(at)));
                Step::Consumed("Second point of mid:")
            }
            Some(Chain::From(Some(_))) => {
                self.chain = None;
                self.temps.clear();
                Step::Done(at)
            }
            Some(Chain::Mid(Some(first))) => {
                self.chain = None;
                self.temps.clear();
                Step::Done(Point::new((first.x + at.x) / 2.0, (first.y + at.y) / 2.0))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn typed_words_name_snaps_by_three_letters_or_more() {
        let only = |k| Some(Wish::Snap(SnapOverride::Only(k)));
        assert_eq!(parse("end"), only(vec![SnapKind::Endpoint]));
        assert_eq!(parse(" ENDP "), only(vec![SnapKind::Endpoint]));
        assert_eq!(
            parse("mid,cen"),
            only(vec![SnapKind::Midpoint, SnapKind::Center])
        );
        assert_eq!(parse("per"), only(vec![SnapKind::Perpendicular]));
        assert_eq!(parse("int"), only(vec![SnapKind::Intersection]));
        assert_eq!(parse("ins"), only(vec![SnapKind::Insertion]));
        assert_eq!(parse("ext"), only(vec![SnapKind::Extension]));
        assert_eq!(parse("par"), only(vec![SnapKind::Parallel]));
        assert_eq!(parse("gcen"), only(vec![SnapKind::GeometricCenter]));
        assert_eq!(parse("geo"), only(vec![SnapKind::GeometricCenter]));
        assert_eq!(parse("app"), only(vec![SnapKind::ApparentIntersection]));
        assert_eq!(parse("appint"), only(vec![SnapKind::ApparentIntersection]));
        assert_eq!(parse("extint"), only(vec![SnapKind::ExtendedIntersection]));
        assert_eq!(
            parse("extended"),
            only(vec![SnapKind::ExtendedIntersection])
        );
        // "EXT" has always been Extension.
        assert_eq!(parse("ext"), only(vec![SnapKind::Extension]));
        assert_eq!(parse("nod"), only(vec![SnapKind::Node]));
        assert_eq!(parse("non"), Some(Wish::Snap(SnapOverride::None)));
        assert_eq!(parse("fro"), Some(Wish::Chain(Chain::From(None))));
        assert_eq!(parse("m2p"), Some(Wish::Chain(Chain::Mid(None))));
    }

    #[test]
    fn anything_else_is_not_a_snap() {
        for text in ["", "e", "en", "1,2", "@3<4", "mid,", "mid,xyz", "endx", "5"] {
            assert_eq!(parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn an_override_is_spent_by_the_next_point() {
        let mut once = OneShot::default();
        once.begin(parse("end").unwrap());
        assert!(once.over().is_some());
        assert_eq!(once.status().as_deref(), Some("[Endpoint]"));
        assert_eq!(once.point(p(1.0, 1.0)), Step::Pass);
        assert!(once.over().is_none() && once.status().is_none());
    }

    #[test]
    fn from_takes_a_base_then_an_offset() {
        let mut once = OneShot::default();
        once.begin(parse("fro").unwrap());
        assert_eq!(once.asking(), Some("Base point:"));
        assert_eq!(once.origin(), None);
        assert_eq!(once.point(p(10.0, 10.0)), Step::Consumed("<Offset>:"));
        // Typed offsets measure from the base, and the band starts there.
        assert_eq!(once.origin(), Some(p(10.0, 10.0)));
        assert_eq!(once.rubber_base(), Some(p(10.0, 10.0)));
        assert_eq!(once.point(p(15.0, 12.0)), Step::Done(p(15.0, 12.0)));
        assert_eq!(once.asking(), None);
    }

    #[test]
    fn the_base_of_from_can_use_a_snap_but_each_point_spends_it() {
        let mut once = OneShot::default();
        once.begin(parse("fro").unwrap());
        once.begin(parse("cen").unwrap());
        assert!(once.over().is_some(), "the base point is snapped");
        once.point(p(0.0, 0.0));
        assert!(once.over().is_none(), "but the offset is not");
        assert_eq!(once.asking(), Some("<Offset>:"));
    }

    #[test]
    fn m2p_gives_the_middle_of_two_points() {
        let mut once = OneShot::default();
        once.begin(parse("m2p").unwrap());
        assert_eq!(
            once.point(p(0.0, 0.0)),
            Step::Consumed("Second point of mid:")
        );
        assert_eq!(once.rubber_base(), Some(p(0.0, 0.0)));
        assert_eq!(once.point(p(10.0, 4.0)), Step::Done(p(5.0, 2.0)));
        assert_eq!(once.status(), None);
    }

    #[test]
    fn a_deferred_perpendicular_is_the_foot_from_the_next_point() {
        let line = Entity::Line {
            start: p(0.0, 0.0),
            end: p(100.0, 0.0),
        };
        let d = Deferred {
            kind: SnapKind::Perpendicular,
            object: EntityId(0),
            near: p(5.0, 0.0),
        };
        assert_eq!(d.point(&line, p(40.0, 25.0)), Some(p(40.0, 0.0)));
        assert_eq!(
            d.point(&line, p(140.0, 25.0)),
            None,
            "beyond the line's end"
        );
        let mut once = OneShot::default();
        once.begin(parse("per").unwrap());
        once.defer(d);
        assert!(once.over().is_none(), "the override was spent on the pick");
        assert_eq!(once.asking(), Some("Specify the other point:"));
        assert!(once
            .status()
            .unwrap()
            .starts_with("[Deferred Perpendicular]"));
        once.settle(p(40.0, 0.0));
        assert_eq!(once.asking(), None);
        assert_eq!(once.take_first(), Some(p(40.0, 0.0)));
        assert_eq!(once.take_first(), None, "once");
    }

    #[test]
    fn a_deferred_tangent_takes_the_tangent_point_nearest_the_pick() {
        let circle = Entity::Circle {
            center: p(0.0, 0.0),
            radius: 5.0,
        };
        let at = |near| Deferred {
            kind: SnapKind::Tangent,
            object: EntityId(0),
            near,
        };
        let upper = at(p(2.0, 4.5)).point(&circle, p(13.0, 0.0)).unwrap();
        let lower = at(p(2.0, -4.5)).point(&circle, p(13.0, 0.0)).unwrap();
        assert!(upper.y > 4.0 && lower.y < -4.0, "{upper:?} {lower:?}");
        assert_eq!(at(p(5.0, 0.0)).point(&circle, p(1.0, 0.0)), None, "inside");
    }

    #[test]
    fn a_temporary_tracking_point_is_kept_for_the_point_that_follows() {
        let mut once = OneShot::default();
        once.begin(parse("tt").unwrap());
        assert_eq!(once.asking(), Some("Specify temporary tracking point:"));
        assert!(matches!(once.point(p(5.0, 5.0)), Step::Consumed(_)));
        assert_eq!(once.temps(), [p(5.0, 5.0)]);
        assert_eq!(once.asking(), None, "the next point is the answer");
        // Another one may follow.
        once.begin(parse("tt").unwrap());
        once.point(p(9.0, 9.0));
        assert_eq!(once.temps().len(), 2);
        assert_eq!(once.point(p(1.0, 1.0)), Step::Pass);
        assert!(once.temps().is_empty(), "spent with the point");
    }

    #[test]
    fn tk_chains_points_until_enter_and_gives_the_last() {
        let mut once = OneShot::default();
        assert_eq!(once.finish_track(), None);
        once.begin(parse("tk").unwrap());
        assert!(once.tracking_chain());
        assert_eq!(once.asking(), Some("First tracking point:"));
        assert!(matches!(once.point(p(0.0, 0.0)), Step::Consumed(_)));
        assert_eq!(once.origin(), Some(p(0.0, 0.0)));
        assert!(matches!(once.point(p(5.0, 0.0)), Step::Consumed(_)));
        assert_eq!(once.rubber_base(), Some(p(5.0, 0.0)));
        assert_eq!(once.finish_track(), Some(p(5.0, 0.0)));
        assert!(!once.tracking_chain());
        assert_eq!(once.asking(), None);
    }

    #[test]
    fn clear_forgets_everything() {
        let mut once = OneShot::default();
        once.begin(parse("fro").unwrap());
        once.point(p(1.0, 1.0));
        once.clear();
        assert_eq!(once.asking(), None);
        assert_eq!(once.point(p(2.0, 2.0)), Step::Pass);
    }
}
