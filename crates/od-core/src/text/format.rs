//! Character formatting for an editor: what a selection looks like, and
//! changing it.
//!
//! A change wraps the selected text in a `{ }` group that starts with the
//! controls, which is how AutoCAD writes it, so the text after the group is
//! untouched. The editor names what it wants (`CharEdit`); it never writes a
//! control itself.

use std::ops::Range;

use super::charstate::CharState;
use super::edit::{check, dangling, EditRefusal};
use super::fontctl::{font_control, font_parts, FontFlags};
use super::layout::TextColor;
use super::limits::TextLimits;
use super::parse::{parse_with, Format, NodeKind, ParsedMText};
use super::stroke_outline::{MAX_WEIGHT, MIN_WEIGHT};
use group::{inside_groups, pieces, superseded};

/// A value shared by everything selected, or not.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mixed<T> {
    Uniform(T),
    Mixed,
}

impl<T: Copy + PartialEq> Mixed<T> {
    fn fold(self, next: T, same: impl Fn(T, T) -> bool) -> Self {
        match self {
            Self::Uniform(v) if same(v, next) => self,
            _ => Self::Mixed,
        }
    }

    pub fn uniform(self) -> Option<T> {
        match self {
            Self::Uniform(v) => Some(v),
            Self::Mixed => None,
        }
    }
}

/// What a selection (or a caret) looks like.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionFormat {
    pub height: Mixed<f64>,
    pub underline: Mixed<bool>,
    pub overline: Mixed<bool>,
    pub strike: Mixed<bool>,
    pub bold: Mixed<bool>,
    pub italic: Mixed<bool>,
    pub color: Mixed<TextColor>,
    pub oblique: Mixed<f64>,
    pub tracking: Mixed<f64>,
    pub width: Mixed<f64>,
    /// A pen width the text sets for itself (thousandths of the cap height),
    /// or `None` where it follows the style.
    pub weight: Mixed<Option<u16>>,
}

impl SelectionFormat {
    fn of(s: &CharState) -> Self {
        Self {
            height: Mixed::Uniform(s.height),
            underline: Mixed::Uniform(s.underline),
            overline: Mixed::Uniform(s.overline),
            strike: Mixed::Uniform(s.strike),
            bold: Mixed::Uniform(s.bold),
            italic: Mixed::Uniform(s.italic),
            color: Mixed::Uniform(s.color),
            oblique: Mixed::Uniform(s.oblique),
            tracking: Mixed::Uniform(s.tracking),
            width: Mixed::Uniform(s.width),
            weight: Mixed::Uniform(s.weight),
        }
    }

    fn fold(&mut self, s: &CharState) {
        let near = |a: f64, b: f64| (a - b).abs() < 1e-9 * a.abs().max(1.0);
        self.height = self.height.fold(s.height, near);
        self.underline = self.underline.fold(s.underline, |a, b| a == b);
        self.overline = self.overline.fold(s.overline, |a, b| a == b);
        self.strike = self.strike.fold(s.strike, |a, b| a == b);
        self.bold = self.bold.fold(s.bold, |a, b| a == b);
        self.italic = self.italic.fold(s.italic, |a, b| a == b);
        self.color = self.color.fold(s.color, |a, b| a == b);
        self.oblique = self.oblique.fold(s.oblique, near);
        self.tracking = self.tracking.fold(s.tracking, near);
        self.width = self.width.fold(s.width, near);
        self.weight = self.weight.fold(s.weight, |a, b| a == b);
    }

    /// The format with `edit` laid over it (what typing after a pending
    /// change will look like).
    pub fn with(mut self, edit: &CharEdit) -> Self {
        macro_rules! overlay {
            ($($f:ident),*) => { $( if let Some(v) = edit.$f { self.$f = Mixed::Uniform(v); } )* };
        }
        overlay!(
            height, underline, overline, strike, bold, italic, color, oblique, tracking, width
        );
        if let Some(w) = edit.weight {
            self.weight = Mixed::Uniform((w != 0).then_some(w));
        }
        self
    }
}

/// A change to character formatting: what is `Some` is set, the rest left.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CharEdit {
    /// An absolute height in drawing units.
    pub height: Option<f64>,
    pub underline: Option<bool>,
    pub overline: Option<bool>,
    pub strike: Option<bool>,
    /// Bold and italic are written as a font control (the family in effect
    /// is kept).
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub color: Option<TextColor>,
    /// Slant in degrees.
    pub oblique: Option<f64>,
    /// A multiple of the normal letter spacing.
    pub tracking: Option<f64>,
    /// A multiple of the style's width factor.
    pub width: Option<f64>,
    /// A stroke font's pen width in thousandths of the cap height, written
    /// in the font control; 0 hands it back to the style.
    pub weight: Option<u16>,
}

impl CharEdit {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Add `later`'s settings, which win over this one's.
    pub fn merge(&mut self, later: &CharEdit) {
        macro_rules! take {
            ($($f:ident),*) => { $( if later.$f.is_some() { self.$f = later.$f; } )* };
        }
        take!(
            height, underline, overline, strike, bold, italic, color, oblique, tracking, width,
            weight
        );
    }

