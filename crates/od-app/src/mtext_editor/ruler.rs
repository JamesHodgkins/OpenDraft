//! The ruler above the editor's box: the paragraph's first-line and left
//! indent markers and its tab stops, drawn in the text's own direction and
//! dragged to change them. It only asks the editor for a `ParagraphEdit`.

use eframe::egui::{self, Pos2, Rect, Shape, Stroke, Vec2};
use od_core::text::{ParagraphEdit, ParagraphStyle};
use od_ui::Theme;

use super::state::MTextEditor;

/// Gap between the box and the ruler, and the ruler's height, in pixels.
const GAP: f32 = 3.0;
const STRIP: f32 = 22.0;
/// How near a marker a press must be to take it.
const GRAB_PX: f32 = 6.0;
/// A tab dragged this far off the ruler is removed.
const REMOVE_PX: f32 = 40.0;
/// The most ticks drawn; a ruler needing more is left bare.
const MAX_TICKS: f64 = 4000.0;

/// What a drag holds.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Grab {
    First,
    Left,
    /// The handle at the right end that sets the wrapping width.
    Width,
    /// A tab stop (`None` while it is dragged off to be removed).
    Tab(Option<f64>),
}

/// A nice interval (1, 2 or 5 times a power of ten) at least `min_px` wide.
fn tick_step(px_per_unit: f64, min_px: f64) -> f64 {
    let wanted = min_px / px_per_unit.max(1e-9);
    let power = 10f64.powf(wanted.log10().floor());
    [1.0, 2.0, 5.0, 10.0]
        .iter()
        .map(|m| m * power)
        .find(|s| *s >= wanted)
        .unwrap_or(10.0 * power)
}

/// `value` on a grid a tenth of `step` wide, not below zero.
fn snap(value: f64, step: f64) -> f64 {
    let grid = step / 10.0;
    let snapped = (value / grid).round() * grid;
    (snapped * 1e6).round() / 1e6
}

fn decimals(step: f64) -> usize {
    if step >= 1.0 {
        0
    } else {
        (-step.log10()).ceil() as usize
    }
}

/// The marker (if any) at local `x`, `upper` when the press is in the upper
/// half of the strip.
fn grab_at(
    x: f64,
    upper: bool,
    px_per_unit: f64,
    style: &ParagraphStyle,
    width: f64,
) -> Option<Grab> {
    let near = |at: f64| (x - at).abs() * px_per_unit <= f64::from(GRAB_PX);
    if near(width) {
        return Some(Grab::Width);
    }
    if upper {
        return near((style.left_indent + style.first_indent).max(0.0)).then_some(Grab::First);
    }
    style
        .tabs
        .iter()
        .find(|t| near(t.offset))
        .map(|t| Grab::Tab(Some(t.offset)))
        .or_else(|| near(style.left_indent).then_some(Grab::Left))
}

/// The ruler's place on screen: local x along the text's own direction.
struct Strip {
    origin: Pos2,
    ux: Vec2,
    uy: Vec2,
    px_per_unit: f32,
}

impl Strip {
    fn point(&self, x: f64, h: f32) -> Pos2 {
        self.origin + self.ux * (x as f32 * self.px_per_unit) + self.uy * h
    }

    /// Local x and height above the box for a screen position.
    fn local(&self, p: Pos2) -> (f64, f32) {
        let d = p - self.origin;
        (f64::from(d.dot(self.ux) / self.px_per_unit), d.dot(self.uy))
    }
}

/// What a drag changes.
#[derive(Debug, PartialEq)]
enum Change {
    Paragraph(ParagraphEdit),
    Width(f64),
}

