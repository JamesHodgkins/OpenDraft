//! Power Select (AutoCAD's QSELECT): pick objects by kind and one property,
//! such as every circle whose diameter is 6. This is the pure part; the
//! dialog is `app/power_select_dialog.rs`.

use od_core::{Attr, Color};
use od_core::{Document, EntityId, EntityKind, Object, Units};

use crate::properties::{self, Field, Unit};

/// Which objects the search looks through.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scope {
    #[default]
    Drawing,
    Selection,
}

/// A property the search can test.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Prop {
    Layer,
    Color,
    Linetype,
    Lineweight,
    LinetypeScale,
    Field(Field),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Operator {
    #[default]
    Equals,
    NotEqual,
    Greater,
    Less,
    /// `*` and `?`, for text.
    Wildcard,
    /// Every object of the kind, whatever the property.
    All,
}

impl Operator {
    pub fn label(self) -> &'static str {
        match self {
            Operator::Equals => "= Equals",
            Operator::NotEqual => "<> Not Equal to",
            Operator::Greater => "> Greater than",
            Operator::Less => "< Less than",
            Operator::Wildcard => "Wildcard Match",
            Operator::All => "*  Select All",
        }
    }
}

/// What the dialog has set.
#[derive(Clone, Debug)]
pub struct Draft {
    pub scope: Scope,
    /// The kind of object, or `None` for any.
    pub kind: Option<EntityKind>,
    pub prop: Prop,
    pub operator: Operator,
    pub value: String,
    /// Include what matches in the new selection (else exclude it).
    pub include: bool,
    /// Add to the current selection instead of replacing it.
    pub append: bool,
}

impl Default for Draft {
    fn default() -> Self {
        Draft {
            scope: Scope::Drawing,
            kind: None,
            prop: Prop::Layer,
            operator: Operator::Equals,
            value: String::new(),
            include: true,
            append: false,
        }
    }
}

/// The dialog's state: whether it is open, hidden while objects are picked
/// for it, and what it has set.
#[derive(Default)]
pub struct Dialog {
    pub open: bool,
    /// Hidden while the user picks the objects to search.
    pub picking: bool,
    pub draft: Draft,
    /// What the lists were built for; when it changes the value is reset.
    pub shown: Option<(Scope, Option<EntityKind>, Prop)>,
}

/// How one object reads for a property.
#[derive(Clone, Debug, PartialEq)]
enum Reading {
    Number(f64),
    Text(String),
}

impl Prop {
    pub fn label(self, rows: &[properties::Row]) -> String {
        match self {
            Prop::Layer => "Layer".to_owned(),
            Prop::Color => "Color".to_owned(),
            Prop::Linetype => "Linetype".to_owned(),
            Prop::Lineweight => "Lineweight".to_owned(),
            Prop::LinetypeScale => "Linetype scale".to_owned(),
            Prop::Field(f) => rows
                .iter()
                .find(|r| r.field == f)
                .map_or_else(|| format!("{f:?}"), |r| r.label.to_owned()),
        }
    }

    /// The unit of a number property; `None` for text.
    fn unit(self) -> Option<Unit> {
        match self {
            Prop::Layer | Prop::Color | Prop::Linetype | Prop::Lineweight => None,
            Prop::LinetypeScale => Some(Unit::Factor),
            Prop::Field(f) => Some(f.unit()),
        }
    }

    /// Whether the value is typed (a number) rather than picked from what
    /// the drawing has.
    pub fn typed(self) -> bool {
        matches!(
            self.unit(),
            Some(Unit::Length | Unit::Angle | Unit::Area | Unit::Count | Unit::Factor)
        )
    }

    /// Operators that make sense for the property.
    pub fn operators(self) -> &'static [Operator] {
        use Operator::*;
        if self.typed() {
            &[Equals, NotEqual, Greater, Less, All]
        } else if matches!(self, Prop::Layer | Prop::Linetype) {
            &[Equals, NotEqual, Wildcard, All]
        } else {
            &[Equals, NotEqual, All]
        }
    }
}

fn colour_text(color: &Attr<Color>) -> String {
    match color {
        Attr::ByLayer => "ByLayer".to_owned(),
        Attr::Set(Color::Foreground) => "White/Black".to_owned(),
        Attr::Set(Color::Rgb(r, g, b)) => format!("{r},{g},{b}"),
    }
}

