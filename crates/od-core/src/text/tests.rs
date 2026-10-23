use super::*;

fn parsed(source: &str) -> ParsedMText {
    let p = parse(source).unwrap();
    assert_tiles(source, &p);
    p
}

/// Every byte belongs to exactly one node, in order.
fn assert_tiles(source: &str, p: &ParsedMText) {
    let mut at = 0;
    for node in &p.nodes {
        assert_eq!(node.source.start, at, "gap before {node:?} in {source:?}");
        assert!(node.source.end > at, "empty node in {source:?}");
        assert!(source.is_char_boundary(node.source.end), "{source:?}");
        at = node.source.end;
    }
    assert_eq!(
        at,
        source.len(),
        "nodes stop short of the end of {source:?}"
    );
    let mut byte = 0;
    for paragraph in &p.paragraphs {
        assert_eq!(paragraph.source.start, byte);
        byte = paragraph.source.end;
    }
    assert_eq!(byte, source.len());
}

fn kinds(source: &str) -> Vec<NodeKind> {
    parsed(source).nodes.into_iter().map(|n| n.kind).collect()
}

#[test]
fn plain_text_is_one_node_and_one_paragraph() {
    let p = parsed("Hello world");
    assert_eq!(p.nodes.len(), 1);
    assert_eq!(p.paragraphs.len(), 1);
    assert!(p.diagnostics.is_empty());
    assert_eq!(p.plain_text("Hello world"), "Hello world");
}

#[test]
fn empty_contents_is_one_empty_paragraph() {
    let p = parsed("");
    assert!(p.nodes.is_empty());
    assert_eq!(p.paragraphs.len(), 1);
}

#[test]
fn paragraph_breaks_split_and_empty_paragraphs_stay() {
    let src = "one\\Ptwo\\P\\Pfour\\P";
    let p = parsed(src);
    assert_eq!(p.paragraphs.len(), 5, "the last one is empty");
    assert_eq!(&src[p.paragraphs[0].source.clone()], "one\\P");
    assert_eq!(&src[p.paragraphs[2].source.clone()], "\\P");
    assert_eq!(p.paragraphs[4].source, src.len()..src.len());
    assert_eq!(p.plain_text(src), "one\ntwo\n\nfour\n");
}

#[test]
fn character_controls_are_read() {
    use Format::*;
    let k = kinds("\\H2.5;\\W0.8;\\T1.2;\\Q15;\\C1;\\c255;\\A1;\\L\\l\\O\\o\\K\\k");
    let scalar = |value, relative| Scalar { value, relative };
    assert_eq!(
        k,
        [
            NodeKind::Format(Height(scalar(2.5, false))),
            NodeKind::Format(Width(scalar(0.8, false))),
            NodeKind::Format(Tracking(scalar(1.2, false))),
            NodeKind::Format(Oblique(15.0)),
            NodeKind::Format(Aci(1)),
            NodeKind::Format(TrueColor(255)),
            NodeKind::Format(Align(1)),
            NodeKind::Format(Underline(true)),
            NodeKind::Format(Underline(false)),
            NodeKind::Format(Overline(true)),
            NodeKind::Format(Overline(false)),
            NodeKind::Format(Strike(true)),
            NodeKind::Format(Strike(false)),
        ]
    );
    assert_eq!(
        kinds("\\H2x;"),
        [NodeKind::Format(Height(scalar(2.0, true)))]
    );
}

#[test]
fn font_and_stack_payloads_stay_in_the_contents() {
    let src = "\\fArial|b1|i0;x\\S1/2;";
    let p = parsed(src);
    let flags = FontFlags {
        bold: true,
        italic: false,
        weight: None,
    };
    assert_eq!(p.nodes[0].kind, NodeKind::Format(Format::Font(flags)));
    assert_eq!(&src[p.nodes[0].source.clone()], "\\fArial|b1|i0;");
    assert_eq!(p.nodes[2].kind, NodeKind::Stack);
    assert_eq!(p.plain_text(src), "x1/2");
}

