//! Selecting by pointer: placing the caret at a point, double and triple click.

use super::MTextEditor;

impl MTextEditor {
    /// Put the caret at the layout point `(x, y)`, extending the selection
    /// when `extend`.
    pub fn place_at(&mut self, x: f64, y: f64, extend: bool) {
        if let Some(i) = self.layout.nearest_caret(x, y) {
            self.preferred_x = None;
            let at = self.pos_of(i);
            self.set_active(at, extend);
        }
    }

    /// Select the word at the layout point.
    pub fn select_word_at(&mut self, x: f64, y: f64) {
        let Some(i) = self.layout.nearest_caret(x, y) else {
            return;
        };
        self.clear_pending();
        let (from, to) = self.layout.word_at(i);
        self.anchor = self.pos_of(from);
        self.active = self.pos_of(to);
        self.preferred_x = None;
        self.history.break_run();
    }

    /// Select the whole line at the layout point.
    pub fn select_line_at(&mut self, x: f64, y: f64) {
        let Some(i) = self.layout.nearest_caret(x, y) else {
            return;
        };
        self.clear_pending();
        self.anchor = self.pos_of(self.layout.line_start(i));
        self.active = self.pos_of(self.layout.line_end(i));
        self.preferred_x = None;
        self.history.break_run();
    }
}