fn read(document: &Document, object: &Object, prop: Prop) -> Reading {
    let attrs = &object.attrs;
    match prop {
        Prop::Layer => Reading::Text(
            document
                .layer(attrs.layer)
                .map(|l| l.name.clone())
                .unwrap_or_default(),
        ),
        Prop::Color => Reading::Text(colour_text(&attrs.color)),
        Prop::Linetype => Reading::Text(match &attrs.linetype {
            Attr::ByLayer => "ByLayer".to_owned(),
            Attr::Set(id) => document
                .linetypes()
                .get(*id)
                .map_or_else(String::new, |r| r.name.clone()),
        }),
        Prop::Lineweight => Reading::Text(match &attrs.lineweight {
            Attr::ByLayer => "ByLayer".to_owned(),
            Attr::Set(w) => format!("{:.2} mm", f64::from(w.0) / 100.0),
        }),
        Prop::LinetypeScale => Reading::Number(attrs.linetype_scale),
        Prop::Field(f) => Reading::Number(properties::value(&object.entity, f)),
    }
}

/// The objects the search looks through: the selectable ones of the space
/// being edited, or those of them that are selected.
fn candidates<'a>(
    document: &'a Document,
    scope: Scope,
    selection: &'a std::collections::BTreeSet<EntityId>,
) -> impl Iterator<Item = &'a Object> {
    document
        .selectable_objects()
        .filter(move |o| scope == Scope::Drawing || selection.contains(&o.id))
}

/// The kinds of object in scope, alphabetically by name.
pub fn kinds_in(
    document: &Document,
    scope: Scope,
    selection: &std::collections::BTreeSet<EntityId>,
) -> Vec<EntityKind> {
    let mut kinds: Vec<_> = candidates(document, scope, selection)
        .map(|o| o.entity.kind())
        .collect();
    kinds.sort_unstable();
    kinds.dedup();
    kinds
}

/// The properties offered for `kind`: layer, colour and the other common
/// ones, then those the kind's objects show in the Properties panel.
pub fn props_for(
    document: &Document,
    scope: Scope,
    selection: &std::collections::BTreeSet<EntityId>,
    kind: Option<EntityKind>,
) -> Vec<(Prop, String)> {
    let mut rows: Vec<properties::Row> = Vec::new();
    if let Some(kind) = kind {
        for o in candidates(document, scope, selection).filter(|o| o.entity.kind() == kind) {
            for row in properties::rows(&o.entity) {
                if !rows.iter().any(|r| r.field == row.field) {
                    rows.push(*row);
                }
            }
        }
    }
    let mut props = vec![
        Prop::Color,
        Prop::Layer,
        Prop::Linetype,
        Prop::LinetypeScale,
        Prop::Lineweight,
    ];
    props.extend(rows.iter().map(|r| Prop::Field(r.field)));
    props.into_iter().map(|p| (p, p.label(&rows))).collect()
}

fn show(units: &Units, prop: Prop, reading: &Reading) -> String {
    match (reading, prop.unit()) {
        (Reading::Text(t), _) => t.clone(),
        (Reading::Number(n), Some(unit)) => properties::format(units, unit, *n),
        (Reading::Number(n), None) => n.to_string(),
    }
}

/// The values objects of `kind` have for `prop`, as the dialog lists them.
pub fn values_present(
    document: &Document,
    units: &Units,
    draft: &Draft,
    selection: &std::collections::BTreeSet<EntityId>,
) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for o in candidates(document, draft.scope, selection)
        .filter(|o| draft.kind.is_none_or(|k| o.entity.kind() == k))
    {
        let reading = read(document, o, draft.prop);
        if matches!(reading, Reading::Number(n) if n.is_nan()) {
            continue;
        }
        let text = show(units, draft.prop, &reading);
        if !seen.contains(&text) {
            seen.push(text);
        }
    }
    seen
}

/// Whether the typed value can be used with the operator.
pub fn value_is_usable(units: &Units, draft: &Draft) -> bool {
    match (draft.operator, draft.prop.unit()) {
        (Operator::All, _) => true,
        (_, Some(unit)) if draft.prop.typed() => {
            properties::parse(units, unit, &draft.value).is_some()
        }
        _ => !draft.value.is_empty(),
    }
}