/// What a drag at local `x` and height `h` asks for.
fn drag_change(
    grab: Grab,
    x: f64,
    h: f32,
    style: &ParagraphStyle,
    step: f64,
    min_width: f64,
) -> (Change, Grab) {
    let at = snap(x, step).max(0.0);
    let paragraph = |edit| Change::Paragraph(edit);
    match grab {
        Grab::Width => (Change::Width(at.max(min_width)), grab),
        Grab::First => {
            let edit = ParagraphEdit {
                first_indent: Some(at - style.left_indent),
                ..ParagraphEdit::default()
            };
            (paragraph(edit), grab)
        }
        Grab::Left => {
            // The first line keeps its distance from the margin but not
            // before the text's left edge.
            let left = at.max(-style.first_indent);
            let edit = ParagraphEdit {
                left_indent: Some(left),
                ..ParagraphEdit::default()
            };
            (paragraph(edit), grab)
        }
        Grab::Tab(current) => {
            let far = (h - (GAP + STRIP / 2.0)).abs() > REMOVE_PX;
            let want = (!far).then_some(at);
            let mut tabs: Vec<f64> = style.tabs.iter().map(|t| t.offset).collect();
            if let Some(c) = current {
                tabs.retain(|t| (t - c).abs() > 1e-9);
            }
            tabs.extend(want);
            let edit = ParagraphEdit {
                tabs: Some(tabs),
                ..ParagraphEdit::default()
            };
            (paragraph(edit), Grab::Tab(want))
        }
    }
}

