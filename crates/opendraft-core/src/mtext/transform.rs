//! Moving, turning and enlarging multiline text, and what it refuses.

use super::{MText, MTextColumns};
use crate::text::{self, Format, NodeKind, OpaqueReason};
use crate::Point;

impl MText {
    pub fn translated(&self, dx: f64, dy: f64) -> MText {
        MText {
            insertion: self.insertion.translated(dx, dy),
            ..self.clone()
        }
    }

    pub fn rotated(&self, center: Point, angle: f64) -> MText {
        let origin = Point::new(0.0, 0.0);
        MText {
            insertion: self.insertion.rotated(center, angle),
            direction: self.direction.rotated(origin, angle),
            ..self.clone()
        }
    }

    /// The text enlarged by `factor` about `base`: its position, height,
    /// width, column sizes and any absolute heights in its contents. Ratios
    /// (width factor, spacing, relative heights) are unchanged.
    pub fn scaled(&self, base: Point, factor: f64) -> MText {
        let columns = &self.columns;
        MText {
            insertion: self.insertion.scaled(base, factor),
            nominal_height: self.nominal_height * factor,
            reference_width: self.reference_width * factor,
            reference_height: self.reference_height * factor,
            contents: scaled_contents(&self.contents, factor),
            columns: MTextColumns {
                width: columns.width * factor,
                gutter: columns.gutter * factor,
                heights: columns.heights.iter().map(|h| h * factor).collect(),
                ..columns.clone()
            },
            ..self.clone()
        }
    }

    /// Why this text cannot be mirrored, if it cannot. The text must not be
    /// reversed, and the drawing has no MIRRTEXT setting to decide it by.
    pub fn mirror_refusal(&self) -> Option<&'static str> {
        Some("multiline text cannot be mirrored yet")
    }
    /// Why this text cannot be scaled, if it cannot: a paragraph control
    /// this build cannot decode holds distances that could not be scaled.
    pub fn scale_refusal(&self) -> Option<&'static str> {
        let parsed = text::parse(&self.contents).ok()?;
        let opaque = NodeKind::Opaque(OpaqueReason::ParagraphControl);
        parsed
            .nodes
            .iter()
            .any(|n| n.kind == opaque)
            .then_some("multiline text with paragraph formatting cannot be scaled yet")
    }
}

/// The contents with every absolute `\H` height multiplied by `factor` and
/// everything else byte for byte as it was.
fn scaled_contents(contents: &str, factor: f64) -> String {
    let Ok(parsed) = text::parse(contents) else {
        return contents.to_owned();
    };
    let mut out = String::with_capacity(contents.len());
    for node in &parsed.nodes {
        match &node.kind {
            NodeKind::Format(Format::Height(h)) if !h.relative => {
                out.push_str(&format!("\\H{};", h.value * factor));
            }
            NodeKind::Paragraph(style) => {
                let mut scaled = style.clone();
                scaled.first_indent *= factor;
                scaled.left_indent *= factor;
                for tab in &mut scaled.tabs {
                    tab.offset *= factor;
                }
                out.push_str(&text::paragraph_control(&scaled));
            }
            _ => out.push_str(&contents[node.source.clone()]),
        }
    }
    out
}