#[test]
fn groups_nest_to_eight_and_the_ninth_is_opaque() {
    let nine = format!("{}x{}", "{".repeat(9), "}".repeat(9));
    let p = parsed(&nine);
    let pushes = p.nodes.iter().filter(|n| n.kind == NodeKind::Push).count();
    let pops = p.nodes.iter().filter(|n| n.kind == NodeKind::Pop).count();
    assert_eq!((pushes, pops), (8, 8));
    let deep = NodeKind::Opaque(OpaqueReason::TooDeep);
    assert_eq!(p.nodes.iter().filter(|n| n.kind == deep).count(), 2);
    assert_eq!(p.diagnostics.len(), 1, "reported once, at the open brace");
}

#[test]
fn unmatched_braces_are_diagnosed_not_dropped() {
    let p = parsed("a}b{c");
    assert!(p
        .nodes
        .iter()
        .any(|n| n.kind == NodeKind::Opaque(OpaqueReason::UnmatchedClose)));
    assert!(p
        .diagnostics
        .iter()
        .any(|d| d.kind == DiagnosticKind::UnmatchedOpen));
}

#[test]
fn paragraph_controls_stay_opaque_and_protect_their_paragraph() {
    let src = "\\pxqd,l3;item\\Pplain";
    let p = parsed(src);
    assert_eq!(
        p.nodes[0].kind,
        NodeKind::Opaque(OpaqueReason::ParagraphControl)
    );
    assert_eq!(&src[p.nodes[0].source.clone()], "\\pxqd,l3;");
    assert!(p.paragraphs[0].protected);
    assert!(
        p.paragraphs[1].protected,
        "it inherits what the control says, which is not known"
    );
    assert_eq!(p.paragraphs[0].style, ParagraphStyle::default());
}

#[test]
fn bad_numbers_and_missing_semicolons_become_opaque() {
    for src in [
        "\\Hx;", "\\H-2;", "\\H0;", "\\Hinf;", "\\C999;", "\\A7;", "\\Q5x;",
    ] {
        let k = kinds(src);
        assert_eq!(k, [NodeKind::Opaque(OpaqueReason::BadNumber)], "{src}");
    }
    let src = "\\H2.5 and more text";
    let p = parsed(src);
    assert_eq!(
        p.nodes[0].kind,
        NodeKind::Opaque(OpaqueReason::Unterminated)
    );
    assert_eq!(p.diagnostics.len(), 1);
    // A payload over the numeric cap is not searched for its end.
    let long = format!("\\H{};", "1".repeat(100));
    assert_eq!(
        parsed(&long).nodes[0].kind,
        NodeKind::Opaque(OpaqueReason::Unterminated)
    );
}

#[test]
fn unknown_and_unsupported_escapes_are_kept() {
    assert_eq!(
        kinds("\\z"),
        [NodeKind::Opaque(OpaqueReason::UnknownEscape)]
    );
    assert_eq!(kinds("\\N"), [NodeKind::Opaque(OpaqueReason::Unsupported)]);
    assert_eq!(
        kinds("end\\"),
        [NodeKind::Text, NodeKind::Opaque(OpaqueReason::Unterminated)]
    );
    // A multi-byte character after the backslash is one unit.
    assert_eq!(
        kinds("\\é"),
        [NodeKind::Opaque(OpaqueReason::UnknownEscape)]
    );
}

#[test]
fn escaped_characters_tabs_and_non_breaking_spaces() {
    let src = "a\\\\b\\{c\\}\td\\~e";
    let p = parsed(src);
    assert_eq!(p.plain_text(src), "a\\b{c}\td e");
}

