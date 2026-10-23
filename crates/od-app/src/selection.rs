//! Selection commands written against the `Editor`: pick a region by its
//! points, and the objects it covers join (or leave) the selection.

use od_canvas::Region;
use od_commands::{Answer, Cancelled, Editor, GhostStyle, PointOptions};
use od_core::{Entity, Point};

use crate::text::plural;

#[derive(Clone, Copy)]
enum Kind {
    /// A closed polygon; selects what is entirely inside it.
    Window,
    /// A closed polygon; selects what is inside it or touches it.
    Crossing,
    /// An open polyline; selects what it crosses.
    Fence,
}

impl Kind {
    fn first_prompt(self) -> &'static str {
        match self {
            Kind::Fence => "Specify first fence point",
            _ => "Specify first polygon point",
        }
    }

    fn next_prompt(self) -> &'static str {
        match self {
            Kind::Fence => "Specify next fence point (Enter to finish)",
            _ => "Specify next point (Enter to finish)",
        }
    }

    fn style(self) -> GhostStyle {
        match self {
            Kind::Window => GhostStyle::Window,
            _ => GhostStyle::Crossing,
        }
    }

    fn region(self, points: Vec<Point>) -> Region {
        match self {
            Kind::Window => Region::Window(points),
            Kind::Crossing => Region::Crossing(points),
            Kind::Fence => Region::Fence(points),
        }
    }
}

/// WPOLYGON: select what lies entirely inside a polygon.
pub fn window_polygon(ed: &mut Editor) -> Result<(), Cancelled> {
    select_by_points(ed, Kind::Window)
}

/// CPOLYGON: select what lies inside a polygon or touches it.
pub fn crossing_polygon(ed: &mut Editor) -> Result<(), Cancelled> {
    select_by_points(ed, Kind::Crossing)
}

/// FENCE: select what a line through the points crosses.
pub fn fence(ed: &mut Editor) -> Result<(), Cancelled> {
    select_by_points(ed, Kind::Fence)
}

/// Collect points until Enter (or a right click), then select what the
/// region covers. The Remove option switches to taking objects out of the
/// selection. Esc selects nothing.
fn select_by_points(ed: &mut Editor, kind: Kind) -> Result<(), Cancelled> {
    let mut points: Vec<Point> = Vec::new();
    let mut removing = false;
    loop {
        let message = if points.is_empty() {
            kind.first_prompt()
        } else {
            kind.next_prompt()
        };
        let so_far = points.clone();
        let closed = !matches!(kind, Kind::Fence);
        let options = PointOptions::new(message)
            .ghost_style(kind.style())
            .ghost(move |at| {
                let mut path = so_far.clone();
                path.push(at);
                vec![Entity::Polyline {
                    points: path,
                    closed,
                    bulges: Vec::new(),
                    widths: Vec::new(),
                }]
            })
            .keyword(
                if removing { "Add" } else { "Remove" },
                if removing { "A" } else { "R" },
            );
        match ed.ask_point(options)? {
            Answer::Value(at) => {
                if points.last() != Some(&at) {
                    points.push(at);
                }
            }
            Answer::Keyword(_) => removing = !removing,
            Answer::Enter => {
                let region = kind.region(points.clone());
                if points.len() < region.min_points() {
                    ed.say("Specify more points first.");
                    continue;
                }
                let ids = ed.read(move |doc| od_canvas::select_in(doc, &region))?;
                let count = plural(ids.len(), "object");
                if removing {
                    ed.deselect(ids);
                    ed.say(format!("{count} deselected"));
                } else {
                    ed.select(ids);
                    ed.say(format!("{count} selected"));
                }
                return Ok(());
            }
        }
    }
}

impl crate::state::AppState {
    /// Apply a canvas click. Selection is sticky: a plain click adds the
    /// entity, shift-click removes it, and clicking empty space changes
    /// nothing (Esc deselects everything).
    pub fn click_select(&mut self, entity: Option<od_core::EntityId>, remove: bool) {
        let selection = &mut self.docs[self.active].selection;
        match (entity, remove) {
            (Some(i), false) => {
                selection.insert(i);
            }
            (Some(i), true) => {
                selection.remove(&i);
            }
            (None, _) => {}
        }
    }

    /// Select every selectable object of the same kind and layer as one that
    /// is selected.
    pub fn select_similar(&mut self) {
        let doc = &mut self.docs[self.active];
        let document = &doc.session.document;
        let kinds: Vec<_> = doc
            .selection
            .iter()
            .filter_map(|id| document.object(*id))
            .map(|o| (std::mem::discriminant(&o.entity), o.attrs.layer))
            .collect();
        let similar: Vec<_> = document
            .selectable_objects()
            .filter(|o| kinds.contains(&(std::mem::discriminant(&o.entity), o.attrs.layer)))
            .map(|o| o.id)
            .collect();
        doc.selection.extend(similar);
    }
}

