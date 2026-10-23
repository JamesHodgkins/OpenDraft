//! The random operations of the editor stress tests: a seedable generator,
//! the contents it makes, and the operations it applies. See
//! `stress_tests.rs`.

use od_core::text::{CharEdit, ListKind, ParagraphAlignment, ParagraphEdit, TextColor};

use super::*;

/// xorshift64*: small, seedable and the same everywhere.
pub(super) struct Rng(u64);

impl Rng {
    pub(super) fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub(super) fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub(super) fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    pub(super) fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    pub(super) fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }

    pub(super) fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Pieces of contents, well formed and not, that a hostile file could hold.
pub(super) const FRAGMENTS: &[&str] = &[
    "a",
    "b",
    "xyz",
    " ",
    "  ",
    "word ",
    "\t",
    "\\P",
    "{",
    "}",
    "{\\L",
    "\\L",
    "\\l",
    "\\O",
    "\\K",
    "\\H2;",
    "\\H0.5x;",
    "\\W2;",
    "\\T3;",
    "\\Q15;",
    "\\C1;",
    "\\c255;",
    "\\A1;",
    "\\S1/2;",
    "\\S+0.1^-0.2;",
    "\\S1#2;",
    "\\S;",
    "\\pxi-3,l3,t3;",
    "\\pxqc;",
    "\\pxqr;",
    "\\pxqj;",
    "\\pxl2;",
    "\\pxqd;",
    "\\pxt1,2,3;",
    "\\fArial|b1|i1;",
    "\\fSymbol|b0|i0|c2|p18;",
    "\\U+00E9",
    "\\U+D83D\\U+DE00",
    "\\U+ZZ",
    "\\~",
    "\\\\",
    "\\{",
    "\\}",
    "\\",
    "\\N",
    "\\X",
    "\\z",
    "%<\\AcVar Date>%",
    "%<",
    ">%",
    "é",
    "漢字",
    "م",
    "1.\t",
    "A.\t",
    "\u{b7}",
    "{\\Lab}",
    "{{{{{{{{{{",
    "}}}}}}}}}}",
    "\\H;",
    "\\H-1;",
    "\\Hinf;",
    ";",
];

pub(super) fn hostile_contents(rng: &mut Rng, pieces: usize) -> String {
    (0..pieces).map(|_| *rng.pick(FRAGMENTS)).collect()
}

#[derive(Clone, Debug)]
pub(super) enum Op {
    Insert(String),
    Newline,
    Backspace(bool),
    Delete(bool),
    Move(Move, bool),
    SelectAll,
    Place(f64, f64, bool),
    SelectWord(f64, f64),
    SelectLine(f64, f64),
    Format(CharEdit),
    Drag(CharEdit),
    Stack,
    Paragraph(ParagraphEdit),
    List(Option<ListKind>),
    Width(f64),
    Undo,
    Redo,
    Cut,
    Paste(String),
}

pub(super) fn typed(rng: &mut Rng) -> String {
    let chars = [
        "a", "b", "c", " ", "  ", "1/2", "x^y", "{", "}", "\\", ";", "é", "漢", "م", "\t", "%<",
        "A.",
    ];
    (0..1 + rng.below(6)).map(|_| *rng.pick(&chars)).collect()
}

fn random_char_edit(rng: &mut Rng) -> CharEdit {
    let mut edit = CharEdit::default();
    for _ in 0..1 + rng.below(3) {
        match rng.below(9) {
            0 => edit.bold = Some(rng.chance(50)),
            1 => edit.italic = Some(rng.chance(50)),
            2 => edit.underline = Some(rng.chance(50)),
            3 => edit.overline = Some(rng.chance(50)),
            4 => edit.strike = Some(rng.chance(50)),
            5 => edit.height = Some(0.1 + rng.unit() * 20.0),
            6 => edit.tracking = Some(0.1 + rng.unit() * 9.0),
            7 => edit.width = Some(0.1 + rng.unit() * 9.0),
            _ => {
                edit.color = Some(if rng.chance(50) {
                    TextColor::Aci(1 + rng.below(250) as u16)
                } else {
                    TextColor::Entity
                });
            }
        }
    }
    if rng.chance(10) {
        edit.oblique = Some(rng.unit() * 170.0 - 85.0);
    }
    edit
}

