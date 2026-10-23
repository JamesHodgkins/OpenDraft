//! MLEADERSTYLE objects and MULTILEADER entities.
//!
//! Written from an AutoCAD 2018 file (`private_material/LEADERS1.dxf`). A
//! multileader's own record holds its effective values (what the style says
//! with the entity's changes), so those are read, not the style's; the style
//! only gives the name it is restyled by. The nested data (`CONTEXT_DATA{`,
//! `LEADER{`, `LEADER_LINE{`) is followed by its markers, not its position.
//!
//! What this build cannot hold is skipped and counted by reason, never
//! approximated: spline or no leader lines, block content, annotative
//! styles and entities, custom arrowheads, several leaders in one object,
//! a content scale other than 1 (whether the sizes in the context already
//! include it is unverified), and a drawing plane other than XY.

use std::collections::HashMap;

use od_core::mleader::{
    place_text, LeaderAttachment, MLeader, MLeaderContent, MLeaderContentType, MLeaderLineType,
    MLeaderLook, MLeaderStyle, MLeaderStyleId, MLeaderText, TextAngle,
};
use od_core::{Arrow, Attr, Color, Lineweight, MText, Point, TextStyleId};

use super::{aci_color, decode, Importer, Record};

/// A colour code in a multileader record: by layer, by block, a true colour
/// (`0xC2RRGGBB`) or an index.
const BY_LAYER: u32 = 0xC000_0000;
const BY_BLOCK: u32 = 0xC100_0000;
const TRUE_COLOUR: u32 = 0xC2;

/// The multileader styles the file named: the style's name by the handle of
/// its object.
#[derive(Default)]
pub(super) struct Styles {
    names: HashMap<String, String>,
}

fn colour(value: Option<i64>) -> Attr<Color> {
    let Some(code) = value.and_then(|v| i32::try_from(v).ok()) else {
        return Attr::ByLayer;
    };
    let code = code as u32;
    match code {
        BY_LAYER | BY_BLOCK => Attr::ByLayer,
        c if c >> 24 == TRUE_COLOUR => {
            Attr::Set(Color::Rgb((c >> 16) as u8, (c >> 8) as u8, c as u8))
        }
        c if (1..=255).contains(&c) => Attr::Set(aci_color(i64::from(c))),
        _ => Attr::ByLayer,
    }
}

fn lineweight(value: Option<i64>) -> Attr<Lineweight> {
    match value.and_then(|v| u16::try_from(v).ok()) {
        Some(w) => Attr::Set(Lineweight(w)),
        None => Attr::ByLayer,
    }
}

fn attachment(value: Option<i64>) -> LeaderAttachment {
    usize::try_from(value.unwrap_or(1))
        .ok()
        .and_then(|i| LeaderAttachment::ALL.get(i).copied())
        .unwrap_or_default()
}

fn flag(record: &Record, code: i32) -> bool {
    record.int(code).unwrap_or(0) != 0
}

