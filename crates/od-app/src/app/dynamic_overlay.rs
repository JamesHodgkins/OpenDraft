//! Compact drafting tooltips, sharing stable field IDs across moving areas.
//! `layout` decides where the boxes go, `boxes` paints them; this module
//! gathers what they need and runs them in order.
mod boxes;
mod layout;

use eframe::egui::{self, FontId, Stroke};
use od_core::Point;

use self::boxes::{
    error_box, field_box, keyword_box, options_menu, prompt_box, Around, FieldView, Look, PromptBox,
};
use self::layout::{plan, prompt_text, Inputs, Plan};
use super::dynamic_input::owns_focus;
use super::panels::PanelViewer;
use crate::dynamic_input::{cursor_clearance, Field};

impl PanelViewer<'_> {
    pub(super) fn dynamic_overlay(&mut self, ui: &mut egui::Ui) {
        let settings = self.state.settings.dynamic_input.clone();
        if !settings.enabled()
            || self.dynamic_suppressed
            || self.state.modal_open()
            || self.state.confirm.is_some()
            || self.state.mtext_editing()
        {
            self.state.docs[self.state.active]
                .dynamic_input
                .hit_rects
                .clear();
            return;
        }
        let idle = self.state.dynamic_ask().is_none();
        let Some(ask) = self
            .state
            .dynamic_ask()
            .or_else(|| self.state.idle_pointer_ask())
        else {
            return;
        };
        let prompt = self.state.cmd_prompt();
        let units = self.state.units();
        let readonly = ui.ctx().wants_keyboard_input() && !owns_focus(ui.ctx(), self.state);
        let colors = &self.theme.colors;
        let light = colors.text;
        let look = Look {
            font: FontId::proportional(self.theme.font_size_small),
            light,
            grey: colors.panel_raised.lerp_to_gamma(light, 0.55),
            ink: colors.canvas_bg,
            accent: colors.accent,
            on_accent: colors.text_on_accent,
        };
        let show_prompt = settings.dynamic_prompts && !idle;
        let text = prompt_text(&prompt);
        let text_width = if show_prompt {
            ui.painter()
                .layout_no_wrap(text.clone(), look.font.clone(), look.ink)
                .size()
                .x
        } else {
            0.0
        };
        let doc = &mut self.state.docs[self.state.active];
        let Some(canvas) = doc.dynamic_input.canvas else {
            return;
        };
        let Some(cursor) = doc.dynamic_input.cursor else {
            return;
        };
        let anchor = doc.view.camera.world_to_screen(cursor, canvas);
        let clearance = cursor_clearance(anchor);
        let inputs = Inputs {
            ask: &ask,
            prompt: &prompt,
            settings: &settings,
            units: &units,
            canvas,
            anchor,
            clearance,
            show_prompt,
            text_width,
        };
        let camera = &doc.view.camera;
        let to_screen = |p: Point| camera.world_to_screen(p, canvas);
        let Plan {
            fields,
            ghost,
            prompt_width,
            prompt_rect,
            row,
            rects,
        } = plan(&inputs, &to_screen, &mut doc.dynamic_input);
        let uid = doc.uid;
        let input = &mut doc.dynamic_input;
        if input.editing && fields == [Field::Coordinate] {
            input.active = Some(Field::Coordinate);
        }
        if let Some(ghost) = &ghost {
            ghost.paint(ui.painter(), Stroke::new(1.0_f32, look.grey));
        }
        input.hit_rects.clear();
        if show_prompt {
            input.hit_rects.push(prompt_rect);
        }
        input.hit_rects.extend(rects.iter().map(|(_, r)| *r));
        let ctx = ui.ctx().clone();
        if show_prompt {
            prompt_box(&ctx, uid, input, &look, &prompt, &text, prompt_rect);
        }
        let first = fields.first().copied();
        for (field, rect) in rects {
            let active =
                !readonly && !idle && input.active.unwrap_or(first.unwrap_or(field)) == field;
            let view = FieldView {
                ask: &ask,
                settings: &settings,
                units: &units,
                active,
                idle,
            };
            field_box(&ctx, uid, input, &look, &view, field, rect);
        }
        let around = Around {
            canvas,
            anchor,
            clearance,
            row,
        };
        if input.editing && fields.is_empty() {
            keyword_box(&ctx, uid, input, &look, around);
        }
        let mut chosen = None;
        if input.menu && !prompt.options.is_empty() {
            let under = PromptBox {
                rect: prompt_rect,
                width: prompt_width,
            };
            chosen = options_menu(&ctx, uid, input, &look, &prompt, under, around);
        }
        if let Some(error) = input.error.clone().filter(|_| !input.menu) {
            error_box(&ctx, uid, input, &look, &error, around);
        }
        if let Some(option) = chosen {
            self.state.cmd_choose(option);
        }
    }
}
