//! The editor window with its Paragraph dialog open over the text.

use eframe::egui::{self, pos2, vec2, Event, PointerButton, RawInput, Rect};
use od_canvas::{Camera, CanvasStyle};
use od_core::{MText, Point, TextStyle, TextStyleId};
use od_ui::Theme;

use super::{show, MTextEditor};

fn style(theme: &Theme) -> CanvasStyle {
    let c = &theme.colors;
    CanvasStyle {
        background: c.canvas_bg,
        grid_minor: c.text,
        grid_major: c.text,
        axis_x: c.text,
        axis_y: c.text,
        entity: c.text,
        hover: c.accent,
        selected: c.accent,
        selected_glow: c.accent,
        snap: c.accent,
        window: c.accent,
        crossing: c.accent,
    }
}

const CANVAS: Rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(900.0, 700.0));

/// An editor over the middle of the canvas, tall enough to sit under where
/// a dialog opens, and the context to run it in.
struct Session {
    ctx: egui::Context,
    theme: Theme,
    looks: CanvasStyle,
    editor: MTextEditor,
}

impl Session {
    fn new() -> Self {
        let ctx = egui::Context::default();
        let theme = Theme::default();
        theme.apply(&ctx);
        let looks = style(&theme);
        let contents = r"line\P".repeat(12) + "end";
        let mut text = MText::new(
            Point::new(50.0, 160.0),
            8.0,
            &contents,
            TextStyleId::STANDARD,
        );
        text.reference_width = 200.0;
        let editor = MTextEditor::new(text, TextStyle::standard());
        Self {
            ctx,
            theme,
            looks,
            editor,
        }
    }

    fn frame(&mut self, events: Vec<Event>) {
        let input = RawInput {
            screen_rect: Some(CANVAS),
            events,
            ..RawInput::default()
        };
        let camera = Camera::default();
        let _ = self.ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show(
                    ui,
                    CANVAS,
                    &camera,
                    &self.looks,
                    &self.theme,
                    &mut self.editor,
                    &[],
                );
            });
        });
    }

    fn button(at: egui::Pos2, button: PointerButton, pressed: bool) -> Event {
        Event::PointerButton {
            pos: at,
            button,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }
}

#[test]
fn a_click_on_the_paragraph_dialog_does_not_move_the_caret_under_it() {
    let mut s = Session::new();
    s.editor.paragraph_dialog = true;
    s.frame(Vec::new());
    s.frame(Vec::new());
    let dialog = s
        .ctx
        .memory(|m| m.area_rect(egui::Id::new("Paragraph")))
        .expect("the dialog is open");
    let at = dialog.center();
    let before = s.editor.selection();
    s.frame(vec![Event::PointerMoved(at)]);
    s.frame(vec![Session::button(at, PointerButton::Primary, true)]);
    s.frame(vec![Session::button(at, PointerButton::Primary, false)]);
    assert_eq!(s.editor.selection(), before);
}

#[test]
fn a_middle_drag_over_the_text_does_not_select_it() {
    let mut s = Session::new();
    s.frame(Vec::new());
    let from = pos2(450.0, 300.0);
    let before = s.editor.selection();
    s.frame(vec![Event::PointerMoved(from)]);
    s.frame(vec![Session::button(from, PointerButton::Middle, true)]);
    for step in 1..=5 {
        let to = from + vec2(30.0 * step as f32, 20.0 * step as f32);
        s.frame(vec![Event::PointerMoved(to)]);
    }
    assert_eq!(s.editor.selection(), before);
}
