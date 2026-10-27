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

#[test]
fn a_click_on_the_paragraph_dialog_does_not_move_the_caret_under_it() {
    let ctx = egui::Context::default();
    let theme = Theme::default();
    theme.apply(&ctx);
    let canvas = Rect::from_min_size(pos2(0.0, 0.0), vec2(900.0, 700.0));
    let camera = Camera::default();
    let looks = style(&theme);
    // A tall box over the middle of the canvas, where the dialog opens.
    let contents = r"line\P".repeat(12) + "end";
    let mut text = MText::new(
        Point::new(50.0, 160.0),
        8.0,
        &contents,
        TextStyleId::STANDARD,
    );
    text.reference_width = 200.0;
    let mut editor = MTextEditor::new(text, TextStyle::standard());
    editor.paragraph_dialog = true;
    let frame = |events: Vec<Event>, editor: &mut MTextEditor| {
        let input = RawInput {
            screen_rect: Some(canvas),
            events,
            ..RawInput::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                show(ui, canvas, &camera, &looks, &theme, editor, &[]);
            });
        });
    };
    frame(Vec::new(), &mut editor);
    frame(Vec::new(), &mut editor);
    let dialog = ctx
        .memory(|m| m.area_rect(egui::Id::new("Paragraph")))
        .expect("the dialog is open");
    let at = dialog.center();
    let before = editor.selection();
    let button = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(vec![Event::PointerMoved(at)], &mut editor);
    frame(vec![button(true)], &mut editor);
    frame(vec![button(false)], &mut editor);
    assert_eq!(editor.selection(), before);
}
