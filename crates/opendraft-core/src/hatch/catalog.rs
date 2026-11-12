//! Pattern definitions: the `.pat` text format and the patterns built in.
//!
//! ```text
//! *pattern-name, description
//! angle, x-origin,y-origin, delta-x,delta-y, dash-1,dash-2,...
//! ```
//!
//! `delta-x` slides each copy along the line and `delta-y` is the distance
//! between copies, so a line's step is `(delta-x, delta-y)` turned by its
//! angle. Positive dashes draw, negative ones skip and zero is a dot.

/// One line of a pattern definition, as written in a `.pat` file.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternDefLine {
    /// Degrees.
    pub angle: f64,
    pub origin: (f64, f64),
    pub delta: (f64, f64),
    pub dashes: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PatternDef {
    pub name: String,
    pub description: String,
    pub lines: Vec<PatternDefLine>,
}

/// A set of pattern definitions, looked up by name ignoring case.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PatternCatalog {
    pub patterns: Vec<PatternDef>,
}

/// Longest pattern name AutoCAD allows.
const MAX_NAME: usize = 31;
/// Most dashes one definition line may have in a file read here.
const MAX_DASHES: usize = 64;
/// Most lines one pattern may have.
const MAX_LINES: usize = 64;

impl PatternCatalog {
    /// Reads `.pat` text. Comments (`;` to the end of the line) and blank
    /// lines are ignored; a pattern or line that is not valid is left out.
    pub fn parse(text: &str) -> Self {
        let mut patterns: Vec<PatternDef> = Vec::new();
        let mut open: Option<PatternDef> = None;
        for raw in text.lines() {
            let line = raw.split(';').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if let Some(header) = line.strip_prefix('*') {
                patterns.extend(open.take());
                let (name, description) = match header.split_once(',') {
                    Some((n, d)) => (n.trim(), d.trim()),
                    None => (header.trim(), ""),
                };
                open = (!name.is_empty() && name.len() <= MAX_NAME).then(|| PatternDef {
                    name: name.to_owned(),
                    description: description.to_owned(),
                    lines: Vec::new(),
                });
            } else if let Some(def) = open.as_mut() {
                match parse_line(line) {
                    Some(l) if def.lines.len() < MAX_LINES => def.lines.push(l),
                    _ => open = None,
                }
            }
        }
        patterns.extend(open);
        patterns.retain(|p| !p.lines.is_empty());
        Self { patterns }
    }

    pub fn find(&self, name: &str) -> Option<&PatternDef> {
        self.patterns
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.patterns.iter().map(|p| p.name.as_str())
    }
}

fn parse_line(line: &str) -> Option<PatternDefLine> {
    let numbers: Vec<f64> = line
        .split(',')
        .map(|s| s.trim().parse::<f64>().ok().filter(|v| v.is_finite()))
        .collect::<Option<_>>()?;
    if numbers.len() < 5 || numbers.len() - 5 > MAX_DASHES {
        return None;
    }
    Some(PatternDefLine {
        angle: numbers[0],
        origin: (numbers[1], numbers[2]),
        delta: (numbers[3], numbers[4]),
        dashes: numbers[5..].to_vec(),
    })
}

/// The patterns built in: a starting set in AutoCAD's imperial units
/// (`acad.pat`), where scale 1 draws `ANSI31` lines an eighth of a unit
/// apart. Solid fill is not a pattern and is not listed.
pub fn builtin() -> &'static PatternCatalog {
    use std::sync::OnceLock;
    static CATALOG: OnceLock<PatternCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| PatternCatalog::parse(BUILTIN))
}

const BUILTIN: &str = "\
*ANGLE, Angle steel
0, 0,0, 0,.275, .2,-.075
90, 0,0, 0,.275, .2,-.075

*ANSI31, ANSI Iron, Brick, Stone masonry
45, 0,0, 0,.125

*ANSI32, ANSI Steel
45, 0,0, 0,.375
45, .176776695,0, 0,.375

*ANSI37, ANSI Lead, Zinc, Magnesium, Sound/Heat/Elec Insulation
45, 0,0, 0,.125
135, 0,0, 0,.125

*LINE, Parallel horizontal lines
0, 0,0, 0,.125

*NET, Horizontal / vertical grid
0, 0,0, 0,.125
90, 0,0, 0,.125

*NET3, Network pattern 0-60-120
0, 0,0, 0,.125
60, 0,0, 0,.125
120, 0,0, 0,.125
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pat_file_reads_with_comments_dashes_and_blank_lines() {
        let text = "\
;; a comment
*DASHY, Dashed lines ; trailing
0, 0,0, 0,1, 3,-1,0,-1

*TWO, Two lines
45, 0,0, 0,.125
135, 0,0, 0,.125
";
        let catalog = PatternCatalog::parse(text);
        assert_eq!(catalog.names().collect::<Vec<_>>(), ["DASHY", "TWO"]);
        let dashy = catalog.find("dashy").unwrap();
        assert_eq!(dashy.description, "Dashed lines");
        assert_eq!(dashy.lines[0].dashes, [3.0, -1.0, 0.0, -1.0]);
        assert_eq!(catalog.find("TWO").unwrap().lines.len(), 2);
    }

    #[test]
    fn bad_patterns_are_left_out_and_the_rest_read() {
        let text = "\
*BAD, Not numbers
0, 0,0, x,1

*LONGNAME_LONGNAME_LONGNAME_LONGNAME, too long
0, 0,0, 0,1

*SHORT, Too few numbers
0, 0,0, 1

*GOOD, Fine
0, 0,0, 0,1
";
        let catalog = PatternCatalog::parse(text);
        assert_eq!(catalog.names().collect::<Vec<_>>(), ["GOOD"]);
    }

    #[test]
    fn the_built_in_patterns_are_there() {
        let catalog = builtin();
        for name in ["ANSI31", "ansi32", "LINE", "NET"] {
            assert!(catalog.find(name).is_some(), "{name}");
        }
        assert_eq!(catalog.find("ANSI37").unwrap().lines.len(), 2);
    }
}