/// Whether `text` fits `pattern`, where `*` matches any run and `?` any
/// one character, ignoring case.
pub(crate) fn wildcard_match(pattern: &str, text: &str) -> bool {
    let (p, t): (Vec<char>, Vec<char>) = (pattern.chars().collect(), text.chars().collect());
    wildcard(&p, &t)
}

/// `*` matches any run and `?` any one character, ignoring case.
fn wildcard(pattern: &[char], text: &[char]) -> bool {
    // Remember the last `*` and how much text it has taken so far; a later
    // mismatch retries with that star taking one more character. Linear in
    // practice, where trying every split of every star is exponential.
    let (mut p, mut t) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        match pattern.get(p) {
            Some('*') => {
                star = Some((p, t));
                p += 1;
            }
            Some(c) if *c == '?' || c.eq_ignore_ascii_case(&text[t]) => {
                p += 1;
                t += 1;
            }
            _ => match star {
                Some((sp, st)) => {
                    star = Some((sp, st + 1));
                    p = sp + 1;
                    t = st + 1;
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|c| *c == '*')
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6 * b.abs().max(1.0)
}

/// Whether an object reading passes the draft's test.
fn passes(units: &Units, draft: &Draft, reading: &Reading) -> bool {
    if draft.operator == Operator::All {
        return true;
    }
    match reading {
        Reading::Number(n) => {
            let target = match draft.prop.unit() {
                Some(unit) if draft.prop.typed() => properties::parse(units, unit, &draft.value),
                // Picked from a list: the value is as the list shows it.
                Some(unit) => (0..=64)
                    .map(f64::from)
                    .find(|v| properties::format(units, unit, *v) == draft.value),
                None => None,
            };
            let Some(target) = target else { return false };
            match draft.operator {
                Operator::Equals => close(*n, target),
                Operator::NotEqual => !close(*n, target),
                Operator::Greater => *n > target && !close(*n, target),
                Operator::Less => *n < target && !close(*n, target),
                _ => false,
            }
        }
        Reading::Text(t) => match draft.operator {
            Operator::Equals => t.eq_ignore_ascii_case(&draft.value),
            Operator::NotEqual => !t.eq_ignore_ascii_case(&draft.value),
            Operator::Wildcard => {
                let pattern: Vec<char> = draft.value.chars().collect();
                wildcard(&pattern, &t.chars().collect::<Vec<_>>())
            }
            _ => false,
        },
    }
}

/// The objects the draft selects: of the kind and passing the test (Include)
/// or everything else in scope (Exclude).
pub fn run(
    document: &Document,
    units: &Units,
    draft: &Draft,
    selection: &std::collections::BTreeSet<EntityId>,
) -> Vec<EntityId> {
    candidates(document, draft.scope, selection)
        .filter(|o| {
            let of_kind = draft.kind.is_none_or(|k| o.entity.kind() == k);
            let hit = of_kind && passes(units, draft, &read(document, o, draft.prop));
            hit == draft.include
        })
        .map(|o| o.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use od_core::{Entity, Point};

    use super::*;

    fn circle(r: f64) -> Entity {
        Entity::Circle {
            center: Point::new(0.0, 0.0),
            radius: r,
        }
    }

    fn line() -> Entity {
        Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(1.0, 0.0),
        }
    }

    /// Circles of diameter 2, 6 and 12, and a line.
    fn drawing() -> (Document, Vec<EntityId>) {
        let mut doc = Document::default();
        let ids = [circle(1.0), circle(3.0), circle(6.0), line()]
            .into_iter()
            .map(|e| doc.add(e))
            .collect();
        (doc, ids)
    }

    fn draft(operator: Operator, value: &str) -> Draft {
        Draft {
            kind: Some(EntityKind::Circle),
            prop: Prop::Field(Field::Diameter),
            operator,
            value: value.to_owned(),
            ..Draft::default()
        }
    }

    fn found(doc: &Document, d: &Draft) -> Vec<EntityId> {
        run(doc, doc.units(), d, &BTreeSet::new())
    }

    #[test]
    fn equals_finds_the_circle_of_that_diameter() {
        let (doc, ids) = drawing();
        assert_eq!(found(&doc, &draft(Operator::Equals, "6.00")), [ids[1]]);
    }

    #[test]
    fn greater_and_less_than_compare_numbers_and_stay_in_the_kind() {
        let (doc, ids) = drawing();
        assert_eq!(
            found(&doc, &draft(Operator::Greater, "2")),
            [ids[1], ids[2]]
        );
        assert_eq!(found(&doc, &draft(Operator::Less, "12")), [ids[0], ids[1]]);
        assert_eq!(
            found(&doc, &draft(Operator::All, "")),
            [ids[0], ids[1], ids[2]]
        );
    }

    #[test]
    fn exclude_takes_everything_else_in_scope() {
        let (doc, ids) = drawing();
        let mut d = draft(Operator::Equals, "6");
        d.include = false;
        assert_eq!(found(&doc, &d), [ids[0], ids[2], ids[3]]);
    }

    #[test]
    fn the_current_selection_limits_the_search() {
        let (doc, ids) = drawing();
        let selection: BTreeSet<_> = [ids[1], ids[3]].into();
        let mut d = draft(Operator::All, "");
        d.scope = Scope::Selection;
        assert_eq!(run(&doc, doc.units(), &d, &selection), [ids[1]]);
        d.kind = None;
        assert_eq!(run(&doc, doc.units(), &d, &selection), [ids[1], ids[3]]);
    }

    fn matches(pattern: &str, text: &str) -> bool {
        let p: Vec<char> = pattern.chars().collect();
        let t: Vec<char> = text.chars().collect();
        wildcard(&p, &t)
    }

    #[test]
    fn wildcard_follows_star_and_question_mark_rules() {
        assert!(matches("", ""));
        assert!(!matches("", "a"));
        assert!(matches("*", ""));
        assert!(matches("**", "abc"));
        assert!(matches("A*C", "abc"));
        assert!(matches("a*b*c", "aXXbYYc"));
        assert!(!matches("a*b*c", "aXXbYY"));
        assert!(matches("a?c", "ABC"));
        assert!(!matches("a?c", "ac"));
        assert!(matches("*b", "abb"));
        assert!(!matches("a", "ab"));
        assert!(matches("a*", "a"));
    }

    #[test]
    fn wildcard_does_not_blow_up_on_repeated_stars() {
        let text = "a".repeat(5_000);
        assert!(!matches(&format!("{}b", "*a".repeat(30)), &text));
        assert!(matches(&format!("{}a", "*a".repeat(30)), &text));
    }

    #[test]
    fn layers_match_by_name_and_by_wildcard() {
        let mut doc = Document::default();
        let a = doc.allocate_layer_id();
        doc.push_layer(od_core::Layer::new(a, "Walls-Ext"));
        let on_walls = doc.add_with(circle(1.0), od_core::Attributes::on_layer(a));
        doc.add(circle(2.0));
        let mut d = Draft {
            prop: Prop::Layer,
            value: "walls-ext".to_owned(),
            ..Draft::default()
        };
        assert_eq!(found(&doc, &d), [on_walls]);
        d.operator = Operator::Wildcard;
        d.value = "wall*".to_owned();
        assert_eq!(found(&doc, &d), [on_walls]);
        d.value = "w?lls-e??".to_owned();
        assert_eq!(found(&doc, &d), [on_walls]);
    }

    #[test]
    fn the_dialog_offers_what_the_drawing_has() {
        let (doc, _) = drawing();
        let none = BTreeSet::new();
        assert_eq!(
            kinds_in(&doc, Scope::Drawing, &none),
            [EntityKind::Circle, EntityKind::Line]
        );
        let props = props_for(&doc, Scope::Drawing, &none, Some(EntityKind::Circle));
        assert!(props.iter().any(|(_, l)| l == "Diameter"));
        let any = props_for(&doc, Scope::Drawing, &none, None);
        assert!(!any.iter().any(|(_, l)| l == "Diameter"));
        let d = draft(Operator::Equals, "");
        assert_eq!(
            values_present(&doc, doc.units(), &d, &none),
            ["2.0000", "6.0000", "12.0000"]
        );
    }

    #[test]
    fn a_value_that_is_not_a_number_is_not_usable() {
        let (doc, _) = drawing();
        assert!(value_is_usable(doc.units(), &draft(Operator::Equals, "6")));
        assert!(!value_is_usable(
            doc.units(),
            &draft(Operator::Equals, "six")
        ));
        assert!(value_is_usable(doc.units(), &draft(Operator::All, "")));
    }
}
