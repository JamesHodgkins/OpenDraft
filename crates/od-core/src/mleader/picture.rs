//! What a multileader draws, worked out from its points: the leader lines,
//! their arrowheads, the landing, the text and its frame.

use super::entity::{MLeader, MLeaderContent};
use super::style::MLeaderLineType;
use crate::dimension::arrow_pieces;
use crate::{Bounds, Entity, Point};

fn polyline(points: Vec<Point>) -> Entity {
    Entity::Polyline {
        points,
        closed: false,
        bulges: Vec::new(),
        widths: Vec::new(),
    }
}

impl MLeader {
    /// The pieces that draw it, as ordinary entities. A style with no leader
    /// line type draws the text alone.
    pub fn picture(&self) -> Vec<Entity> {
        let mut pieces = Vec::new();
        if self.look.leader_type != MLeaderLineType::None {
            for line in &self.lines {
                if let [tip, next, ..] = line.as_slice() {
                    let (dx, dy) = (next.x - tip.x, next.y - tip.y);
                    let length = dx.hypot(dy);
                    if length > 1e-12 {
                        pieces.extend(arrow_pieces(
                            self.look.arrow,
                            *tip,
                            (dx / length, dy / length),
                            self.look.arrow_size,
                        ));
                    }
                }
                pieces.push(polyline(line.clone()));
            }
            if let (true, Some(start)) = (self.look.dogleg, self.landing_start()) {
                pieces.push(polyline(vec![start, self.landing_end()]));
            }
        }
        if let MLeaderContent::MText(t) = &self.content {
            if t.frame {
                let b = Entity::MText(Box::new(t.text.clone())).bounds();
                let gap = self.look.landing_gap;
                pieces.push(Entity::Rectangle {
                    min: Point::new(b.min.x - gap, b.min.y - gap),
                    max: Point::new(b.max.x + gap, b.max.y + gap),
                });
            }
            pieces.push(Entity::MText(Box::new(t.text.clone())));
        }
        pieces
    }

    /// The box round everything it draws.
    pub fn bounds(&self) -> Bounds {
        self.bounds_of(&self.picture())
    }

    /// The box round `picture` (what [`MLeader::picture`] returned).
    pub fn bounds_of(&self, picture: &[Entity]) -> Bounds {
        picture
            .iter()
            .map(Entity::bounds)
            .reduce(Bounds::union)
            .unwrap_or_else(|| Bounds::from_points(Point::default(), Point::default()))
    }

    /// Where an object snap can land: every leader point and the end of the
    /// landing.
    pub fn snap_points(&self) -> Vec<Point> {
        let mut points: Vec<Point> = self.lines.iter().flatten().copied().collect();
        if self.look.dogleg {
            points.push(self.landing_end());
        }
        points
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mleader::MLeaderStyle;

    fn made(style: &MLeaderStyle) -> MLeader {
        MLeader::build(
            style,
            vec![Point::new(0.0, 0.0), Point::new(10.0, 10.0)],
            "Note",
        )
        .unwrap()
    }

    fn polylines(pieces: &[Entity]) -> usize {
        pieces
            .iter()
            .filter(|e| matches!(e, Entity::Polyline { .. }))
            .count()
    }

    #[test]
    fn it_draws_an_arrow_a_line_a_landing_and_text() {
        let pieces = made(&MLeaderStyle::standard()).picture();
        assert_eq!(polylines(&pieces), 3);
        assert!(pieces.iter().any(|e| matches!(e, Entity::MText(_))));
    }

    #[test]
    fn a_frame_and_no_landing_change_the_pieces() {
        let mut style = MLeaderStyle::standard();
        style.content.frame = true;
        style.dogleg = false;
        let pieces = made(&style).picture();
        assert!(pieces.iter().any(|e| matches!(e, Entity::Rectangle { .. })));
        assert_eq!(polylines(&pieces), 2);
    }

    #[test]
    fn no_leader_type_draws_only_the_text() {
        let mut style = MLeaderStyle::standard();
        style.leader_type = MLeaderLineType::None;
        assert_eq!(made(&style).picture().len(), 1);
    }

    #[test]
    fn the_bounds_hold_the_arrow_tip_and_the_text() {
        let m = made(&MLeaderStyle::standard());
        let b = m.bounds();
        assert!(b.min.x <= 0.0 && b.min.y <= 0.0);
        let text = Entity::MText(Box::new(m.text().unwrap().text.clone())).bounds();
        assert!(b.max.x >= text.max.x && b.max.y >= text.max.y);
    }

    #[test]
    fn the_snap_points_are_the_leader_points_and_the_landing_end() {
        let m = made(&MLeaderStyle::standard());
        assert_eq!(m.snap_points().len(), 3);
    }
}
