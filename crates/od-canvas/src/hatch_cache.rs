//! What a hatch draws, kept between frames.
//!
//! Working out a hatch (its fill area and pattern strokes) is far more than
//! drawing it, so the result is kept per object and thrown away when the
//! hatch changes. The cache is checked by content, not only told: each entry
//! keeps the hatch it was made from and is reused only while the hatch is
//! equal to it, so an edit, an undo, or another drawing's object that happens
//! to share an id can never show a stale picture. [`HatchCache::invalidate`]
//! frees entries early (the app calls it for the ids of a `Changes`).
//!
//! Curves are followed to a tolerance in whole powers of two, so zooming
//! rebuilds a hatch only when it crosses one.

use std::cell::OnceCell;
use std::collections::HashMap;
use std::rc::Rc;

use lyon_tessellation::math::point;
use lyon_tessellation::path::Path;
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, VertexBuffers,
};
use od_core::hatch::{EvaluatedHatch, HatchEntity, HatchError, HatchFill, HatchLimits};
use od_core::{EntityId, Point};

/// Most hatches kept before the cache is emptied and started again.
const MAX_ENTRIES: usize = 512;
/// Most strokes kept over all entries.
const MAX_KEPT_STROKES: usize = 2_000_000;

/// Triangles of a filled area, relative to `origin` so that single
/// precision is enough far from the drawing's origin.
pub struct SolidMesh {
    pub origin: Point,
    pub vertices: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

/// A hatch that was worked out.
pub struct Worked {
    pub evaluated: EvaluatedHatch,
    /// The smallest distance between pattern lines in drawing units, or
    /// `None` for a solid fill.
    pub spacing: Option<f64>,
    mesh: OnceCell<Option<SolidMesh>>,
}

impl Worked {
    /// The triangles of the filled area, made on first use.
    pub fn mesh(&self) -> Option<&SolidMesh> {
        self.mesh
            .get_or_init(|| tessellate(&self.evaluated))
            .as_ref()
    }
}

/// The outcome for one hatch.
pub enum Outcome {
    Worked(Worked),
    /// It could not be worked out (too dense, a broken boundary); what can
    /// still be shown is its boundary.
    Failed {
        error: HatchError,
        boundary: Vec<Vec<Point>>,
    },
}

struct Entry {
    hatch: HatchEntity,
    bucket: i32,
    outcome: Rc<Outcome>,
}

/// Hatches worked out so far, by object.
#[derive(Default)]
pub struct HatchCache {
    entries: HashMap<EntityId, Entry>,
    /// The one hatch that is not in the drawing yet (a command's preview).
    preview: Option<Entry>,
    strokes: usize,
}

fn strokes_of(outcome: &Outcome) -> usize {
    match outcome {
        Outcome::Worked(w) => w.evaluated.strokes.len() + w.evaluated.dots.len(),
        Outcome::Failed { .. } => 0,
    }
}

/// The smallest gap between the lines of a pattern.
fn spacing_of(hatch: &HatchEntity) -> Option<f64> {
    let HatchFill::Pattern(p) = &hatch.fill else {
        return None;
    };
    p.lines
        .iter()
        .map(|l| {
            let (s, c) = l.angle.sin_cos();
            (l.offset[0] * -s + l.offset[1] * c).abs()
        })
        .reduce(f64::min)
}

fn work(hatch: &HatchEntity, tolerance: f64, limits: &HatchLimits) -> Outcome {
    match hatch.evaluate(tolerance, limits) {
        Ok(evaluated) => Outcome::Worked(Worked {
            evaluated,
            spacing: spacing_of(hatch),
            mesh: OnceCell::new(),
        }),
        Err(error) => Outcome::Failed {
            error,
            boundary: hatch.contours(tolerance).unwrap_or_default(),
        },
    }
}

fn tessellate(done: &EvaluatedHatch) -> Option<SolidMesh> {
    let origin = Point::new(
        (done.bounds.min.x + done.bounds.max.x) / 2.0,
        (done.bounds.min.y + done.bounds.max.y) / 2.0,
    );
    let mut builder = Path::builder();
    for contour in &done.fill {
        let mut points = contour
            .iter()
            .map(|p| point((p.x - origin.x) as f32, (p.y - origin.y) as f32));
        let Some(first) = points.next() else {
            continue;
        };
        builder.begin(first);
        for p in points {
            builder.line_to(p);
        }
        builder.end(true);
    }
    let mut buffers: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
    // The loops are oriented so that both rules agree.
    let options = FillOptions::default().with_fill_rule(FillRule::EvenOdd);
    FillTessellator::new()
        .tessellate_path(
            &builder.build(),
            &options,
            &mut BuffersBuilder::new(&mut buffers, |v: FillVertex| v.position().to_array()),
        )
        .ok()?;
    Some(SolidMesh {
        origin,
        vertices: buffers.vertices,
        indices: buffers.indices,
    })
}

/// The tolerance bucket (a power of two) at or below `tolerance`.
fn bucket_of(tolerance: f64) -> i32 {
    tolerance.max(1e-12).log2().floor().clamp(-40.0, 40.0) as i32
}

impl HatchCache {
    /// What `hatch` draws, with curves followed to about `tolerance`. `id`
    /// is the object it belongs to, or `None` for a preview.
    pub fn get(
        &mut self,
        id: Option<EntityId>,
        hatch: &HatchEntity,
        tolerance: f64,
        limits: &HatchLimits,
    ) -> Rc<Outcome> {
        let bucket = bucket_of(tolerance);
        let found = match id {
            Some(id) => self.entries.get(&id),
            None => self.preview.as_ref(),
        };
        if let Some(e) = found.filter(|e| e.bucket == bucket && e.hatch == *hatch) {
            return Rc::clone(&e.outcome);
        }
        let outcome = Rc::new(work(hatch, f64::from(bucket).exp2(), limits));
        let entry = Entry {
            hatch: hatch.clone(),
            bucket,
            outcome: Rc::clone(&outcome),
        };
        match id {
            Some(id) => {
                if self.entries.len() >= MAX_ENTRIES || self.strokes > MAX_KEPT_STROKES {
                    self.clear();
                }
                if let Some(old) = self.entries.insert(id, entry) {
                    self.strokes = self.strokes.saturating_sub(strokes_of(&old.outcome));
                }
                self.strokes += strokes_of(&outcome);
            }
            None => self.preview = Some(entry),
        }
        outcome
    }

