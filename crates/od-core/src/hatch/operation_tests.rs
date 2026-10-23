//! What each operation on an entity does with a hatch (the table in the
//! specification's section 4.8): supported, or refused without damage.

use super::testing::*;
use crate::{Document, DocumentError, GripKind, Layout, Viewport, ViewportId};

fn hatch_entity(h: HatchEntity) -> Entity {
    Entity::Hatch(Box::new(h))
}

fn striped() -> Entity {
    let pattern = catalog_pattern("LINE", 2.0, 0.0, p(0.0, 0.0));
    hatch_entity(patterned(vec![square(0.0, 0.0, 10.0)], pattern))
}

fn filled() -> Entity {
    hatch_entity(solid(vec![square(0.0, 0.0, 10.0)], HatchStyle::Normal))
}

#[test]
fn it_has_a_name_and_bounds_and_one_grip_that_moves_it() {
    let h = striped();
    assert_eq!(h.kind(), crate::EntityKind::Hatch);
    assert_eq!(h.bounds().max, p(10.0, 10.0));
    let grips = h.grips();
    assert_eq!(grips.len(), 1);
    assert_eq!((grips[0].at, grips[0].kind), (p(5.0, 5.0), GripKind::Move));
    let moved = h.with_grip_moved(0, p(8.0, 5.0)).unwrap();
    assert_eq!(moved.bounds().min, p(3.0, 0.0));
    assert!(h.with_grip_moved(1, p(0.0, 0.0)).is_none());
}

#[test]
fn stretch_moves_a_hatch_only_when_the_window_holds_all_of_it() {
    let h = filled();
    let all = h.stretched(
        |q| q.x >= -1.0 && q.x <= 11.0 && q.y >= -1.0 && q.y <= 11.0,
        5.0,
        0.0,
    );
    assert_eq!(all.unwrap().bounds().min, p(5.0, 0.0));
    let part = h.stretched(|q| q.x >= 4.0, 5.0, 0.0);
    assert!(
        part.is_none(),
        "its boundary is not stretched piece by piece"
    );
}

#[test]
fn trim_extend_fillet_offset_break_join_and_explode_do_not_apply() {
    for h in [striped(), filled()] {
        assert!(h.curves().is_empty(), "never an edge");
        assert!(h.offset(1.0, p(1.0, 1.0)).is_none());
        assert_eq!(h.split_at(&[p(5.0, 0.0)]), vec![h.clone()]);
        assert!(h.explode().is_none());
        assert!(h.mirror_refusal().is_none() && h.scale_refusal().is_none());
    }
}

#[test]
fn the_document_checks_a_hatch_and_refuses_one_in_a_block() {
    let mut doc = Document::default();
    let id = doc.add(filled());
    assert_eq!(doc.validate(), Ok(()));
    let Entity::Hatch(mut broken) = filled() else {
        panic!("hatch");
    };
    broken.elevation = f64::NAN;
    doc.replace(id, Entity::Hatch(broken));
    assert!(matches!(doc.validate(), Err(DocumentError::BadHatch(i, _)) if i == id));
    // A block definition cannot hold one.
    let doc = Document::default();
    let def = crate::BlockDef::from_objects(
        crate::BlockId(doc.next_block_id()),
        "B",
        p(0.0, 0.0),
        Document::from_entities([filled()]).objects(),
    );
    assert!(doc.redefined_blocks(def).is_err());
}

/// A drawing with `entity` in model space and a 1:1 layout whose viewport
/// shows model coordinates 0..100 on a 100 x 100 mm window.
fn plotted(entity: Entity) -> crate::plot::PlotSheet {
    let mut doc = Document::default();
    doc.add(entity);
    let id = doc.allocate_layout_id();
    let viewport = Viewport {
        id: ViewportId(1),
        center: p(100.0, 100.0),
        width: 100.0,
        height: 100.0,
        view_center: p(50.0, 50.0),
        scale: 1.0,
        locked: false,
        on: true,
        frozen: Vec::new(),
    };
    doc.set_layouts(vec![Layout {
        id,
        name: "A".into(),
        width: 297.0,
        height: 210.0,
        viewports: vec![viewport],
    }]);
    crate::plot::plot_layout(&doc, id).unwrap()
}

#[test]
fn a_pattern_plots_as_its_strokes_and_a_solid_as_a_fill_with_its_islands() {
    let sheet = plotted(striped());
    // LINE at scale 2 is 0.25 apart: lines at 0 to 10 over the 10 mm square,
    // the two along its edges included.
    assert_eq!(sheet.paths.len(), 41);
    assert!(sheet.fills.is_empty());
    assert!(sheet
        .paths
        .iter()
        .all(|path| path.points.len() == 2 && !path.closed));
    let islands = hatch_entity(solid(
        vec![square(0.0, 0.0, 40.0), square(10.0, 10.0, 10.0)],
        HatchStyle::Normal,
    ));
    let sheet = plotted(islands);
    assert!(sheet.paths.is_empty());
    assert_eq!(sheet.fills.len(), 1);
    let contours = &sheet.fills[0].contours;
    assert_eq!(contours.len(), 2);
    // Outer counter-clockwise, island clockwise: the non-zero rule fills the
    // ring between them.
    let signed = |c: &Vec<crate::Point>| -> f64 {
        (0..c.len())
            .map(|i| {
                let (a, b) = (c[i], c[(i + 1) % c.len()]);
                a.x * b.y - b.x * a.y
            })
            .sum::<f64>()
            / 2.0
    };
    let mut areas: Vec<f64> = contours.iter().map(signed).collect();
    areas.sort_by(f64::total_cmp);
    assert!(areas[0] < 0.0 && areas[1] > 0.0, "{areas:?}");
    assert!((areas[0].abs() - 100.0).abs() < 1e-6 && (areas[1] - 1600.0).abs() < 1e-6);
}

#[test]
fn a_pattern_too_dense_to_plot_is_an_error_not_a_hang() {
    let pattern = catalog_pattern("LINE", 1e-7, 0.0, p(0.0, 0.0));
    let mut doc = Document::default();
    doc.add(hatch_entity(patterned(
        vec![square(0.0, 0.0, 100.0)],
        pattern,
    )));
    let id = doc.allocate_layout_id();
    doc.set_layouts(vec![Layout {
        id,
        name: "A".into(),
        width: 297.0,
        height: 210.0,
        viewports: vec![Viewport {
            id: ViewportId(1),
            center: p(100.0, 100.0),
            width: 100.0,
            height: 100.0,
            view_center: p(50.0, 50.0),
            scale: 1.0,
            locked: false,
            on: true,
            frozen: Vec::new(),
        }],
    }]);
    assert_eq!(
        crate::plot::plot_layout(&doc, id),
        Err(crate::plot::PlotError::HatchTooDense)
    );
}