    fn valid(&self) -> bool {
        let positive = |v: Option<f64>| v.is_none_or(|v| v.is_finite() && v > 0.0);
        positive(self.height)
            && positive(self.tracking)
            && positive(self.width)
            && self
                .oblique
                .is_none_or(|v| v.is_finite() && v.abs() <= 85.0)
            && self.weight.is_none_or(|w| {
                w == 0 || (w as f64) >= MIN_WEIGHT * 1000.0 && (w as f64) <= MAX_WEIGHT * 1000.0
            })
    }

    /// The controls that make these settings, in order; `font` is the font
    /// control for bold and italic, made by the caller.
    fn controls(&self, font: Option<String>) -> String {
        let mut out = font.unwrap_or_default();
        let flag = |out: &mut String, v: Option<bool>, on: &str, off: &str| {
            if let Some(v) = v {
                out.push_str(if v { on } else { off });
            }
        };
        flag(&mut out, self.underline, "\\L", "\\l");
        flag(&mut out, self.overline, "\\O", "\\o");
        flag(&mut out, self.strike, "\\K", "\\k");
        if let Some(h) = self.height {
            out.push_str(&format!("\\H{h};"));
        }
        match self.color {
            Some(TextColor::Entity) => out.push_str("\\C256;"),
            Some(TextColor::Aci(n)) => out.push_str(&format!("\\C{n};")),
            Some(TextColor::Rgb(v)) => out.push_str(&format!("\\c{v};")),
            None => {}
        }
        if let Some(v) = self.oblique {
            out.push_str(&format!("\\Q{v};"));
        }
        if let Some(v) = self.tracking {
            out.push_str(&format!("\\T{v};"));
        }
        if let Some(v) = self.width {
            out.push_str(&format!("\\W{v};"));
        }
        out
    }
}

/// How the text in `range` of `contents` looks. A caret (an empty range)
/// looks like the text just before it.
pub fn format_in(contents: &str, nominal_height: f64, range: Range<usize>) -> SelectionFormat {
    let plain = CharState::plain(nominal_height);
    let Ok(parsed) = parse_with(contents, TextLimits::default()) else {
        return SelectionFormat::of(&plain);
    };
    let mut state = plain;
    let mut stack: Vec<CharState> = Vec::new();
    let mut found: Option<SelectionFormat> = None;
    let mut at_start = plain;
    for node in &parsed.nodes {
        let n = &node.source;
        let shown = matches!(
            node.kind,
            NodeKind::Text
                | NodeKind::Tab
                | NodeKind::EscapedChar
                | NodeKind::NonBreakingSpace
                | NodeKind::Unicode(_)
                | NodeKind::Stack
        );
        if shown && n.start < range.end && n.end > range.start {
            match &mut found {
                Some(f) => f.fold(&state),
                None => found = Some(SelectionFormat::of(&state)),
            }
        }
        match &node.kind {
            NodeKind::Push => stack.push(state),
            NodeKind::Pop => state = stack.pop().unwrap_or(state),
            NodeKind::Format(format) => state = state.after(*format).unwrap_or(state),
            _ => {}
        }
        // Everything up to the caret, or the start of the selection.
        if n.end <= range.start {
            at_start = state;
        }
    }
    if range.start == range.end {
        return SelectionFormat::of(&at_start);
    }
    found.unwrap_or_else(|| SelectionFormat::of(&at_start))
}

/// The result of a formatting edit: the new contents and where the text
/// that was in `range` is now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Formatted {
    pub contents: String,
    pub range: Range<usize>,
}

/// The font control for a bold or italic change at `at`: the font in effect
/// there (its family and its character set and pitch fields) with the flags
/// changed, or `family` if no font control is in effect.
fn font_control_for(
    parsed: &ParsedMText,
    contents: &str,
    at: usize,
    edit: &CharEdit,
    family: &str,
) -> Option<String> {
    if edit.bold.is_none() && edit.italic.is_none() && edit.weight.is_none() {
        return None;
    }
    let mut state = CharState::plain(1.0);
    let mut font: Option<Range<usize>> = None;
    let mut stack: Vec<(CharState, Option<Range<usize>>)> = Vec::new();
    for node in parsed.nodes.iter().take_while(|n| n.source.end <= at) {
        match &node.kind {
            NodeKind::Push => stack.push((state, font.clone())),
            NodeKind::Pop => {
                if let Some((s, f)) = stack.pop() {
                    (state, font) = (s, f);
                }
            }
            NodeKind::Format(format) => {
                state = state.after(*format).unwrap_or(state);
                if matches!(format, Format::Font(_)) {
                    font = Some(node.source.clone());
                }
            }
            _ => {}
        }
    }
    let flags = FontFlags {
        bold: edit.bold.unwrap_or(state.bold),
        italic: edit.italic.unwrap_or(state.italic),
        weight: match edit.weight {
            Some(0) => None,
            Some(w) => Some(w),
            None => state.weight,
        },
    };
    let current = font.as_ref().map(|r| font_parts(&contents[r.clone()]));
    let name = current.as_ref().map_or(family, |p| p.family);
    Some(font_control(name, flags, current.as_ref()))
}