    /// Forget the objects with these ids (a command changed them).
    pub fn invalidate(&mut self, ids: impl IntoIterator<Item = EntityId>) {
        for id in ids {
            if let Some(old) = self.entries.remove(&id) {
                self.strokes = self.strokes.saturating_sub(strokes_of(&old.outcome));
            }
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.preview = None;
        self.strokes = 0;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use od_core::hatch::{
        HatchBoundary, HatchLoop, HatchLoopFlags, HatchPattern, HatchPolyline, PatternCatalog,
        PatternType,
    };

    use super::*;

    fn square(side: f64) -> HatchLoop {
        HatchLoop {
            flags: HatchLoopFlags::default(),
            boundary: HatchBoundary::Polyline(HatchPolyline {
                vertices: vec![
                    Point::new(0.0, 0.0),
                    Point::new(side, 0.0),
                    Point::new(side, side),
                    Point::new(0.0, side),
                ],
                bulges: Vec::new(),
                closed: true,
            }),
            sources: Vec::new(),
        }
    }

    fn striped(side: f64) -> HatchEntity {
        let def = PatternCatalog::parse("*L, l\n0, 0,0, 0,1\n");
        let p = HatchPattern::from_definition(
            def.find("L").unwrap(),
            PatternType::Custom,
            1.0,
            0.0,
            Point::default(),
        )
        .unwrap();
        HatchEntity::new(vec![square(side)], HatchFill::Pattern(p))
    }

    #[test]
    fn a_hatch_is_worked_out_once_and_again_only_when_it_changes() {
        let mut cache = HatchCache::default();
        let limits = HatchLimits::default();
        let id = EntityId(1);
        let h = striped(10.0);
        let a = cache.get(Some(id), &h, 0.01, &limits);
        let b = cache.get(Some(id), &h, 0.01, &limits);
        assert!(Rc::ptr_eq(&a, &b), "the same hatch is reused");
        // The same id with another hatch (an edit, or another drawing).
        let c = cache.get(Some(id), &striped(20.0), 0.01, &limits);
        assert!(!Rc::ptr_eq(&a, &c));
        let Outcome::Worked(w) = &*c else {
            panic!("worked out");
        };
        // Lines at y = 0 to 20, the two along the edges included.
        assert_eq!(w.evaluated.strokes.len(), 21);
        // A tolerance in the same power of two is the same entry; another is not.
        let d = cache.get(Some(id), &striped(20.0), 0.012, &limits);
        assert!(Rc::ptr_eq(&c, &d));
        let e = cache.get(Some(id), &striped(20.0), 0.002, &limits);
        assert!(!Rc::ptr_eq(&c, &e));
    }

    #[test]
    fn invalidate_frees_entries_and_the_count_of_strokes() {
        let mut cache = HatchCache::default();
        let limits = HatchLimits::default();
        cache.get(Some(EntityId(1)), &striped(10.0), 0.01, &limits);
        cache.get(Some(EntityId(2)), &striped(10.0), 0.01, &limits);
        assert_eq!(cache.len(), 2);
        cache.invalidate([EntityId(1)]);
        assert_eq!(cache.len(), 1);
        cache.invalidate([EntityId(2), EntityId(9)]);
        assert!(cache.is_empty());
        assert_eq!(cache.strokes, 0);
    }

    #[test]
    fn the_cache_stays_bounded() {
        let mut cache = HatchCache::default();
        let limits = HatchLimits::default();
        let h = striped(3.0);
        for i in 0..(MAX_ENTRIES as u64 * 2) {
            cache.get(Some(EntityId(i)), &h, 0.01, &limits);
        }
        assert!(cache.len() <= MAX_ENTRIES);
    }

    #[test]
    fn a_hatch_that_cannot_be_worked_out_still_has_its_boundary() {
        let mut cache = HatchCache::default();
        let dense = {
            let def = PatternCatalog::parse("*L, l\n0, 0,0, 0,1e-7\n");
            let p = HatchPattern::from_definition(
                def.find("L").unwrap(),
                PatternType::Custom,
                1.0,
                0.0,
                Point::default(),
            )
            .unwrap();
            HatchEntity::new(vec![square(100.0)], HatchFill::Pattern(p))
        };
        let out = cache.get(Some(EntityId(1)), &dense, 0.01, &HatchLimits::default());
        let Outcome::Failed { error, boundary } = &*out else {
            panic!("too dense");
        };
        assert!(matches!(error, HatchError::TooDense { .. }));
        assert_eq!(boundary.len(), 1);
    }

    #[test]
    fn a_solid_square_is_two_triangles_around_its_own_middle() {
        let mut cache = HatchCache::default();
        let h = HatchEntity::new(vec![square(10.0)], HatchFill::Solid);
        let out = cache.get(None, &h, 0.01, &HatchLimits::default());
        let Outcome::Worked(w) = &*out else {
            panic!("worked out");
        };
        let mesh = w.mesh().unwrap();
        assert_eq!(mesh.origin, Point::new(5.0, 5.0));
        assert_eq!(mesh.indices.len(), 6);
        assert!(mesh
            .vertices
            .iter()
            .all(|v| v[0].abs() <= 5.0 && v[1].abs() <= 5.0));
    }
}