#[test]
fn unicode_escapes_and_surrogate_pairs() {
    let src = "\\U+00E9\\U+D83D\\U+DE00";
    let p = parsed(src);
    assert_eq!(p.nodes[0].kind, NodeKind::Unicode('é'));
    assert_eq!(p.nodes[1].kind, NodeKind::Unicode('😀'));
    assert_eq!(p.plain_text(src), "é😀");
    for bad in ["\\U+D83D", "\\U+DE00", "\\U+12", "\\U+12G4"] {
        let k = kinds(bad);
        assert!(
            matches!(k[0], NodeKind::Opaque(OpaqueReason::BadNumber)),
            "{bad}: {k:?}"
        );
    }
}

#[test]
fn fields_are_kept_whole_and_a_lone_percent_is_text() {
    let src = "a%<\\AcVar Date \\f \"%yyyy\">%b";
    let p = parsed(src);
    assert_eq!(p.nodes[1].kind, NodeKind::Field);
    assert_eq!(&src[p.nodes[1].source.clone()], &src[1..src.len() - 1]);
    assert_eq!(kinds("100%"), [NodeKind::Text]);
    assert_eq!(kinds("%<never closed"), [NodeKind::Text]);
    let nested = "%<a%<b>%c>%";
    assert_eq!(kinds(nested), [NodeKind::Field]);
}

#[test]
fn limits_are_enforced_before_work() {
    let big = "a".repeat(TextLimits::default().max_content_bytes + 1);
    assert_eq!(
        parse(&big).unwrap_err(),
        TextError::ResourceLimit {
            resource: TextResource::ContentBytes,
            limit: 1 << 20
        }
    );
    let few_nodes = TextLimits {
        max_nodes: 3,
        ..TextLimits::default()
    };
    let few_paragraphs = TextLimits {
        max_paragraphs: 2,
        ..TextLimits::default()
    };
    assert!(matches!(
        parse_with("\\L\\l\\L\\l", few_nodes),
        Err(TextError::ResourceLimit {
            resource: TextResource::Nodes,
            ..
        })
    ));
    assert!(matches!(
        parse_with("a\\Pb\\Pc", few_paragraphs),
        Err(TextError::ResourceLimit {
            resource: TextResource::Paragraphs,
            ..
        })
    ));
    // The largest allowed sizes still parse.
    assert!(parse(&"a".repeat(1 << 20)).is_ok());
    let most = "\\P".repeat(99_999);
    assert_eq!(parse(&most).unwrap().paragraphs.len(), 100_000);
    assert!(matches!(
        parse(&"\\P".repeat(100_000)),
        Err(TextError::ResourceLimit {
            resource: TextResource::Paragraphs,
            ..
        })
    ));
}

#[test]
fn hostile_strings_never_panic_or_leave_gaps() {
    const PIECES: &[&str] = &[
        "\\", "{", "}", "%<", ">%", ";", "P", "p", "H", "S", "U+", "D83D", "é", "😀", "\t", "x",
        "1.5", "\\f", "\\H", "\\pxi", "\\U+00", "\\;", "a b",
    ];
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..3000 {
        let count = (next() % 24) as usize;
        let src: String = (0..count)
            .map(|_| PIECES[(next() % PIECES.len() as u64) as usize])
            .collect();
        let p = parse(&src).unwrap();
        assert_tiles(&src, &p);
    }
}

#[test]
fn a_paragraph_control_holds_for_the_paragraphs_after_it() {
    // The shape AutoCAD writes for a list: the control only on the first item.
    let src = r"LIST\P\pxi-3,l3,t3;1.	A\P2.	B\Pplain\P\px;reset";
    let p = parsed(src);
    let left: Vec<f64> = p.paragraphs.iter().map(|q| q.style.left_indent).collect();
    assert_eq!(left, [0.0, 3.0, 3.0, 3.0, 0.0]);
    assert!(p.paragraphs.iter().all(|q| !q.protected));
    assert_eq!(p.paragraphs[2].style.tabs.len(), 1);
}