#[cfg(test)]
mod tests {
    use od_core::EntityId;

    use super::*;
    use crate::state::AppState;

    fn circle_at(x: f64) -> Entity {
        Entity::Circle {
            center: Point::new(x, 0.0),
            radius: 1.0,
        }
    }

    /// A drawing with circles at x = 0, 10 and 100.
    fn state() -> AppState {
        let mut state = AppState::new();
        for x in [0.0, 10.0, 100.0] {
            state.add_entity(circle_at(x));
        }
        state
    }

    fn selected(state: &AppState) -> Vec<f64> {
        let document = &state.session().document();
        state
            .selection()
            .iter()
            .filter_map(|id| document.get(*id))
            .filter_map(|e| match e {
                Entity::Circle { center, .. } => Some(center.x),
                _ => None,
            })
            .collect()
    }

    fn run(
        state: &mut AppState,
        command: fn(&mut Editor) -> Result<(), Cancelled>,
        inputs: &[&str],
    ) {
        state.start_command("Test", command);
        state.settle_command();
        for input in inputs {
            state.cmd_submit(input);
            state.settle_command();
        }
    }

    #[test]
    fn a_window_polygon_selects_what_is_entirely_inside() {
        let mut state = state();
        // Covers the circles at 0 and 10 but only half of nothing else.
        run(
            &mut state,
            window_polygon,
            &["-5,-5", "15,-5", "15,5", "-5,5", ""],
        );
        assert!(!state.command_running());
        assert_eq!(selected(&state), [0.0, 10.0]);
        assert!(state.message.contains("2 objects selected"));
    }

    #[test]
    fn a_crossing_polygon_also_takes_what_it_touches() {
        let mut state = state();
        // Window would miss the circle at 10 (only partly inside).
        run(
            &mut state,
            crossing_polygon,
            &["-5,-5", "10,-5", "10,5", "-5,5", ""],
        );
        assert_eq!(selected(&state), [0.0, 10.0]);
    }

    #[test]
    fn a_fence_selects_what_it_crosses() {
        let mut state = state();
        run(&mut state, fence, &["-5,0", "105,0", ""]);
        assert_eq!(selected(&state), [0.0, 10.0, 100.0]);
    }

    #[test]
    fn too_few_points_wait_for_more() {
        let mut state = state();
        run(&mut state, window_polygon, &["0,0", "5,0", ""]);
        assert!(state.command_running());
        assert!(state.cmdline.log.last().unwrap().contains("more points"));
        assert!(state.selection().is_empty());
    }

    #[test]
    fn the_remove_option_takes_objects_out_of_the_selection() {
        let mut state = state();
        state.select_all();
        run(
            &mut state,
            window_polygon,
            &["r", "-5,-5", "15,-5", "15,5", "-5,5", ""],
        );
        assert_eq!(selected(&state), [100.0]);
        assert!(state.message.contains("deselected"));
    }

    #[test]
    fn select_similar_takes_the_same_kind_on_the_same_layer() {
        let mut state = state();
        state.add_entity(Entity::Line {
            start: Point::new(0.0, 5.0),
            end: Point::new(1.0, 5.0),
        });
        let first = state
            .session()
            .document()
            .selectable_objects()
            .next()
            .map(|o| o.id);
        state.select_ids(first, false);
        state.select_similar();
        assert_eq!(selected(&state), [0.0, 10.0, 100.0]);
        assert_eq!(state.selection().len(), 3);
    }

    #[test]
    fn clicks_place_the_points_and_right_click_finishes() {
        let mut state = state();
        state.start_command("Fence", fence);
        state.settle_command();
        assert!(state.docs[0].view.is_prompting());
        for (x, y) in [(-5.0, 0.0), (105.0, 0.0)] {
            state.answer_point(Point::new(x, y));
            state.settle_command();
        }
        state.answer_enter();
        state.settle_command();
        assert_eq!(selected(&state).len(), 3);
    }

    #[test]
    fn escape_selects_nothing() {
        let mut state = state();
        run(&mut state, fence, &["-5,0", "105,0"]);
        state.cancel();
        assert!(state.selection().is_empty());
        assert_eq!(state.cmdline.log.last().unwrap(), "*Cancel*");
    }

    #[test]
    fn selecting_adds_to_what_is_already_selected() {
        let mut state = state();
        let first = state.session().document().objects()[0].id;
        state.select_ids([first], false);
        run(&mut state, fence, &["95,0", "105,0", ""]);
        assert_eq!(selected(&state), [0.0, 100.0]);
        let _: EntityId = first;
    }
}
