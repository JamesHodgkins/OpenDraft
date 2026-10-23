//! Moving, turning, enlarging and editing a multileader.

use super::entity::{MLeader, MLeaderContent};
use crate::grip::{Grip, GripKind};
use crate::{MText, Point};

impl MLeader {
    fn map_points(&self, f: impl Fn(Point) -> Point) -> MLeader {
        MLeader {
            lines: self
                .lines
                .iter()
                .map(|line| line.iter().map(|p| f(*p)).collect())
                .collect(),
            ..self.clone()
        }
    }

    fn map_text(mut self, f: impl Fn(&MText) -> MText) -> MLeader {
        if let MLeaderContent::MText(t) = &mut self.content {
            t.text = f(&t.text);
        }
        self
    }

    pub fn translated(&self, dx: f64, dy: f64) -> MLeader {
        self.map_points(|p| p.translated(dx, dy))
            .map_text(|t| t.translated(dx, dy))
    }

    pub fn rotated(&self, center: Point, angle: f64) -> MLeader {
        let origin = Point::new(0.0, 0.0);
        MLeader {
            landing_dir: self.landing_dir.rotated(origin, angle),
            ..self.map_points(|p| p.rotated(center, angle))
        }
        .map_text(|t| t.rotated(center, angle))
    }

    /// Enlarged by `factor` about `base`: points, sizes and text.
    pub fn scaled(&self, base: Point, factor: f64) -> MLeader {
        let mut scaled = self
            .map_points(|p| p.scaled(base, factor))
            .map_text(|t| t.scaled(base, factor));
        let look = &mut scaled.look;
        look.arrow_size *= factor;
        look.dogleg_length *= factor;
        look.landing_gap *= factor;
        scaled
    }

    /// The reflection in the line through `a` and `b`. The text is moved to
    /// its reflected place and never itself reversed; callers ask
    /// [`MLeader::mirror_refusal`] first.
    pub fn mirrored(&self, a: Point, b: Point) -> MLeader {
        let origin = Point::new(0.0, 0.0).mirrored(a, b);
        let dir = self.landing_dir.mirrored(a, b);
        MLeader {
            landing_dir: Point::new(dir.x - origin.x, dir.y - origin.y),
            ..self.map_points(|p| p.mirrored(a, b))
        }
        .map_text(|t| {
            let to = t.insertion.mirrored(a, b);
            t.translated(to.x - t.insertion.x, to.y - t.insertion.y)
        })
    }

