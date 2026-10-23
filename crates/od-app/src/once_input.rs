//! The part of answering a prompt that belongs to one-shot snaps: typed
//! `END`, `FRO`... at a point prompt, and a point that is only one part of a
//! `FRO` or `M2P`.

use od_canvas::SnapPoint;
use od_commands::{Prompt as Ask, Reply, Wants};
use od_core::units::parse_length;
use od_core::Point;

use crate::prompt_reply::{keyword_reply, takes_clicked_points};
use crate::snap_once::{parse, Deferred, Step, Wish};
use crate::state::AppState;

impl AppState {
    /// Start a one-shot snap, and say so in the command line.
    pub fn begin_one_shot(&mut self, wish: Wish) {
        let said = match &wish {
            Wish::Snap(over) => format!("{} of", over.label()),
            Wish::Chain(_) => String::new(),
        };
        self.once.begin(wish);
        let said = self.once.asking().map_or(said, str::to_owned);
        self.cmdline.say(said);
    }

    /// A click on a Perpendicular or Tangent that had no last point to work
    /// from: hold it until the next point. Returns whether the click was taken.
    pub(crate) fn defer_one_shot(&mut self, snap: Option<SnapPoint>) -> bool {
        let Some((snap, object)) = snap
            .filter(|s| s.deferred)
            .and_then(|s| Some((s, s.source[0]?)))
        else {
            return false;
        };
        let first_point = self
            .waiting()
            .is_some_and(|a| a.wants == Wants::Point && a.base.is_none());
        if !first_point {
            self.cmdline
                .say(format!("{} needs a point to work from.", snap.kind.label()));
            return true;
        }
        let kind = snap.kind;
        self.once.defer(Deferred {
            kind,
            object,
            near: snap.point,
        });
        self.cmdline.say(format!(
            "Deferred {}: specify the other point.",
            kind.label().to_lowercase()
        ));
        true
    }

    /// Where a waiting deferred snap puts its point when the next point is
    /// `from`, for the rubber band to start at.
    pub(crate) fn deferred_point(&self, from: Point) -> Option<Point> {
        let d = self.once.deferred()?;
        let entity = self
            .docs
            .get(self.run_doc)?
            .session
            .document()
            .get(d.object)?;
        d.point(entity, from)
    }

    /// The point `n` along the one tracking or polar path the pointer
    /// follows, from where the path starts, in the pointer's direction along
    /// it. `None` when it follows no single path.
    pub(crate) fn track_point_at(&self, n: f64) -> Option<Point> {
        let snap = self.snap_now?;
        let [Some(guide), None] = snap.guides else {
            return None;
        };
        let side = (snap.point.x - guide.origin.x) * guide.dir.x
            + (snap.point.y - guide.origin.y) * guide.dir.y;
        let n = if side < 0.0 { -n } else { n };
        Some(Point::new(
            guide.origin.x + guide.dir.x * n,
            guide.origin.y + guide.dir.y * n,
        ))
    }

    /// A distance typed at a point prompt while the pointer follows a path.
    pub(crate) fn track_distance(&self, ask: &Ask, text: &str) -> Option<Point> {
        if ask.wants != Wants::Point {
            return None;
        }
        self.track_point_at(parse_length(text)?)
    }

    /// A click was turned down because the one-shot override found no snap:
    /// say so and let go of it, so the next click places a point as usual
    /// instead of leaving the prompt stuck.
    pub fn refuse_one_shot(&mut self) {
        if let Some(over) = self.once.drop_override() {
            let said = format!("No {} snap found. Specify the point again.", over.label());
            self.cmdline.say(said);
        }
    }

    /// Typed text that names a one-shot snap, at a prompt that takes points,
    /// starts it. A word the prompt itself offers as an option is the
    /// option's. Returns whether the text was taken.
    pub(crate) fn take_one_shot(&mut self, ask: &Ask, text: &str) -> bool {
        let chains_too = matches!(
            ask.wants,
            Wants::Point | Wants::AngleOrPoint | Wants::DistanceOrPoint
        );
        if !takes_clicked_points(ask) || keyword_reply(ask, text.trim()).is_some() {
            return false;
        }
        match parse(text) {
            Some(Wish::Chain(_)) if !chains_too => false,
            Some(wish) => {
                self.begin_one_shot(wish);
                true
            }
            None => false,
        }
    }

