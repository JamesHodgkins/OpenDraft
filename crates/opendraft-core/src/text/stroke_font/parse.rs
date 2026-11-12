//! The `.odf` text format:
//!
//! ```text
//! odroman 1
//! metrics unit=1000 cap=1000 xheight=659 descender=364 space=556 weight=100 bold=1.6 italic=15
//! glyph 0041 adv=760
//!   stroke [w=0.8] M 0 0  L 380 1000  C x1 y1 x2 y2 x y
//!   dot [w=0.8] x y
//! ```
//!
//! `weight` is thousandths of cap height. Everything is bounded before it is
//! built, so no input can make the reader do unbounded work.

use super::{Pt, Segment, Stroke, StrokeFont, StrokeFontError, StrokeGlyph, StrokeMetrics};

/// Largest file read.
pub const MAX_BYTES: usize = 1 << 20;
const MAX_GLYPHS: usize = 4096;
const MAX_STROKES: usize = 64;
const MAX_SEGMENTS: usize = 256;
/// Coordinates beyond this (in font units) are corrupt, not design.
const MAX_COORD: f64 = 20_000.0;

fn fail<T>(line: usize, message: impl Into<String>) -> Result<T, StrokeFontError> {
    Err(StrokeFontError {
        line,
        message: message.into(),
    })
}

fn number(line: usize, token: &str) -> Result<f64, StrokeFontError> {
    match token.parse::<f64>() {
        Ok(v) if v.is_finite() && v.abs() <= MAX_COORD => Ok(v),
        _ => fail(line, format!("`{token}` is not a usable number")),
    }
}

fn point(line: usize, tokens: &mut std::slice::Iter<'_, &str>) -> Result<Pt, StrokeFontError> {
    let (Some(x), Some(y)) = (tokens.next(), tokens.next()) else {
        return fail(line, "a point needs two numbers");
    };
    Ok((number(line, x)?, number(line, y)?))
}

fn metrics(line: usize, fields: &[&str]) -> Result<StrokeMetrics, StrokeFontError> {
    let mut m = StrokeMetrics {
        unit: 1000.0,
        cap_height: 1000.0,
        x_height: 650.0,
        descender: 300.0,
        space: 500.0,
        default_weight: 0.1,
        bold_factor: 1.6,
        italic_degrees: 15.0,
    };
    for field in fields {
        let Some((key, value)) = field.split_once('=') else {
            return fail(line, format!("`{field}` is not key=value"));
        };
        let v = number(line, value)?;
        match key {
            "unit" => m.unit = v,
            "cap" => m.cap_height = v,
            "xheight" => m.x_height = v,
            "descender" => m.descender = v,
            "space" => m.space = v,
            "weight" => m.default_weight = v / 1000.0,
            "bold" => m.bold_factor = v,
            "italic" => m.italic_degrees = v,
            _ => return fail(line, format!("unknown metric `{key}`")),
        }
    }
    if m.unit <= 0.0 || m.cap_height <= 0.0 || m.default_weight <= 0.0 || m.bold_factor <= 0.0 {
        return fail(line, "unit, cap, weight and bold must be positive");
    }
    Ok(m)
}

/// The weight a line starts with (`w=0.8`, else 1.0) and what follows it.
fn weight_and_rest<'a, 'b>(
    line: usize,
    tokens: &'b [&'a str],
) -> Result<(f64, &'b [&'a str]), StrokeFontError> {
    match tokens.first().and_then(|t| t.strip_prefix("w=")) {
        Some(value) => {
            let weight = number(line, value)?;
            if weight <= 0.0 {
                return fail(line, "a stroke weight must be positive");
            }
            Ok((weight, &tokens[1..]))
        }
        None => Ok((1.0, tokens)),
    }
}

fn dot(line: usize, tokens: &[&str]) -> Result<Stroke, StrokeFontError> {
    let (weight, rest) = weight_and_rest(line, tokens)?;
    let mut it = rest.iter();
    Ok(Stroke {
        weight,
        start: point(line, &mut it)?,
        segments: Vec::new(),
    })
}

fn stroke(line: usize, tokens: &[&str]) -> Result<Stroke, StrokeFontError> {
    let (weight, rest) = weight_and_rest(line, tokens)?;
    let mut it = rest.iter();
    if it.next() != Some(&"M") {
        return fail(line, "a stroke starts with M");
    }
    let start = point(line, &mut it)?;
    let mut segments = Vec::new();
    while let Some(&command) = it.next() {
        if segments.len() == MAX_SEGMENTS {
            return fail(line, "too many segments in a stroke");
        }
        segments.push(match command {
            "L" => Segment::Line(point(line, &mut it)?),
            "C" => Segment::Cubic(
                point(line, &mut it)?,
                point(line, &mut it)?,
                point(line, &mut it)?,
            ),
            other => return fail(line, format!("unknown command `{other}`")),
        });
    }
    Ok(Stroke {
        weight,
        start,
        segments,
    })
}

pub(super) fn parse(text: &str) -> Result<StrokeFont, StrokeFontError> {
    if text.len() > MAX_BYTES {
        return fail(0, "the font file is too large");
    }
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'));
    match lines.next() {
        Some((_, "odroman 1")) => {}
        Some((n, _)) => return fail(n, "not an OD Roman file (expected `odroman 1`)"),
        None => return fail(0, "the font file is empty"),
    }
    let mut measures = None;
    let mut glyphs: Vec<StrokeGlyph> = Vec::new();
    for (n, line) in lines {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let Some((&head, args)) = tokens.split_first() else {
            continue;
        };
        match head {
            "metrics" => {
                if measures.is_some() {
                    return fail(n, "metrics given twice");
                }
                measures = Some(metrics(n, args)?);
            }
            "glyph" => {
                if glyphs.len() == MAX_GLYPHS {
                    return fail(n, "too many glyphs");
                }
                let (Some(code), Some(adv)) = (args.first(), args.get(1)) else {
                    return fail(n, "glyph needs a code and adv=");
                };
                let ch = u32::from_str_radix(code, 16)
                    .ok()
                    .and_then(char::from_u32)
                    .ok_or_else(|| StrokeFontError {
                        line: n,
                        message: format!("`{code}` is not a code point"),
                    })?;
                let advance = match adv.strip_prefix("adv=") {
                    Some(v) => number(n, v)?,
                    None => return fail(n, "glyph needs adv="),
                };
                if advance < 0.0 {
                    return fail(n, "an advance cannot be negative");
                }
                if glyphs.iter().any(|g| g.ch == ch) {
                    return fail(n, format!("glyph {code} given twice"));
                }
                glyphs.push(StrokeGlyph {
                    ch,
                    advance,
                    strokes: Vec::new(),
                });
            }
            "stroke" | "dot" => {
                let Some(glyph) = glyphs.last_mut() else {
                    return fail(n, "a stroke before any glyph");
                };
                if glyph.strokes.len() == MAX_STROKES {
                    return fail(n, "too many strokes in a glyph");
                }
                let made = if head == "stroke" {
                    stroke(n, args)?
                } else {
                    dot(n, args)?
                };
                glyph.strokes.push(made);
            }
            other => return fail(n, format!("unknown line `{other}`")),
        }
    }
    let Some(measures) = measures else {
        return fail(0, "no metrics line");
    };
    StrokeFont::from_parts(measures, glyphs)
}
