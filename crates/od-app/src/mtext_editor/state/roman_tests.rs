//! Tests of OD Roman pen weights in the editor, and of editing a Select All
//! over and over.

use od_core::text::{CharEdit, Mixed};
use od_core::TextStyle;

use super::formatting_tests::bold_edit;
use super::tests::{caret_at, editor};
use super::*;

fn roman_editor(contents: &str) -> MTextEditor {
    let text = MText::new(
        od_core::Point::new(0.0, 0.0),
        2.0,
        contents,
        od_core::TextStyleId::STANDARD,
    );
    let style = TextStyle {
        font: od_core::FontDescriptor::OdRoman {
            bold: false,
            italic: false,
        },
        ..TextStyle::standard()
    };
    MTextEditor::new(text, style)
}

#[test]
fn a_weight_set_on_a_selection_is_written_for_od_roman_and_drawn() {
    let mut e = roman_editor("hello");
    assert!(e.uses_stroke_font());
    assert_eq!(e.style_weight(), 0.10);
    caret_at(&mut e, 1);
    for _ in 0..3 {
        e.move_caret(Move::Right, true);
    }
    let edit = CharEdit {
        weight: Some(250),
        ..CharEdit::default()
    };
    assert!(e.set_format(edit, 0.0));
    assert_eq!(e.contents(), "h{\\fOD Roman|b0|i0|w0.25;ell}o");
    assert_eq!(e.format().weight, Mixed::Uniform(Some(250)));
    let weights: Vec<_> = e.layout().glyphs.iter().map(|g| g.weight).collect();
    assert_eq!(weights, [100, 250, 250, 250, 100]);
    assert!(e.undo());
    assert_eq!(e.contents(), "hello");
}

#[test]
fn a_pending_weight_applies_to_the_next_typed_text() {
    let mut e = roman_editor("ab");
    let edit = CharEdit {
        weight: Some(300),
        ..CharEdit::default()
    };
    e.set_format(edit, 1.0);
    assert_eq!(e.format().weight, Mixed::Uniform(Some(300)));
    e.insert("cd", 1.1);
    assert_eq!(e.contents(), "ab{\\fOD Roman|b0|i0|w0.3;cd}");
    assert_eq!(e.layout().glyphs[2].weight, 300);
}

#[test]
fn the_other_font_does_not_offer_a_weight() {
    assert!(!editor("x").uses_stroke_font());
}

/// A drag sends a change every frame; the text must not grow with each one.
#[test]
fn dragging_a_value_does_not_stack_a_control_per_frame() {
    let mut e = roman_editor("HELLO WORLD");
    e.select_all();
    let mut t = 0.0;
    for i in 0..1500u16 {
        t += 0.016;
        let edit = CharEdit {
            weight: Some(10 + i % 390),
            ..CharEdit::default()
        };
        assert!(e.adjust_format(edit, t), "step {i}: {:?}", e.notice);
    }
    assert!(e.contents().len() < 60, "{}", e.contents());
    assert_eq!(e.contents().matches('{').count(), 1);
}

/// Select All takes in a group's braces; editing again and again must reuse
/// that group, not nest a new one each time.
#[test]
fn select_all_and_format_over_and_over_never_nests_or_refuses() {
    let mut e = roman_editor("HELLO WORLD");
    let mut t = 0.0;
    for i in 0..40u16 {
        t += 1.0;
        e.select_all();
        let edit = CharEdit {
            weight: Some(20 + i),
            ..CharEdit::default()
        };
        assert!(e.adjust_format(edit, t), "round {i}: {:?}", e.notice);
        let edit = CharEdit {
            height: Some(1.0 + f64::from(i)),
            ..CharEdit::default()
        };
        assert!(e.set_format(edit, t), "round {i}: {:?}", e.notice);
    }
    assert_eq!(e.contents().matches('{').count(), 1, "{}", e.contents());
    assert_eq!(e.format().weight, Mixed::Uniform(Some(59)));
}

/// Random sessions of the operations a person mixes while formatting; a
/// format applied to a Select All is never refused.
#[test]
fn random_formatting_after_select_all_is_never_refused() {
    let mut seed = 12345u64;
    let mut next = move |n: u64| {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) % n
    };
    for session in 0..400 {
        let mut e = roman_editor("HELLO WORLD");
        let mut log: Vec<String> = Vec::new();
        let mut t = 0.0;
        for _ in 0..60 {
            t += 0.016 * (1 + next(40)) as f64;
            let weight = CharEdit {
                weight: Some(10 + next(390) as u16),
                ..CharEdit::default()
            };
            let height = CharEdit {
                height: Some(1.0 + next(100) as f64 / 10.0),
                ..CharEdit::default()
            };
            match next(9) {
                0 => {
                    e.select_all();
                    log.push(format!("select_all, weight {:?}", weight.weight));
                    assert!(
                        e.adjust_format(weight, t),
                        "session {session}: {:?}
{}
{}",
                        e.notice,
                        log.join(
                            "
"
                        ),
                        e.contents()
                    );
                }
                1 => {
                    e.select_all();
                    log.push("select_all, bold".into());
                    assert!(
                        e.set_format(bold_edit(), t),
                        "session {session}: {:?}
{}
{}",
                        e.notice,
                        log.join(
                            "
"
                        ),
                        e.contents()
                    );
                }
                2 => {
                    e.adjust_format(weight, t);
                    log.push("weight".into());
                }
                3 => {
                    e.adjust_format(height, t);
                    log.push("height".into());
                }
                4 => {
                    e.undo();
                    log.push("undo".into());
                }
                5 => {
                    e.redo();
                    log.push("redo".into());
                }
                6 => {
                    e.insert("ab", t);
                    log.push("insert".into());
                }
                7 => {
                    e.move_caret(Move::Home, false);
                    log.push("home".into());
                }
                _ => {
                    e.move_caret(Move::End, false);
                    log.push("end".into());
                }
            }
        }
    }
}
