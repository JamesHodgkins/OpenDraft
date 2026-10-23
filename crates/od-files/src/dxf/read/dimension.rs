//! DIMENSION entities and the DIMSTYLE table. A dimension is rebuilt from
//! its definition points, its style (by name, with the overrides carried as
//! `ACAD`/`DSTYLE` extended data) and its text; the picture AutoCAD kept in
//! the anonymous block is not read, ours is drawn from the points.
//!
//! Linear and aligned, radius, diameter, angular (from three points and from
//! two lines) and ordinate dimensions are read, and a large radial dimension
//! as a jogged radius. `ARC_DIMENSION` and the others are skipped by name.

use std::f64::consts::TAU;

use od_core::dimension::lines_angle_points;
use od_core::{DimStyle, DimText, Dimension, Entity, Point};

use super::{decode, Group, Importer, Record};
use crate::dxf::dimstyle_codec::{apply, finish, Value};

/// The dimension styles of the file's table, in its order.
#[derive(Default)]
pub(in crate::dxf) struct Styles {
    table: Vec<DimStyle>,
}

/// A group's value as a style variable: text for the two that are text,
/// a number for the rest.
fn value_of(code: i32, text: &str) -> Option<Value> {
    if matches!(code, 3 | 5) {
        return Some(Value::Text(decode(text)));
    }
    text.trim().parse::<f64>().ok().map(Value::Real)
}

/// The variables `record` overrides: the pairs inside `ACAD` / `DSTYLE`
/// extended data.
fn overrides(groups: &[Group]) -> Vec<(i32, Value)> {
    let mut found = Vec::new();
    let mut i = 0;
    while i + 2 < groups.len() {
        let start = groups[i].code == 1001
            && groups[i].value == "ACAD"
            && groups[i + 1].code == 1000
            && groups[i + 1].value == "DSTYLE"
            && groups[i + 2].code == 1002
            && groups[i + 2].value == "{";
        i += 1;
        if !start {
            continue;
        }
        let mut j = i + 2;
        while j + 1 < groups.len() && !(groups[j].code == 1002 && groups[j].value == "}") {
            // Each variable is its code in a 1070 group, then its value.
            if groups[j].code == 1070 {
                if let Ok(code) = groups[j].value.trim().parse::<i32>() {
                    if let Some(value) = value_of(code, &groups[j + 1].value) {
                        found.push((code, value));
                    }
                }
                j += 2;
            } else {
                j += 1;
            }
        }
        break;
    }
    found
}

fn degrees(record: &Record, code: i32) -> Option<f64> {
    record.number(code).map(f64::to_radians)
}

impl Importer {
    /// The DIMSTYLE table, read before the entities that name its styles.
    pub(super) fn scan_dim_styles(&mut self, records: &[Record]) {
        for record in records.iter().filter(|r| r.name == "DIMSTYLE") {
            let Some(name) = record.text(2) else {
                continue;
            };
            let mut style = DimStyle {
                name: decode(name),
                ..DimStyle::default()
            };
            for group in &record.groups {
                if group.code == 2 {
                    continue;
                }
                if let Some(value) = value_of(group.code, &group.value) {
                    apply(&mut style, group.code, &value);
                }
            }
            finish(&mut style);
            if !self
                .dim_styles
                .table
                .iter()
                .any(|s| s.name.eq_ignore_ascii_case(&style.name))
            {
                self.dim_styles.table.push(style.sanitized());
            }
        }
    }

    /// Put the table in the drawing, `Standard` first and current.
    pub(super) fn finish_dim_styles(&mut self) {
        let mut table = std::mem::take(&mut self.dim_styles.table);
        if table.is_empty() {
            return;
        }
        match table
            .iter()
            .position(|s| s.name.eq_ignore_ascii_case("Standard"))
        {
            Some(0) => {}
            Some(i) => {
                let standard = table.remove(i);
                table.insert(0, standard);
            }
            None => table.insert(0, DimStyle::default()),
        }
        let current = table[0].clone();
        self.document.set_dim_styles(table, current);
    }

