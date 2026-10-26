//! A memory of recent layouts, so asking for the same text's geometry again
//! (every bounds query and every repaint does) costs a lookup, not a shaping
//! pass. It only remembers answers; the answer for a given text and style is
//! the same with or without it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::{layout_text, TextLayout};
use crate::mtext::MText;
use crate::text_style::{FontDescriptor, TextStyle};

/// Entries kept before the memory is emptied and started again.
const CAPACITY: usize = 512;

/// Everything layout reads, and nothing it does not (not the position, the
/// direction or the attachment, so moving text keeps its layout).
#[derive(PartialEq, Eq, Hash)]
struct Key {
    contents: String,
    numbers: [u64; 7],
    codes: [u8; 3],
    font: FontDescriptor,
}

impl Key {
    fn new(text: &MText, style: &TextStyle) -> Self {
        Self {
            contents: text.contents.clone(),
            numbers: [
                text.nominal_height.to_bits(),
                text.reference_width.to_bits(),
                text.reference_height.to_bits(),
                text.line_spacing.factor.to_bits(),
                style.width_factor.to_bits(),
                style.oblique_radians.to_bits(),
                style.stroke_weight.to_bits(),
            ],
            codes: [
                text.line_spacing.style.code(),
                text.flow.code() | u8::from(style.vertical) << 7,
                text.columns.kind as u8,
            ],
            font: style.font.clone(),
        }
    }
}

thread_local! {
    static MEMORY: RefCell<HashMap<Key, Rc<TextLayout>>> = RefCell::new(HashMap::new());
}

pub(crate) fn layout(text: &MText, style: &TextStyle) -> Rc<TextLayout> {
    let key = Key::new(text, style);
    MEMORY.with(|memory| {
        if let Some(found) = memory.borrow().get(&key) {
            return Rc::clone(found);
        }
        let made = Rc::new(layout_text(text, style));
        let mut memory = memory.borrow_mut();
        if memory.len() >= CAPACITY {
            memory.clear();
        }
        memory.insert(key, Rc::clone(&made));
        made
    })
}