/// The style a MLEADERSTYLE record describes, or why this build cannot hold it.
fn style_of(
    record: &Record,
    id: MLeaderStyleId,
    name: &str,
    text_styles: &super::text::Styles,
) -> Result<MLeaderStyle, &'static str> {
    if flag(record, 296) {
        return Err("annotative");
    }
    let content_kind = match record.int(170) {
        Some(0) => MLeaderContentType::None,
        Some(2) => MLeaderContentType::MText,
        _ => return Err("block content"),
    };
    let leader_type = match record.int(173) {
        Some(0) => MLeaderLineType::None,
        Some(1) => MLeaderLineType::Straight,
        Some(2) => MLeaderLineType::Spline,
        _ => return Err("unknown leader type"),
    };
    let mut style = MLeaderStyle::new(id, name);
    let number = |code, fallback| record.number(code).unwrap_or(fallback);
    let angle = |code| record.number(code).filter(|a| *a != 0.0);
    style.description = decode(record.text(3).unwrap_or(""));
    style.leader_type = leader_type;
    style.leader_color = colour(record.int(91));
    style.leader_lineweight = lineweight(record.int(92));
    style.arrow_size = number(44, style.arrow_size);
    style.break_gap = number(143, style.break_gap);
    style.max_points = record
        .int(90)
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| *n >= 2);
    style.first_angle = angle(40);
    style.second_angle = angle(41);
    style.dogleg = flag(record, 291);
    style.dogleg_length = number(43, style.dogleg_length);
    style.overall_scale = number(142, 1.0);
    let c = &mut style.content;
    c.kind = content_kind;
    c.default_text = decode(record.text(300).unwrap_or(""));
    c.text_style = record
        .text(342)
        .and_then(|h| text_styles.by_handle.get(h))
        .copied()
        .unwrap_or(TextStyleId::STANDARD);
    c.text_angle = match record.int(175) {
        Some(0) => TextAngle::AsInserted,
        Some(2) => TextAngle::RightReading,
        _ => TextAngle::KeepHorizontal,
    };
    c.text_color = colour(record.int(93));
    c.text_height = number(45, c.text_height);
    c.always_left_justify = flag(record, 297);
    c.frame = flag(record, 292);
    c.left_attachment = attachment(record.int(174));
    c.right_attachment = attachment(record.int(178));
    c.landing_gap = number(42, c.landing_gap);
    if style.is_valid() {
        Ok(style)
    } else {
        Err("values out of range")
    }
}