    pub(super) fn dimension(&mut self, record: &Record) {
        match self.read_dimension(record) {
            Some(dimension) if dimension.measurement().is_some() => {
                self.add(record, Entity::Dimension(Box::new(dimension)));
            }
            Some(_) => self.damaged += 1,
            None => self.skip("DIMENSION (kind)"),
        }
    }

    fn read_dimension(&self, record: &Record) -> Option<Dimension> {
        let flags = record.int(70)?;
        let at = |x: i32| Self::point(record, x, x + 10);
        let name = record.text(3).map(decode).unwrap_or_default();
        let mut style = self
            .dim_styles
            .table
            .iter()
            .find(|s| s.name.eq_ignore_ascii_case(&name))
            .cloned()
            .unwrap_or_default();
        for (code, value) in overrides(&record.groups) {
            apply(&mut style, code, &value);
        }
        finish(&mut style);
        let large = record
            .groups
            .iter()
            .any(|g| g.code == 100 && g.value == "AcDbRadialDimensionLarge");
        let mut dimension = match flags & 15 {
            0 | 1 => {
                let (p1, p2, line_point) = (at(13)?, at(14)?, at(10)?);
                let aligned = flags & 15 == 1;
                let line = if aligned {
                    (p2.y - p1.y).atan2(p2.x - p1.x)
                } else {
                    degrees(record, 50).unwrap_or(0.0)
                };
                let mut d = if aligned {
                    Dimension::aligned(p1, p2, line_point, &style)
                } else {
                    Dimension::linear(p1, p2, line_point, line, &style)
                };
                if let (Some(slant), od_core::DimKind::Linear { oblique, .. }) =
                    (degrees(record, 52), &mut d.kind)
                {
                    if (slant - std::f64::consts::FRAC_PI_2).abs() > 1e-9 {
                        *oblique = Some((line + slant).rem_euclid(TAU));
                    }
                }
                d
            }
            2 => {
                let first = (at(13)?, at(14)?);
                let second = (at(15)?, at(10)?);
                let (vertex, p1, p2) = lines_angle_points(first, second)?;
                Dimension::angular(vertex, p1, p2, at(16)?, true, &style)
            }
            5 => Dimension::angular(at(15)?, at(13)?, at(14)?, at(10)?, false, &style),
            3 => {
                let (far, chord) = (at(10)?, at(15)?);
                let centre = Point::new((far.x + chord.x) / 2.0, (far.y + chord.y) / 2.0);
                Dimension::diameter(centre, chord, &style)
            }
            4 if large => Dimension::jogged(at(10)?, at(13)?, at(14)?, at(15)?, &style),
            4 => Dimension::radius(at(10)?, at(15)?, &style),
            6 => Dimension::ordinate(at(13)?, at(14)?, flags & 64 != 0, &style),
            _ => return None,
        };
        if let od_core::DimKind::Radial {
            leader_length,
            jog: None,
            ..
        } = &mut dimension.kind
        {
            if let Some(length) = record.number(40).filter(|l| *l >= 0.0) {
                *leader_length = length;
            }
        }
        dimension.text = DimText::from_entry(&record.text(1).map(decode).unwrap_or_default());
        if flags & 128 != 0 {
            dimension.text_pos = at(11);
        }
        dimension.text_angle = degrees(record, 53);
        Some(dimension)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(code: i32, value: &str) -> Group {
        Group {
            code,
            value: value.to_owned(),
        }
    }

    #[test]
    fn overrides_are_the_pairs_inside_the_dstyle_braces() {
        let groups = vec![
            group(3, "Standard"),
            group(1001, "ACAD"),
            group(1000, "DSTYLE"),
            group(1002, "{"),
            group(1070, "140"),
            group(1040, "5.5"),
            group(1070, "77"),
            group(1070, "0"),
            group(1070, "3"),
            group(1000, "<> mm"),
            group(1002, "}"),
            group(1001, "OTHER"),
        ];
        assert_eq!(
            overrides(&groups),
            [
                (140, Value::Real(5.5)),
                (77, Value::Real(0.0)),
                (3, Value::Text("<> mm".to_owned())),
            ]
        );
        assert!(overrides(&[group(1001, "ACAD"), group(1000, "OTHER")]).is_empty());
    }
}