/// Draw the ruler for the box whose top-left corner is `corner` on screen
/// (`local(x, y)` gives screen points) and handle its input. Returns the
/// screen rectangle it covers.
pub fn show(
    ui: &egui::Ui,
    theme: &Theme,
    editor: &mut MTextEditor,
    local_to_screen: &dyn Fn(f64, f64) -> Pos2,
    top: f64,
    x_range: (f64, f64),
    now: f64,
) -> Rect {
    let ctx = ui.ctx();
    let zero = local_to_screen(0.0, top);
    let across = local_to_screen(1.0, top) - zero;
    let up = local_to_screen(0.0, top + 1.0) - zero;
    let px_per_unit = across.length().max(1e-6);
    let strip = Strip {
        origin: zero,
        ux: across / px_per_unit,
        uy: up.normalized(),
        px_per_unit,
    };
    let (x0, x1) = x_range;
    let corners = [
        strip.point(x0, GAP),
        strip.point(x1, GAP),
        strip.point(x1, GAP + STRIP),
        strip.point(x0, GAP + STRIP),
    ];
    let area = Rect::from_points(&corners);
    let view = editor.paragraph();
    let style = view.style.clone();
    let min_width = editor.text().nominal_height;
    let wrap = |editor: &MTextEditor| match editor.text().reference_width {
        w if w > 0.0 => w,
        _ => editor.layout().frame_width,
    };
    let step = tick_step(f64::from(px_per_unit), 48.0);

    // Input: a press takes a marker (or adds a tab stop), a drag moves it.
    let id = egui::Id::new("mtext_ruler_grab");
    let (pressed, down, pointer) = ctx.input(|i| {
        (
            i.pointer.primary_pressed(),
            i.pointer.primary_down(),
            i.pointer.interact_pos(),
        )
    });
    let mut grab: Option<Grab> = ctx.data(|d| d.get_temp(id)).filter(|_| down);
    if let (false, true, Some(pos)) = (view.protected, pressed, pointer) {
        let (x, h) = strip.local(pos);
        let inside = area.contains(pos) && (GAP..=GAP + STRIP).contains(&h);
        if inside && (x0..=x1).contains(&x) {
            let upper = h > GAP + STRIP / 2.0;
            grab = grab_at(x, upper, f64::from(px_per_unit), &style, wrap(editor)).or_else(|| {
                let at = snap(x, step).max(0.0);
                let mut tabs: Vec<f64> = style.tabs.iter().map(|t| t.offset).collect();
                tabs.push(at);
                let edit = ParagraphEdit {
                    tabs: Some(tabs),
                    ..ParagraphEdit::default()
                };
                editor
                    .set_paragraph(&edit, false, now)
                    .then_some(Grab::Tab(Some(at)))
            });
            editor.active_since = now;
        }
    } else if let (Some(held), Some(pos)) = (grab, pointer) {
        let (x, h) = strip.local(pos);
        let (change, next) = drag_change(held, x, h, &style, step, min_width);
        match change {
            Change::Paragraph(edit) => editor.set_paragraph(&edit, true, now),
            Change::Width(width) => editor.set_width(width, true, now),
        };
        grab = Some(next);
    }
    ctx.data_mut(|d| match grab {
        Some(g) => d.insert_temp(id, g),
        None => d.remove::<Grab>(id),
    });
    if grab.is_some() {
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
    }
    // Paint from the state after the input.
    let view = editor.paragraph();
    let style = view.style;
    let painter = ui.painter();
    let weak = theme.colors.text_weak;
    painter.add(Shape::convex_polygon(
        corners.to_vec(),
        theme.colors.panel_bg,
        Stroke::new(1.0_f32, theme.colors.border),
    ));
    let minor = step / 10.0;
    if (x1 - x0) / minor <= MAX_TICKS {
        let mut k = (x0 / minor).ceil() as i64;
        while (k as f64) * minor <= x1 {
            let x = k as f64 * minor;
            let major = k % 10 == 0;
            let tall = if major {
                9.0
            } else if k % 5 == 0 {
                6.0
            } else {
                3.0
            };
            painter.line_segment(
                [strip.point(x, GAP), strip.point(x, GAP + tall)],
                Stroke::new(1.0_f32, weak),
            );
            if major {
                painter.text(
                    strip.point(x, GAP + 10.0) + egui::vec2(2.0, 0.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{x:.*}", decimals(step)),
                    egui::FontId::proportional(9.0),
                    weak,
                );
            }
            k += 1;
        }
    }
    // The width handle: a bar across the strip at the wrapping edge.
    let edge = wrap(editor);
    painter.line_segment(
        [strip.point(edge, GAP), strip.point(edge, GAP + STRIP)],
        Stroke::new(3.0_f32, theme.colors.accent),
    );
    if view.protected {
        let rect = area;
        ui.interact(
            rect,
            egui::Id::new("mtext_ruler_protected"),
            egui::Sense::hover(),
        )
        .on_hover_text("This paragraph's formatting cannot be edited yet");
        return area;
    }
    let colour = theme.colors.accent;
    let mid = GAP + STRIP / 2.0;
    let first = (style.left_indent + style.first_indent).max(0.0);
    let triangle = |x: f64, base: f32, apex: f32| {
        Shape::convex_polygon(
            vec![
                strip.point(x, base) - strip.ux * 4.0,
                strip.point(x, base) + strip.ux * 4.0,
                strip.point(x, apex),
            ],
            colour,
            Stroke::NONE,
        )
    };
    painter.add(triangle(first, GAP + STRIP - 1.0, mid));
    painter.add(triangle(style.left_indent, GAP + 1.0, mid));
    for tab in &style.tabs {
        let stroke = Stroke::new(2.0_f32, theme.colors.text);
        painter.line_segment(
            [
                strip.point(tab.offset, GAP + 2.0),
                strip.point(tab.offset, mid),
            ],
            stroke,
        );
        painter.line_segment(
            [
                strip.point(tab.offset, GAP + 2.0),
                strip.point(tab.offset, GAP + 2.0) + strip.ux * 6.0,
            ],
            stroke,
        );
    }
    ui.interact(area, egui::Id::new("mtext_ruler_tip"), egui::Sense::hover())
        .on_hover_text(
            "Drag the markers to set the first-line and left indents.\n\
             Click the ruler to add a tab stop; drag one away to remove it.",
        );
    area
}

#[cfg(test)]
mod tests {
    use super::*;
    use od_core::text::{TabKind, TabStop};

    fn style(first: f64, left: f64, tabs: &[f64]) -> ParagraphStyle {
        ParagraphStyle {
            first_indent: first,
            left_indent: left,
            tabs: tabs
                .iter()
                .map(|&offset| TabStop {
                    offset,
                    kind: TabKind::Left,
                    decimal: '.',
                })
                .collect(),
            ..ParagraphStyle::default()
        }
    }

    fn drag_edit(
        grab: Grab,
        x: f64,
        h: f32,
        style: &ParagraphStyle,
        step: f64,
    ) -> (ParagraphEdit, Grab) {
        match drag_change(grab, x, h, style, step, 1.0) {
            (Change::Paragraph(edit), next) => (edit, next),
            other => panic!("not a paragraph change: {other:?}"),
        }
    }

    #[test]
    fn the_width_handle_wins_at_the_edge_and_never_goes_below_the_minimum() {
        let s = style(0.0, 0.0, &[]);
        assert_eq!(grab_at(100.2, true, 10.0, &s, 100.0), Some(Grab::Width));
        assert_eq!(grab_at(100.2, false, 10.0, &s, 100.0), Some(Grab::Width));
        assert_eq!(grab_at(50.0, false, 10.0, &s, 100.0), None);
        let (change, next) = drag_change(Grab::Width, 40.04, 10.0, &s, 5.0, 2.5);
        assert_eq!((change, next), (Change::Width(40.0), Grab::Width));
        let (change, _) = drag_change(Grab::Width, 0.2, 10.0, &s, 5.0, 2.5);
        assert_eq!(change, Change::Width(2.5));
    }

    #[test]
    fn ticks_use_one_two_or_five_and_never_crowd() {
        assert_eq!(tick_step(10.0, 48.0), 5.0);
        assert_eq!(tick_step(30.0, 48.0), 2.0);
        assert_eq!(tick_step(100.0, 48.0), 0.5);
        assert_eq!(tick_step(48.0, 48.0), 1.0);
        for ppu in [0.5, 3.0, 17.0, 240.0, 5000.0] {
            let step = tick_step(ppu, 48.0);
            assert!(step * ppu >= 48.0 - 1e-9, "{ppu}");
            assert!(step * ppu < 48.0 * 2.5 + 1e-9, "{ppu}");
        }
    }

    #[test]
    fn values_snap_to_a_tenth_of_a_tick() {
        assert_eq!(snap(3.04, 1.0), 3.0);
        assert_eq!(snap(3.06, 1.0), 3.1);
        assert_eq!(snap(2.19, 2.0), 2.2);
        assert_eq!(snap(0.30000000000000004, 0.5), 0.3);
        assert_eq!(decimals(5.0), 0);
        assert_eq!(decimals(0.5), 1);
        assert_eq!(decimals(0.02), 2);
    }

    #[test]
    fn a_press_takes_the_marker_under_it() {
        let s = style(-4.0, 4.0, &[2.0, 9.0]);
        // 10 px per unit: 6 px is 0.6 of a unit.
        assert_eq!(grab_at(0.3, true, 10.0, &s, 100.0), Some(Grab::First));
        assert_eq!(grab_at(4.2, false, 10.0, &s, 100.0), Some(Grab::Left));
        assert_eq!(
            grab_at(2.1, false, 10.0, &s, 100.0),
            Some(Grab::Tab(Some(2.0)))
        );
        assert_eq!(grab_at(6.0, false, 10.0, &s, 100.0), None);
        assert_eq!(
            grab_at(4.0, true, 10.0, &s, 100.0),
            None,
            "first line is at 0"
        );
        // A tab on top of the left marker is taken first.
        let both = style(0.0, 2.0, &[2.0]);
        assert_eq!(
            grab_at(2.0, false, 10.0, &both, 100.0),
            Some(Grab::Tab(Some(2.0)))
        );
    }

    #[test]
    fn dragging_moves_a_marker_and_keeps_the_rest_valid() {
        let s = style(-4.0, 4.0, &[]);
        let (e, _) = drag_edit(Grab::First, 1.04, 15.0, &s, 1.0);
        assert_eq!(e.first_indent, Some(-3.0), "absolute 1.0 less the left 4");
        let (e, _) = drag_edit(Grab::First, -3.0, 15.0, &s, 1.0);
        assert_eq!(e.first_indent, Some(-4.0), "not before the left edge");
        // The left marker cannot pass the first line's reach to the edge.
        let (e, _) = drag_edit(Grab::Left, 1.0, 15.0, &s, 1.0);
        assert_eq!(e.left_indent, Some(4.0));
        let (e, _) = drag_edit(Grab::Left, 6.0, 15.0, &s, 1.0);
        assert_eq!(e.left_indent, Some(6.0));
    }

    #[test]
    fn a_tab_moves_and_is_removed_when_dragged_away() {
        let s = style(0.0, 0.0, &[2.0, 6.0]);
        let (e, next) = drag_edit(Grab::Tab(Some(2.0)), 8.0, 15.0, &s, 1.0);
        assert_eq!(e.tabs, Some(vec![6.0, 8.0]));
        assert_eq!(next, Grab::Tab(Some(8.0)));
        let (e, next) = drag_edit(Grab::Tab(Some(2.0)), 8.0, 90.0, &s, 1.0);
        assert_eq!(e.tabs, Some(vec![6.0]));
        assert_eq!(next, Grab::Tab(None));
        // Brought back, it returns.
        let after = style(0.0, 0.0, &[6.0]);
        let (e, _) = drag_edit(Grab::Tab(None), 3.0, 15.0, &after, 1.0);
        assert_eq!(e.tabs, Some(vec![6.0, 3.0]));
    }
}