impl Importer {
    /// Read the style objects and name them by the dictionary that holds
    /// them, before any multileader refers to one.
    pub(super) fn scan_mleader_styles(&mut self, records: &[Record]) {
        let mut dictionary: HashMap<&str, &str> = HashMap::new();
        for record in records.iter().filter(|r| r.name == "DICTIONARY") {
            let mut name = None;
            for group in &record.groups {
                match group.code {
                    3 => name = Some(group.value.as_str()),
                    350 | 360 => {
                        if let Some(name) = name.take() {
                            dictionary.insert(group.value.as_str(), name);
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut table = self.document.mleader_styles().to_vec();
        for record in records.iter().filter(|r| r.name == "MLEADERSTYLE") {
            let Some(handle) = record.text(5) else {
                continue;
            };
            let Some(name) = dictionary.get(handle).map(|n| decode(n)) else {
                continue;
            };
            let slot = table
                .iter()
                .position(|s| s.name.eq_ignore_ascii_case(&name));
            let id = match slot {
                Some(i) => table[i].id,
                None => MLeaderStyleId(table.iter().map(|s| s.id.0 + 1).max().unwrap_or(1)),
            };
            // A style this build cannot hold is not reported by itself
            // (every AutoCAD drawing has an annotative one); a multileader
            // that uses it is.
            if let Ok(style) = style_of(record, id, &name, &self.text_styles) {
                match slot {
                    Some(i) => table[i] = style,
                    None => table.push(style),
                }
                self.mleaders.names.insert(handle.to_owned(), name);
            }
        }
        let next = table.iter().map(|s| s.id.0 + 1).max().unwrap_or(1);
        self.document
            .set_mleader_styles(table, MLeaderStyleId::STANDARD, next);
    }

    /// A MULTILEADER entity: into the drawing, or counted by why not.
    pub(super) fn multileader(&mut self, record: &Record) {
        match read(record, &self.mleaders, &self.text_styles) {
            Ok(m) if m.validate().is_ok() => {
                self.add(record, od_core::Entity::MLeader(Box::new(m)))
            }
            Ok(_) => self.damaged += 1,
            Err(why) => self.skip(&format!("MULTILEADER ({why})")),
        }
    }
}

#[derive(Default)]
struct Node {
    last: Option<Point>,
    dogleg: Option<Point>,
    length: f64,
    lines: Vec<Vec<Point>>,
}

/// Where in the nested data the groups are.
#[derive(Clone, Copy, PartialEq)]
enum Level {
    Outer,
    Context,
    Node,
    Line,
}

#[derive(Default)]
struct Context {
    scale: Option<f64>,
    text_height: Option<f64>,
    arrow_size: Option<f64>,
    gap: Option<f64>,
    left: Option<i64>,
    right: Option<i64>,
    has_text: bool,
    text: Option<String>,
    text_at: Option<Point>,
    text_style: Option<String>,
    width: f64,
    has_block: bool,
    /// The plane's x and y axes, when given: anything but east and north is
    /// not a drawing in XY.
    plane: Vec<(i32, f64)>,
    nodes: Vec<Node>,
}

/// What the record says outside the context data.
#[derive(Default)]
struct Outer {
    style: Option<String>,
    leader_type: Option<i64>,
    content: Option<i64>,
    dogleg: bool,
    frame: bool,
    annotative: bool,
    arrow_block: bool,
}

fn point(x: Option<f64>, y: Option<f64>) -> Option<Point> {
    Some(Point::new(x?, y?))
}

/// The groups of `record`, sorted into the context data and the rest.
fn sort(record: &Record) -> Option<(Context, Outer)> {
    let (mut ctx, mut outer) = (Context::default(), Outer::default());
    let mut level = Level::Outer;
    let mut seen_context = false;
    let mut x: Option<f64> = None;
    let (mut text_x, mut text_y, mut dog_x, mut dog_y) = (None, None, None, None);
    let (mut last_x, mut last_y): (Option<f64>, Option<f64>) = (None, None);
    for g in &record.groups {
        let num = g.value.parse::<f64>().ok().filter(|v| v.is_finite());
        let int = g.value.parse::<i64>().ok();
        match (g.code, g.value.as_str(), level) {
            (300, "CONTEXT_DATA{", Level::Outer) => level = Level::Context,
            (301, "}", Level::Context) => {
                level = Level::Outer;
                seen_context = true;
            }
            (302, "LEADER{", Level::Context) => {
                ctx.nodes.push(Node::default());
                level = Level::Node;
            }
            (304, "LEADER_LINE{", Level::Node) => {
                ctx.nodes.last_mut()?.lines.push(Vec::new());
                level = Level::Line;
            }
            (305, "}", Level::Line) => level = Level::Node,
            (303, "}", Level::Node) => level = Level::Context,
            (_, _, Level::Context) => match g.code {
                40 => ctx.scale = num,
                41 => ctx.text_height = num,
                140 => ctx.arrow_size = num,
                145 => ctx.gap = num,
                174 => ctx.left = int,
                175 => ctx.right = int,
                290 => ctx.has_text = int? != 0,
                304 => ctx.text = Some(g.value.clone()),
                340 => ctx.text_style = Some(g.value.clone()),
                12 => text_x = num,
                22 => text_y = num,
                43 => ctx.width = num.unwrap_or(0.0),
                296 => ctx.has_block = int? != 0,
                111 | 121 | 131 | 112 | 122 | 132 => ctx.plane.push((g.code, num?)),
                _ => {}
            },
            (_, _, Level::Node) => {
                let node = ctx.nodes.last_mut()?;
                match g.code {
                    10 => last_x = num,
                    20 => last_y = num,
                    11 => dog_x = num,
                    21 => dog_y = num,
                    40 => node.length = num?,
                    _ => {}
                }
                node.last = point(last_x, last_y);
                node.dogleg = point(dog_x, dog_y);
            }
            (10 | 20, _, Level::Line) => {
                if g.code == 10 {
                    x = num;
                } else {
                    let p = point(x.take(), num)?;
                    ctx.nodes.last_mut()?.lines.last_mut()?.push(p);
                }
            }
            (_, _, Level::Line) => {}
            (340, _, Level::Outer) if seen_context && outer.style.is_none() => {
                outer.style = Some(g.value.clone());
            }
            (170, _, Level::Outer) if seen_context => outer.leader_type = int,
            (172, _, Level::Outer) if seen_context => outer.content = int,
            (291, _, Level::Outer) if seen_context => outer.dogleg = int? != 0,
            (292, _, Level::Outer) if seen_context => outer.frame = int? != 0,
            (293, _, Level::Outer) if seen_context => outer.annotative = int? != 0,
            (342, _, Level::Outer) if seen_context => outer.arrow_block = true,
            _ => {}
        }
    }
    ctx.text_at = point(text_x, text_y);
    (seen_context && level == Level::Outer).then_some((ctx, outer))
}

fn read(
    record: &Record,
    styles: &Styles,
    text_styles: &super::text::Styles,
) -> Result<MLeader, &'static str> {
    let (ctx, outer) = sort(record).ok_or("damaged")?;
    let style_name = outer
        .style
        .as_ref()
        .and_then(|h| styles.names.get(h))
        .ok_or("its style was not imported")?
        .clone();
    if outer.annotative {
        return Err("annotative");
    }
    if outer.arrow_block {
        return Err("custom arrowhead");
    }
    if ctx.has_block || outer.content == Some(1) {
        return Err("block content");
    }
    match outer.leader_type {
        Some(1) => {}
        Some(0) => return Err("no leader line"),
        Some(2) => return Err("spline leader"),
        _ => return Err("unknown leader type"),
    }
    if ctx.scale.is_none_or(|s| (s - 1.0).abs() > 1e-9) {
        return Err("content scale other than 1");
    }
    let expected = [
        (111, 1.0),
        (121, 0.0),
        (131, 0.0),
        (112, 0.0),
        (122, 1.0),
        (132, 0.0),
    ];
    if ctx
        .plane
        .iter()
        .any(|(c, v)| expected.iter().any(|(e, w)| e == c && (v - w).abs() > 1e-9))
    {
        return Err("not drawn in the XY plane");
    }
    let node = match ctx.nodes.as_slice() {
        [] => return Err("no leader line"),
        [node] => node,
        _ => return Err("several leaders in one object"),
    };
    let [line] = node.lines.as_slice() else {
        return Err("several leader lines");
    };
    let mut points = line.clone();
    points.push(node.last.ok_or("damaged")?);
    let dir = node
        .dogleg
        .filter(|d| d.x.hypot(d.y) > 1e-9)
        .map_or(Point::new(1.0, 0.0), |d| {
            let len = d.x.hypot(d.y);
            Point::new(d.x / len, d.y / len)
        });
    let look = MLeaderLook {
        leader_type: MLeaderLineType::Straight,
        arrow: Arrow::Closed,
        arrow_size: ctx.arrow_size.ok_or("damaged")?,
        dogleg: outer.dogleg,
        dogleg_length: node.length,
        landing_gap: ctx.gap.ok_or("damaged")?,
    };
    let on_left = dir.x >= 0.0;
    let content = match (outer.content, ctx.has_text, ctx.text) {
        (Some(2), true, Some(text)) => {
            let height = ctx.text_height.ok_or("damaged")?;
            let attach = attachment(if on_left { ctx.left } else { ctx.right });
            let end = Point::new(
                node.last.map_or(0.0, |p| p.x)
                    + dir.x * if look.dogleg { look.dogleg_length } else { 0.0 },
                node.last.map_or(0.0, |p| p.y)
                    + dir.y * if look.dogleg { look.dogleg_length } else { 0.0 },
            );
            let (kind, at) = place_text(attach, on_left, end, look.landing_gap, height);
            let mut mtext = MText::new(
                ctx.text_at.unwrap_or(at),
                height,
                decode(&text),
                ctx.text_style
                    .as_ref()
                    .and_then(|h| text_styles.by_handle.get(h))
                    .copied()
                    .unwrap_or(TextStyleId::STANDARD),
            );
            mtext.attachment = kind;
            mtext.reference_width = ctx.width.max(0.0);
            MLeaderContent::MText(MLeaderText {
                text: mtext,
                frame: outer.frame,
            })
        }
        (Some(0), _, _) => MLeaderContent::None,
        _ => return Err("unknown content"),
    };
    Ok(MLeader {
        style_name,
        look,
        lines: vec![points],
        landing_dir: dir,
        content,
    })
}