/// Apply `edit` to the text in `range`. `family` names the font that bold
/// and italic are written for when the text has no font control of its own.
/// A range that cuts groups takes the edit stretch by stretch, so each part
/// keeps the group it is in.
pub fn apply_char_edit(
    contents: &str,
    range: Range<usize>,
    edit: &CharEdit,
    family: &str,
) -> Result<Formatted, EditRefusal> {
    if edit.is_empty() || range.start >= range.end || !edit.valid() {
        return apply_in_one_group(contents, range, edit, family);
    }
    let parsed = parse_with(contents, TextLimits::default()).map_err(|_| EditRefusal::TooLarge)?;
    let peeled = inside_groups(&parsed, &range);
    let cut = dangling(&parsed, &peeled);
    if peeled.start >= peeled.end || cut.is_empty() {
        return apply_in_one_group(contents, range, edit, family);
    }
    let stretches = pieces(&parsed, &peeled, &cut);
    let Some(last) = stretches.last() else {
        // Only braces and controls: nothing to format.
        return Ok(Formatted {
            contents: contents.to_owned(),
            range,
        });
    };
    let (mut text, mut grown, mut start) = (contents.to_owned(), 0isize, last.start);
    // From the end, so the stretches before it stay where they are.
    for stretch in stretches.iter().rev() {
        let done = apply_in_one_group(&text, stretch.clone(), edit, family)?;
        grown += done.contents.len() as isize - text.len() as isize;
        start = done.range.start;
        text = done.contents;
    }
    let end = (peeled.end as isize + grown).max(start as isize) as usize;
    Ok(Formatted {
        contents: text,
        range: start..end,
    })
}

/// [`apply_char_edit`] for a range that holds whole groups only.
fn apply_in_one_group(
    contents: &str,
    range: Range<usize>,
    edit: &CharEdit,
    family: &str,
) -> Result<Formatted, EditRefusal> {
    let unchanged = || Formatted {
        contents: contents.to_owned(),
        range: range.clone(),
    };
    if edit.is_empty() || range.start >= range.end {
        return Ok(unchanged());
    }
    if !edit.valid() {
        return Err(EditRefusal::BadValue);
    }
    let limits = TextLimits::default();
    let parsed = parse_with(contents, limits).map_err(|_| EditRefusal::TooLarge)?;
    // A selection that is exactly a group is the group's inside.
    let range = inside_groups(&parsed, &range);
    if range.start >= range.end {
        return Ok(unchanged());
    }
    check(&parsed, contents, &range)?;
    let before = |n: &super::parse::Node| n.source.end <= range.start;
    let depth = parsed
        .nodes
        .iter()
        .take_while(|n| before(n))
        .fold(0i64, |d, n| match n.kind {
            NodeKind::Push => d + 1,
            NodeKind::Pop => d - 1,
            _ => d,
        });
    let font = font_control_for(&parsed, contents, range.start, edit, family);
    let controls = edit.controls(font);
    // A range that is exactly the inside of a group takes the controls
    // inside it, rather than a group of its own.
    // (Controls already at its start belong to that group.)
    let mut at = range.start;
    let mut opens = false;
    while let Some(node) = parsed.nodes.iter().find(|n| n.source.end == at) {
        match node.kind {
            NodeKind::Push => {
                opens = true;
                break;
            }
            NodeKind::Format(_) => at = node.source.start,
            _ => break,
        }
    }
    let closes = parsed
        .nodes
        .iter()
        .any(|n| n.source.start == range.end && n.kind == NodeKind::Pop);
    let (text, new_range) = if opens && closes {
        // The group's leading controls stay, except those this edit sets.
        let kept: String = parsed
            .nodes
            .iter()
            .filter(|n| n.source.start >= at && n.source.end <= range.start)
            .filter(|n| !matches!(&n.kind, NodeKind::Format(f) if superseded(edit, f)))
            .map(|n| &contents[n.source.clone()])
            .collect();
        let mut text = String::with_capacity(contents.len() + controls.len());
        text.push_str(&contents[..at]);
        text.push_str(&kept);
        text.push_str(&controls);
        text.push_str(&contents[range.start..]);
        let start = at + kept.len() + controls.len();
        (text, start..start + (range.end - range.start))
    } else {
        if depth + 1 > limits.max_group_depth as i64 {
            return Err(EditRefusal::TooDeep);
        }
        let mut text = String::with_capacity(contents.len() + controls.len() + 2);
        text.push_str(&contents[..range.start]);
        text.push('{');
        text.push_str(&controls);
        text.push_str(&contents[range.clone()]);
        text.push('}');
        text.push_str(&contents[range.end..]);
        let shift = 1 + controls.len();
        (text, range.start + shift..range.end + shift)
    };
    if text.len() > limits.max_content_bytes {
        return Err(EditRefusal::TooLarge);
    }
    parse_with(&text, limits).map_err(|_| EditRefusal::TooLarge)?;
    Ok(Formatted {
        contents: text,
        range: new_range,
    })
}

mod group;

#[cfg(test)]
mod group_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod weight_tests;