    /// Why it cannot be mirrored, if it cannot: its text cannot.
    pub fn mirror_refusal(&self) -> Option<&'static str> {
        self.text()
            .and_then(|t| t.text.mirror_refusal())
            .map(|_| "a multileader with text cannot be mirrored yet")
    }

    /// Why it cannot be scaled, if it cannot.
    pub fn scale_refusal(&self) -> Option<&'static str> {
        self.text().and_then(|t| t.text.scale_refusal())
    }

    /// Every leader point (line by line), then the text's insertion point.
    pub fn grips(&self) -> Vec<Grip> {
        let mut grips: Vec<Grip> = self
            .lines
            .iter()
            .flatten()
            .map(|p| Grip {
                at: *p,
                kind: GripKind::Stretch,
            })
            .collect();
        if let Some(t) = self.text() {
            grips.push(Grip {
                at: t.text.insertion,
                kind: GripKind::Move,
            });
        }
        grips
    }

    /// The multileader with grip `index` dragged to `to`. A point of a
    /// leader line moves alone, except the one the landing leaves, which
    /// takes the landing and the text with it; the text's grip does the same.
    pub fn with_grip_moved(&self, index: usize, to: Point) -> Option<MLeader> {
        let from = self.grips().get(index)?.at;
        let (dx, dy) = (to.x - from.x, to.y - from.y);
        if !(dx.is_finite() && dy.is_finite()) {
            return None;
        }
        let points: usize = self.lines.iter().map(Vec::len).sum();
        let tail = self.lines.first()?.len().checked_sub(1)?;
        if index >= points || index == tail {
            return Some(self.moved_by(|_| false, true, dx, dy));
        }
        let mut moved = self.clone();
        let mut n = index;
        for line in &mut moved.lines {
            if n < line.len() {
                line[n] = line[n].translated(dx, dy);
                break;
            }
            n -= line.len();
        }
        Some(moved)
    }

    /// The multileader with the leader points for which `inside` is true
    /// moved by `(dx, dy)`, and the landing and text with them when the
    /// landing's start or the text is caught; `None` when nothing is.
    pub fn stretched(&self, inside: &impl Fn(Point) -> bool, dx: f64, dy: f64) -> Option<MLeader> {
        let text_caught = self.text().is_some_and(|t| inside(t.text.insertion));
        let tail_caught = self.landing_start().is_some_and(inside);
        let any = self.lines.iter().flatten().any(|p| inside(*p));
        (text_caught || any).then(|| self.moved_by(inside, text_caught || tail_caught, dx, dy))
    }

    /// Points for which `inside` is true moved; with `with_landing`, the
    /// landing's start and the text as well.
    fn moved_by(
        &self,
        inside: impl Fn(Point) -> bool,
        with_landing: bool,
        dx: f64,
        dy: f64,
    ) -> MLeader {
        let mut moved = self.clone();
        for (i, line) in moved.lines.iter_mut().enumerate() {
            let last = line.len().saturating_sub(1);
            for (j, p) in line.iter_mut().enumerate() {
                let is_tail = i == 0 && j == last;
                if inside(*p) || (is_tail && with_landing) {
                    *p = p.translated(dx, dy);
                }
            }
        }
        if with_landing {
            if let MLeaderContent::MText(t) = &mut moved.content {
                t.text = t.text.translated(dx, dy);
            }
        }
        moved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mleader::MLeaderStyle;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn made() -> MLeader {
        let mut style = MLeaderStyle::standard();
        style.max_points = None;
        MLeader::build(
            &style,
            vec![p(0.0, 0.0), p(5.0, 5.0), p(10.0, 10.0)],
            "Note",
        )
        .unwrap()
    }

    fn near(a: Point, b: Point) -> bool {
        (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
    }

    #[test]
    fn moving_and_turning_back_gives_the_same_multileader() {
        let m = made();
        assert_eq!(m.translated(3.0, -4.0).translated(-3.0, 4.0), m);
        let turned = m.rotated(p(1.0, 2.0), 0.7).rotated(p(1.0, 2.0), -0.7);
        for (a, b) in turned.lines[0].iter().zip(&m.lines[0]) {
            assert!(near(*a, *b));
        }
        assert!(near(turned.landing_dir, m.landing_dir));
        let (t, u) = (turned.text().unwrap(), m.text().unwrap());
        assert!(near(t.text.insertion, u.text.insertion));
    }

    #[test]
    fn scaling_multiplies_positions_sizes_and_text_once() {
        let m = made().scaled(p(0.0, 0.0), 2.0);
        assert_eq!(m.lines[0][2], p(20.0, 20.0));
        assert_eq!(m.look.arrow_size, 5.0);
        assert_eq!(m.look.dogleg_length, 16.0);
        assert_eq!(m.text().unwrap().text.nominal_height, 5.0);
        assert!(m.validate().is_ok());
    }

    #[test]
    fn mirroring_flips_the_landing_and_text_is_refused() {
        let m = made();
        assert!(m.mirror_refusal().is_some());
        let mirrored = m.mirrored(p(0.0, 0.0), p(0.0, 1.0));
        assert!(near(mirrored.landing_dir, p(-1.0, 0.0)));
        assert!(near(mirrored.lines[0][2], p(-10.0, 10.0)));
    }

    #[test]
    fn grips_are_the_points_then_the_text() {
        let grips = made().grips();
        assert_eq!(grips.len(), 4);
        assert_eq!(grips[3].kind, GripKind::Move);
    }

    #[test]
    fn dragging_an_arrow_or_a_bend_moves_only_that_point() {
        let m = made();
        let arrow = m.with_grip_moved(0, p(-2.0, 0.0)).unwrap();
        assert_eq!(arrow.lines[0][0], p(-2.0, 0.0));
        assert_eq!(arrow.lines[0][2], m.lines[0][2]);
        assert_eq!(arrow.text(), m.text());
        let bend = m.with_grip_moved(1, p(5.0, 8.0)).unwrap();
        assert_eq!(bend.lines[0][1], p(5.0, 8.0));
        assert_eq!(bend.lines[0][0], m.lines[0][0]);
    }

    #[test]
    fn dragging_the_text_or_the_landing_start_takes_both_with_it() {
        let m = made();
        for index in [2, 3] {
            let at = m.grips()[index].at;
            let moved = m.with_grip_moved(index, p(at.x + 1.0, at.y + 2.0)).unwrap();
            assert_eq!(moved.lines[0][2], p(11.0, 12.0));
            assert_eq!(moved.lines[0][0], m.lines[0][0]);
            let (a, b) = (moved.text().unwrap(), m.text().unwrap());
            assert!(near(
                a.text.insertion,
                p(b.text.insertion.x + 1.0, b.text.insertion.y + 2.0)
            ));
        }
        assert!(m.with_grip_moved(9, p(0.0, 0.0)).is_none());
    }

    #[test]
    fn stretching_moves_what_the_window_catches() {
        let m = made();
        assert!(m.stretched(&|q: Point| q.x < -100.0, 1.0, 0.0).is_none());
        let arrow = m.stretched(&|q: Point| q.x < 1.0, 4.0, 0.0).unwrap();
        assert_eq!(arrow.lines[0][0], p(4.0, 0.0));
        assert_eq!(arrow.lines[0][2], m.lines[0][2]);
        let text = m.text().unwrap().text.insertion;
        let all = m.stretched(&|q: Point| q.x > 9.0, 0.0, 3.0).unwrap();
        assert_eq!(all.lines[0][2], p(10.0, 13.0));
        assert_eq!(all.text().unwrap().text.insertion, p(text.x, text.y + 3.0));
    }
}
