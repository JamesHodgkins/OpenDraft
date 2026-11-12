//! OVERKILL: remove duplicate objects and merge lines and arcs that overlap
//! or meet end to end on the same line or circle. Pure; the command only
//! asks which objects, groups them by the properties that must match, marks
//! the ones that must not change, and applies the outcome.

mod equal;
mod merge;
mod polyline;
mod prepare;
#[cfg(test)]
mod prepare_tests;
#[cfg(test)]
mod tests;

pub use prepare::{prepare, Prepared};

use crate::Entity;

/// Properties that may differ between two objects that are still compared.
/// The caller applies these when it groups the objects.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ignore {
    pub layer: bool,
    pub color: bool,
    pub linetype: bool,
    pub linetype_scale: bool,
    pub lineweight: bool,
}

/// What OVERKILL may do beyond removing exact duplicates and objects lying
/// wholly on top of another.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// How far apart two numbers may be and still count as equal; zero asks
    /// for an exact match.
    pub tolerance: f64,
    pub ignore: Ignore,
    /// Drop repeated vertices and the middle vertex of straight runs in
    /// polylines.
    pub optimize_polylines: bool,
    /// Merge objects that overlap only in part into one that spans both.
    pub overlapping: bool,
    /// Merge objects that touch end to end into one.
    pub end_to_end: bool,
    /// Leave alone the objects that others (hatches, dimensions, leaders)
    /// are tied to. The caller marks them [`Candidate::protected`].
    pub maintain_associative: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            tolerance: 1e-6,
            ignore: Ignore::default(),
            optimize_polylines: true,
            overlapping: true,
            end_to_end: true,
            maintain_associative: true,
        }
    }
}

impl Settings {
    /// The tolerance has to be a number that is not negative.
    pub fn validate(&self) -> Result<(), OverkillError> {
        if self.tolerance.is_finite() && self.tolerance >= 0.0 {
            Ok(())
        } else {
            Err(OverkillError::BadTolerance)
        }
    }
}

/// One object to tidy, and what the caller knows about it.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub entity: Entity,
    /// Only objects of one group are compared (the caller groups by layout,
    /// layer, colour and so on).
    pub group: usize,
    /// Must not be deleted or changed, though a duplicate of it may go.
    pub protected: bool,
}

/// What to do to the objects given, by position.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    /// Objects that become something else (a longer line, a bigger arc).
    pub changed: Vec<(usize, Entity)>,
    /// Objects to delete, lowest first.
    pub removed: Vec<usize>,
}

/// Why OVERKILL did nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OverkillError {
    BadTolerance,
    TooManyObjects { count: usize, limit: usize },
    TooMuchWork { limit: u64 },
}

impl std::fmt::Display for OverkillError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OverkillError::BadTolerance => write!(f, "the tolerance must be zero or more"),
            OverkillError::TooManyObjects { count, limit } => {
                write!(f, "{count} objects is more than the {limit} it can compare")
            }
            OverkillError::TooMuchWork { limit } => {
                write!(f, "more than {limit} comparisons were needed")
            }
        }
    }
}

/// The most objects one run compares, and the most pairs it tests.
pub const MAX_OBJECTS: usize = 100_000;
pub const MAX_COMPARISONS: u64 = 25_000_000;

/// Tidy `items`. Fails, changing nothing, on a bad tolerance or when the
/// selection is too big to compare.
pub fn overkill(items: &[Candidate], settings: &Settings) -> Result<Outcome, OverkillError> {
    overkill_capped(items, settings, MAX_OBJECTS, MAX_COMPARISONS)
}

/// [`overkill`] with its own limits, for a caller that must stay quick (a
/// look that runs while the user waits) and so compares less.
pub fn overkill_capped(
    items: &[Candidate],
    settings: &Settings,
    max_objects: usize,
    max_comparisons: u64,
) -> Result<Outcome, OverkillError> {
    settings.validate()?;
    if items.len() > max_objects {
        return Err(OverkillError::TooManyObjects {
            count: items.len(),
            limit: max_objects,
        });
    }
    let tol = settings.tolerance;
    let mut now: Vec<Entity> = items.iter().map(|c| c.entity.clone()).collect();
    if settings.optimize_polylines {
        for (e, c) in now.iter_mut().zip(items) {
            if let Some(better) = polyline::optimized(e, tol).filter(|_| !c.protected) {
                *e = better;
            }
        }
    }
    let mut gone = vec![false; items.len()];
    let mut work = 0u64;
    let mut count = |n: u64| {
        work += n;
        if work > max_comparisons {
            Err(OverkillError::TooMuchWork {
                limit: max_comparisons,
            })
        } else {
            Ok(())
        }
    };
    for i in 0..items.len() {
        for j in i + 1..items.len() {
            if gone[i] {
                break;
            }
            count(1)?;
            if gone[j] || items[i].group != items[j].group {
                continue;
            }
            if equal::same(&now[i], &now[j], tol) {
                // The one that must stay survives, whichever came first.
                match (items[i].protected, items[j].protected) {
                    (true, true) => {}
                    (false, true) => gone[i] = true,
                    _ => gone[j] = true,
                }
            }
        }
    }
    for i in 0..items.len() {
        let mut j = i + 1;
        while j < items.len() {
            count(1)?;
            if !gone[i] && !gone[j] && items[i].group == items[j].group && !items[j].protected {
                if let Some(merged) = merge::combine(&now[i], &now[j], settings) {
                    // A protected object only ever absorbs what lies on it.
                    if !items[i].protected || merged == now[i] {
                        now[i] = merged;
                        gone[j] = true;
                        // The longer object may now reach ones passed over.
                        j = i + 1;
                        continue;
                    }
                }
            }
            j += 1;
        }
    }
    let changed = (0..items.len())
        .filter(|&i| !gone[i] && now[i] != items[i].entity)
        .map(|i| (i, now[i].clone()))
        .collect();
    let removed = (0..items.len()).filter(|&i| gone[i]).collect();
    Ok(Outcome { changed, removed })
}