    /// What a reply becomes when a one-shot is waiting. `None`: it was only
    /// one part of a chain (the next part is asked for). Any answer spends
    /// the override; an answer that is not a point also ends a chain.
    pub(crate) fn intercept(&mut self, reply: Reply) -> Option<Reply> {
        let Reply::Point(at) = reply else {
            // Enter ends `TK`, and its last point is the answer.
            let last = self.once.finish_track().filter(|_| reply == Reply::Enter);
            self.once.clear();
            return Some(last.map_or(reply, Reply::Point));
        };
        if let Some(d) = self.once.deferred() {
            return match self.deferred_point(at) {
                Some(first) => {
                    self.once.settle(first);
                    Some(Reply::Point(at))
                }
                None => {
                    self.cmdline.say(format!(
                        "No {} from that point. Specify another point.",
                        d.kind.label().to_lowercase()
                    ));
                    None
                }
            };
        }
        match self.once.point(at) {
            Step::Pass => Some(Reply::Point(at)),
            Step::Done(point) => Some(Reply::Point(point)),
            Step::Consumed(next) => {
                self.cmdline.say(next);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use od_canvas::{SnapKind, SnapPoint};
    use od_core::{Entity, EntityId, Point};

    use crate::draw::draw_line;
    use crate::state::AppState;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn lines(state: &AppState) -> Vec<(Point, Point)> {
        let objects = state.session().document().objects();
        objects
            .iter()
            .filter_map(|o| match o.entity {
                Entity::Line { start, end } => Some((start, end)),
                _ => None,
            })
            .collect()
    }

    fn submit(state: &mut AppState, text: &str) {
        state.cmd_submit(text);
        state.settle_command();
    }

    /// A drawing with one line along the x axis (id 0), and a Line command
    /// waiting for its first point.
    fn start_over_a_line() -> AppState {
        let mut state = AppState::new();
        state.perform(od_commands::AddEntity::new(Entity::Line {
            start: p(0.0, 0.0),
            end: p(100.0, 0.0),
        }));
        state.start_command("Line", draw_line);
        state.settle_command();
        state
    }

    /// The snap point the canvas reports for a click near `at` on object 0.
    fn deferred(kind: SnapKind, at: Point) -> Option<SnapPoint> {
        let mut snap = SnapPoint::new(at, kind);
        snap.source[0] = Some(EntityId(0));
        snap.deferred = true;
        Some(snap)
    }

    fn start() -> AppState {
        let mut state = AppState::new();
        state.start_command("Line", draw_line);
        state.settle_command();
        state
    }

    #[test]
    fn typing_a_snap_keeps_the_prompt_and_is_spent_by_the_next_point() {
        let mut state = start();
        submit(&mut state, "end");
        assert!(state.once.over().is_some());
        assert!(
            state.waiting().is_some(),
            "still asking for the first point"
        );
        assert!(state.command_status().unwrap().ends_with("[Endpoint]"));
        state.answer_point(p(3.0, 4.0));
        state.settle_command();
        assert!(state.once.over().is_none(), "one point only");
        assert_eq!(state.waiting().and_then(|a| a.base), Some(p(3.0, 4.0)));
    }

    #[test]
    fn a_new_override_replaces_the_waiting_one_and_a_command_end_clears_it() {
        let mut state = start();
        submit(&mut state, "end");
        submit(&mut state, "mid,cen");
        assert_eq!(state.once.status().as_deref(), Some("[Midpoint, Center]"));
        state.cancel();
        assert!(state.once.over().is_none());
    }

    #[test]
    fn from_adds_an_offset_to_a_base_point() {
        let mut state = start();
        submit(&mut state, "fro");
        assert!(state.command_status().unwrap().contains("Base point"));
        submit(&mut state, "10,10");
        assert!(
            state.waiting().and_then(|a| a.base).is_none(),
            "line not started"
        );
        assert_eq!(state.once.rubber_base(), Some(p(10.0, 10.0)));
        // The offset measures from the base, so this is (15, 10).
        submit(&mut state, "@5,0");
        assert_eq!(state.waiting().and_then(|a| a.base), Some(p(15.0, 10.0)));
        assert!(state.once.asking().is_none());
        submit(&mut state, "@0,5");
        assert_eq!(lines(&state), [(p(15.0, 10.0), p(15.0, 15.0))]);
    }

    #[test]
    fn m2p_starts_the_line_at_the_middle_of_two_points() {
        let mut state = start();
        submit(&mut state, "m2p");
        submit(&mut state, "0,0");
        submit(&mut state, "10,4");
        assert_eq!(state.waiting().and_then(|a| a.base), Some(p(5.0, 2.0)));
        submit(&mut state, "9,9");
        assert_eq!(lines(&state), [(p(5.0, 2.0), p(9.0, 9.0))]);
    }

    #[test]
    fn a_refused_click_says_so_and_releases_the_override() {
        let mut state = start();
        submit(&mut state, "end");
        state.refuse_one_shot();
        assert!(state.once.over().is_none());
        assert!(state
            .cmdline
            .log
            .last()
            .unwrap()
            .contains("No Endpoint snap found"));
        assert!(state.waiting().is_some(), "still asking for the point");
        // With nothing waiting there is nothing to say.
        let before = state.cmdline.log.len();
        state.refuse_one_shot();
        assert_eq!(state.cmdline.log.len(), before);
    }

    #[test]
    fn enter_in_the_middle_of_a_chain_drops_it() {
        let mut state = start();
        submit(&mut state, "fro");
        submit(&mut state, "");
        assert!(state.once.asking().is_none());
    }

    #[test]
    fn a_word_the_prompt_offers_is_the_prompts_not_a_snap() {
        let mut state = start();
        submit(&mut state, "5,5");
        // The Line prompt now offers Undo (and, with two segments, Close).
        submit(&mut state, "u");
        assert!(state.once.over().is_none());
        // "per" is not an option of Line, so it is a snap.
        submit(&mut state, "per");
        assert!(state.once.over().is_some());
    }

    #[test]
    fn coordinates_and_other_words_are_not_snaps() {
        let mut state = start();
        submit(&mut state, "1,2");
        assert!(state.once.over().is_none());
        assert!(state.waiting().and_then(|a| a.base) == Some(p(1.0, 2.0)));
    }

    #[test]
    fn a_deferred_perpendicular_makes_the_line_square_to_the_object() {
        let mut state = start_over_a_line();
        submit(&mut state, "per");
        // The click on the line is held, not given to the command.
        assert!(state.defer_one_shot(deferred(SnapKind::Perpendicular, p(5.0, 0.0))));
        assert!(state.waiting().and_then(|a| a.base).is_none());
        assert!(state
            .command_status()
            .unwrap()
            .contains("[Deferred Perpendicular]"));
        // The rubber band follows the pointer's foot on the line.
        assert_eq!(state.deferred_point(p(40.0, 25.0)), Some(p(40.0, 0.0)));
        // The other point settles it: the line runs from the foot to there.
        state.answer_point(p(40.0, 25.0));
        state.settle_command();
        assert_eq!(state.waiting().and_then(|a| a.base), Some(p(40.0, 25.0)));
        assert!(state.once.deferred().is_none());
        submit(&mut state, "");
        let made = lines(&state);
        assert_eq!(made[1], (p(40.0, 0.0), p(40.0, 25.0)));
    }

    #[test]
    fn a_deferred_perpendicular_with_no_foot_asks_for_another_point() {
        let mut state = start_over_a_line();
        submit(&mut state, "per");
        assert!(state.defer_one_shot(deferred(SnapKind::Perpendicular, p(95.0, 0.0))));
        // From (150, 20) the foot would be beyond the line's end.
        state.answer_point(p(150.0, 20.0));
        state.settle_command();
        assert!(state
            .cmdline
            .log
            .last()
            .unwrap()
            .contains("No perpendicular"));
        assert!(state.once.deferred().is_some(), "still waiting for a point");
        assert!(state.waiting().and_then(|a| a.base).is_none());
        state.answer_point(p(60.0, 20.0));
        state.settle_command();
        assert_eq!(state.waiting().and_then(|a| a.base), Some(p(60.0, 20.0)));
    }

    #[test]
    fn enter_gives_up_a_deferred_snap() {
        let mut state = start_over_a_line();
        assert!(state.defer_one_shot(deferred(SnapKind::Perpendicular, p(5.0, 0.0))));
        submit(&mut state, "");
        assert!(state.once.deferred().is_none());
    }

    #[test]
    fn only_a_deferred_snap_at_a_first_point_is_held() {
        let mut state = start_over_a_line();
        // An ordinary snap is not taken, and a click that is not on a snap.
        assert!(!state.defer_one_shot(Some(SnapPoint::new(p(1.0, 1.0), SnapKind::Endpoint))));
        assert!(!state.defer_one_shot(None));
        // At the next point there is a last point to work from: the canvas
        // finds the real snap, so a deferred one is refused with a word.
        submit(&mut state, "10,10");
        assert!(state.defer_one_shot(deferred(SnapKind::Perpendicular, p(5.0, 0.0))));
        assert!(state.once.deferred().is_none());
        assert!(state.cmdline.log.last().unwrap().contains("needs a point"));
    }

    /// A snap point aligned on a path from `origin` along `dir`, at `point`.
    fn tracking_at(origin: Point, dir: Point, point: Point) -> SnapPoint {
        let mut snap = SnapPoint::new(point, SnapKind::Tracking);
        snap.guides = [
            Some(od_canvas::Guide {
                origin,
                dir,
                kind: Some(SnapKind::Endpoint),
            }),
            None,
        ];
        snap
    }

    #[test]
    fn a_typed_distance_goes_that_far_from_the_start_of_the_path() {
        let mut state = start();
        submit(&mut state, "0,0");
        // Following the horizontal path from (100, 50), pointer to its right.
        state.snap_now = Some(tracking_at(p(100.0, 50.0), p(1.0, 0.0), p(160.0, 50.0)));
        submit(&mut state, "30");
        assert_eq!(lines(&state), [(p(0.0, 0.0), p(130.0, 50.0))]);
        // Pointer to its left: the distance runs that way.
        state.snap_now = Some(tracking_at(p(100.0, 50.0), p(1.0, 0.0), p(40.0, 50.0)));
        submit(&mut state, "30");
        assert_eq!(lines(&state)[1], (p(130.0, 50.0), p(70.0, 50.0)));
    }

    #[test]
    fn a_typed_distance_with_no_single_path_works_as_before() {
        let mut state = start();
        submit(&mut state, "0,0");
        // Two paths (a crossing) or none: the distance follows the cursor.
        let mut cross = tracking_at(p(100.0, 50.0), p(1.0, 0.0), p(160.0, 50.0));
        cross.guides[1] = cross.guides[0];
        state.snap_now = Some(cross);
        state.cursor = Some(p(0.0, 100.0));
        submit(&mut state, "5");
        assert_eq!(lines(&state), [(p(0.0, 0.0), p(0.0, 5.0))]);
    }

    #[test]
    fn tt_tracks_from_a_point_that_is_not_the_answer() {
        let mut state = start();
        submit(&mut state, "tt");
        submit(&mut state, "10,10");
        assert_eq!(state.once.temps(), [p(10.0, 10.0)]);
        assert!(
            state.waiting().and_then(|a| a.base).is_none(),
            "still the first point"
        );
        submit(&mut state, "50,50");
        assert_eq!(state.waiting().and_then(|a| a.base), Some(p(50.0, 50.0)));
        assert!(state.once.temps().is_empty(), "spent with the point");
    }

    #[test]
    fn tk_gives_the_last_tracking_point_on_enter() {
        let mut state = start();
        submit(&mut state, "tk");
        assert!(state.once.tracking_chain());
        submit(&mut state, "0,0");
        submit(&mut state, "30,0");
        submit(&mut state, "30,20");
        assert!(
            state.waiting().and_then(|a| a.base).is_none(),
            "nothing answered yet"
        );
        // A typed point while tracking measures from the last tracking point.
        submit(&mut state, "@5,0");
        submit(&mut state, "");
        assert_eq!(state.waiting().and_then(|a| a.base), Some(p(35.0, 20.0)));
        assert!(!state.once.tracking_chain());
    }
}