fn random_paragraph_edit(rng: &mut Rng) -> ParagraphEdit {
    let mut edit = ParagraphEdit::default();
    if rng.chance(60) {
        edit.first_indent = Some(rng.unit() * 20.0 - 10.0);
    }
    if rng.chance(60) {
        edit.left_indent = Some(rng.unit() * 20.0);
    }
    if rng.chance(40) {
        edit.alignment = Some(*rng.pick(&[
            ParagraphAlignment::Left,
            ParagraphAlignment::Center,
            ParagraphAlignment::Right,
            ParagraphAlignment::Justified,
        ]));
    }
    if rng.chance(40) {
        edit.tabs = Some((0..rng.below(6)).map(|_| rng.unit() * 30.0).collect());
    }
    edit
}

pub(super) fn random_op(rng: &mut Rng) -> Op {
    let moves = [
        Move::Left,
        Move::Right,
        Move::WordLeft,
        Move::WordRight,
        Move::Up,
        Move::Down,
        Move::Home,
        Move::End,
        Move::DocStart,
        Move::DocEnd,
    ];
    let (x, y) = (rng.unit() * 60.0 - 5.0, -rng.unit() * 30.0 + 3.0);
    match rng.below(100) {
        0..=21 => Op::Insert(typed(rng)),
        22..=26 => Op::Newline,
        27..=33 => Op::Backspace(rng.chance(20)),
        34..=38 => Op::Delete(rng.chance(20)),
        39..=56 => Op::Move(*rng.pick(&moves), rng.chance(40)),
        57..=59 => Op::SelectAll,
        60..=62 => Op::Place(x, y, rng.chance(30)),
        63 => Op::SelectWord(x, y),
        64 => Op::SelectLine(x, y),
        65..=71 => Op::Format(random_char_edit(rng)),
        72..=74 => Op::Drag(random_char_edit(rng)),
        75..=77 => Op::Stack,
        78..=81 => Op::Paragraph(random_paragraph_edit(rng)),
        82..=84 => Op::List(*rng.pick(&[
            None,
            Some(ListKind::Bullet),
            Some(ListKind::Number),
            Some(ListKind::LowerLetter),
            Some(ListKind::UpperLetter),
        ])),
        85..=86 => Op::Width(0.5 + rng.unit() * 80.0),
        87..=91 => Op::Undo,
        92..=94 => Op::Redo,
        95..=96 => Op::Cut,
        _ => Op::Paste(typed(rng) + &typed(rng)),
    }
}

pub(super) fn apply(e: &mut MTextEditor, op: &Op, now: f64) {
    match op {
        Op::Insert(t) => drop(e.insert(t, now)),
        Op::Newline => drop(e.insert("\n", now)),
        Op::Backspace(word) => drop(e.backspace(*word, now)),
        Op::Delete(word) => drop(e.delete(*word, now)),
        Op::Move(to, extend) => e.move_caret(*to, *extend),
        Op::SelectAll => e.select_all(),
        Op::Place(x, y, extend) => e.place_at(*x, *y, *extend),
        Op::SelectWord(x, y) => e.select_word_at(*x, *y),
        Op::SelectLine(x, y) => e.select_line_at(*x, *y),
        Op::Format(edit) => drop(e.set_format(*edit, now)),
        Op::Drag(edit) => drop(e.adjust_format(*edit, now)),
        Op::Stack => drop(e.toggle_stack(now)),
        Op::Paragraph(edit) => drop(e.set_paragraph(edit, false, now)),
        Op::List(kind) => drop(e.set_list(*kind, now)),
        Op::Width(w) => drop(e.set_width(*w, false, now)),
        Op::Undo => drop(e.undo()),
        Op::Redo => drop(e.redo()),
        Op::Cut => drop(e.cut_text(now)),
        Op::Paste(t) => drop(e.paste(t, now)),
    }
}
