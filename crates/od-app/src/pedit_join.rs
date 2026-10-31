//! PEDIT's Join: which of the chosen objects may join which polyline, and
//! doing it as one undo step. The chaining is `od_core::pedit`.

use od_commands::{Answer, Cancelled, Editor, NumberOptions, Options};
use od_core::pedit::{JoinType, PolyData, MAX_JOIN_CANDIDATES};
use od_core::{Attributes, Entity, EntityId, LayoutId};

use crate::defaults::EditorDefaults;
use crate::text::plural;

/// An object that may take part in a join, as the drawing holds it.
struct Item {
    id: EntityId,
    entity: Entity,
    attrs: Attributes,
    layout: Option<LayoutId>,
    locked: bool,
}

/// What is joined to what: the seeds that may take pieces on, the pieces
/// they took, and the object that results.
struct JoinPlan {
    keep: EntityId,
    entity: Entity,
    gone: Vec<EntityId>,
    added: usize,
}

/// Plan the joins among `items`, where only the first `seeds` start a
/// polyline. Objects on locked layers or in another space than the seed do
/// not take part.
fn plan_joins(items: &[Item], seeds: usize, fuzz: f64, kind: JoinType) -> Vec<JoinPlan> {
    let data: Vec<Option<PolyData>> = items.iter().map(|i| PolyData::of(&i.entity)).collect();
    let mut taken = vec![false; items.len()];
    let mut plans = Vec::new();
    for seed in 0..seeds.min(items.len()) {
        let Some(base) = data[seed]
            .clone()
            .filter(|_| !taken[seed] && !items[seed].locked)
        else {
            continue;
        };
        let pool: Vec<usize> = (0..items.len())
            .filter(|&j| {
                j != seed
                    && !taken[j]
                    && data[j].is_some()
                    && !items[j].locked
                    && items[j].layout == items[seed].layout
            })
            .collect();
        let pieces: Vec<PolyData> = pool.iter().filter_map(|&j| data[j].clone()).collect();
        let Some((joined, used)) = base.joined(&pieces, fuzz, kind) else {
            continue;
        };
        let gone: Vec<EntityId> = used.iter().map(|&u| items[pool[u]].id).collect();
        for &u in &used {
            taken[pool[u]] = true;
        }
        taken[seed] = true;
        let segments = |p: &PolyData| od_core::bulge::segment_count(p.points.len(), p.closed);
        plans.push(JoinPlan {
            keep: items[seed].id,
            added: segments(&joined) - segments(&base),
            entity: joined.into_entity(),
            gone,
        });
    }
    plans
}

/// Join `others` onto each of `seeds` (or the seeds among themselves when
/// `others` is empty), in one undo step. Returns how many segments were added.
pub(crate) fn join_group(
    ed: &Editor,
    seeds: Vec<EntityId>,
    others: Vec<EntityId>,
    fuzz: f64,
    kind: JoinType,
) -> Result<usize, Cancelled> {
    if others.len() > MAX_JOIN_CANDIDATES {
        ed.say(format!(
            "Too many objects to join (the limit is {MAX_JOIN_CANDIDATES})."
        ));
        return Ok(0);
    }
    let count = seeds.len();
    let ids: Vec<EntityId> = seeds
        .iter()
        .copied()
        .chain(others.iter().copied().filter(|o| !seeds.contains(o)))
        .collect();
    let items: Vec<Item> = ed.read(move |doc| {
        ids.iter()
            .filter_map(|id| {
                let o = doc.object(*id)?;
                Some(Item {
                    id: *id,
                    entity: o.entity.clone(),
                    attrs: o.attrs.clone(),
                    layout: o.layout,
                    locked: doc.layer(o.attrs.layer).is_some_and(|l| l.locked),
                })
            })
            .collect()
    })?;
    let plans = plan_joins(&items, count, fuzz, kind);
    let added: usize = plans.iter().map(|p| p.added).sum();
    if plans.is_empty() {
        ed.say("0 segments added to polyline.");
        return Ok(0);
    }
    let attrs_of = |id: EntityId| items.iter().find(|i| i.id == id).map(|i| &i.attrs);
    let differing = plans
        .iter()
        .flat_map(|p| p.gone.iter().map(move |g| (p.keep, *g)))
        .filter(|(keep, gone)| attrs_of(*keep) != attrs_of(*gone))
        .count();
    ed.transaction("Join", move |tx| {
        for plan in plans {
            let entity = plan.entity;
            tx.modify(plan.keep, move |_| entity);
            tx.remove(plan.gone);
        }
        Ok(())
    })?;
    ed.say(format!("{} added to polyline.", plural(added, "segment")));
    if differing > 0 {
        ed.say(format!(
            "{} had other properties; the polyline's were kept.",
            plural(differing, "joined object")
        ));
    }
    Ok(added)
}

/// Multiple's Join prompt: the fuzz distance, or the Jointype to change.
pub(crate) fn ask_fuzz(ed: &Editor) -> Result<(f64, JoinType), Cancelled> {
    loop {
        let options = NumberOptions::new("Enter fuzz distance")
            .keyword("Jointype", "J")
            .default(0.0);
        match ed.ask_distance(options)? {
            Answer::Value(fuzz) if fuzz >= 0.0 => {
                return Ok((fuzz, ed.defaults().pedit.get().join_type))
            }
            Answer::Value(_) => ed.say("Requires a value of zero or more."),
            Answer::Keyword(_) => {
                let current = match ed.defaults().pedit.get().join_type {
                    JoinType::Extend => "Extend",
                    JoinType::Add => "Add",
                    JoinType::Both => "Both",
                };
                let answer = ed.get_keyword(
                    Options::new("Enter join type")
                        .keyword("Extend", "E")
                        .keyword("Add", "A")
                        .keyword("Both", "B")
                        .default(current.to_owned()),
                )?;
                ed.defaults().pedit.change(|m| {
                    m.join_type = match answer.as_str() {
                        "Add" => JoinType::Add,
                        "Both" => JoinType::Both,
                        _ => JoinType::Extend,
                    }
                });
            }
            Answer::Enter => return Ok((0.0, ed.defaults().pedit.get().join_type)),
        }
    }
}
