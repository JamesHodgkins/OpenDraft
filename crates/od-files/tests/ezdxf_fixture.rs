//! Reading a DXF written by an independent library (ezdxf, AutoCAD 2018
//! format). See `fixtures/make_ezdxf_sample.py` for what is in it.

use std::f64::consts::FRAC_1_SQRT_2;
use std::path::Path;

use od_core::{Attr, Color, Entity, Lineweight, Point};

fn sample() -> od_files::Loaded {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ezdxf_sample.dxf");
    od_files::load(&path).expect("the sample reads")
}

#[test]
fn it_is_an_import_and_says_what_it_left_out() {
    let loaded = sample();
    assert!(loaded.imported);
    assert_eq!(loaded.document.model_objects().count(), 13);
    // ezdxf adds three arrowhead blocks drawn "by block"; one holds a SOLID.
    // Their by-block properties are kept.
    assert_eq!(
        loaded.warnings,
        [
            // ezdxf's add_spline writes fit points alone; they are fitted.
            "Skipped 2 unsupported objects: 1 SOLID, 1 TEXT.",
        ]
    );
    assert_eq!(loaded.document.blocks().len(), 4);
    // add_ellipse((0, 0), major_axis=(3, 0), ratio=0.5): a full ellipse.
    let ellipse = loaded
        .document
        .model_objects()
        .find_map(|o| match o.entity {
            Entity::Ellipse(e) => Some(e),
            _ => None,
        });
    let ellipse = ellipse.expect("the ellipse");
    assert!(ellipse.is_full());
    assert_eq!(
        (ellipse.center, ellipse.major_axis, ellipse.radius_ratio),
        (Point::new(0.0, 0.0), Point::new(3.0, 0.0), 0.5)
    );
    // The block and its two references.
    let door = loaded.document.block_named("DOOR").expect("the block");
    assert_eq!(door.entities.len(), 1);
    assert_eq!(loaded.document.block_references(door.id).len(), 2);
    assert_eq!(loaded.document.validate(), Ok(()));
    // The paper space entity came in on the sample's layout, "Layout1".
    let layout = &loaded.document.layouts()[0];
    assert_eq!(layout.name, "Layout1");
    assert_eq!(loaded.document.objects_of_layout(layout.id).len(), 1);
}

fn layouts_sample() -> od_files::Loaded {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ezdxf_layouts.dxf");
    od_files::load(&path).expect("the layouts sample reads")
}

/// `ezdxf_layouts.dxf` (see `fixtures/make_ezdxf_layouts.py`): two layouts, one
/// with margins, a frozen layer in a viewport, a side view, an off locked viewport.
#[test]
fn layouts_and_viewports_come_in_with_their_sheets() {
    let loaded = layouts_sample();
    let doc = &loaded.document;
    let names: Vec<_> = doc.layouts().iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["Layout1", "Plan"]);
    let (layout1, plan) = (&doc.layouts()[0], &doc.layouts()[1]);
    assert_eq!((layout1.width, layout1.height), (297.0, 210.0));
    assert_eq!((plan.width, plan.height), (420.0, 297.0));

    // One window: the sheet-wide viewport is not one and the side view is skipped.
    assert_eq!(layout1.viewports.len(), 1);
    let vp = &layout1.viewports[0];
    // Paper coordinates start at the printable area, so everything on the
    // sheet moves by the left (8) and bottom (6) margins.
    assert_eq!(vp.center, Point::new(108.0, 86.0));
    assert_eq!((vp.width, vp.height), (50.0, 40.0));
    assert!((vp.scale - 0.5).abs() < 1e-12, "40 mm showing 80 units");
    assert_eq!(vp.view_center, Point::new(10.0, 5.0));
    assert!(vp.on && !vp.locked);
    let walls = doc.layers().iter().find(|l| l.name == "Walls").unwrap().id;
    assert_eq!(vp.frozen, [walls]);
    let on_sheet: Vec<_> = doc
        .objects_of_layout(layout1.id)
        .into_iter()
        .filter_map(|id| doc.get(id).cloned())
        .collect();
    assert_eq!(
        on_sheet,
        [Entity::Line {
            start: Point::new(18.0, 16.0),
            end: Point::new(68.0, 16.0)
        }]
    );

    let off = &plan.viewports[0];
    assert!(!off.on && off.locked);
    assert_eq!(off.center, Point::new(210.0, 148.5), "no margins, no shift");
    assert_eq!(doc.objects_of_layout(plan.id).len(), 1);

    assert_eq!(doc.model_objects().count(), 2);
    assert_eq!(doc.validate(), Ok(()));
    assert!(
        loaded
            .warnings
            .iter()
            .any(|w| w.starts_with("Skipped 1 viewport that show")),
        "{:?}",
        loaded.warnings
    );
}

#[test]
fn layers_keep_their_colour_linetype_weight_and_flags() {
    let loaded = sample();
    let layer = |name: &str| {
        loaded
            .document
            .layers()
            .iter()
            .find(|l| l.name == name)
            .unwrap_or_else(|| panic!("no layer {name}"))
            .clone()
    };
    let walls = layer("Walls");
    assert_eq!(walls.color, Color::Rgb(255, 0, 0));
    assert_eq!(walls.linetype, od_core::LinetypeId::DASHED);
    assert_eq!(walls.lineweight, Lineweight(50));
    assert!(layer("Frozen").frozen);
    assert!(layer("Locked").locked);
    assert!(!layer("Off").on);
    assert_eq!(layer("TrueColor").color, Color::Rgb(10, 200, 100));
    assert_eq!(layer("0").color, Color::Foreground);
    // A name outside ASCII survives, and a layer only entities mention exists.
    layer("Caf\u{e9}");
    layer("Undeclared");
}

#[test]
fn entities_arrive_on_their_layers_with_their_attributes() {
    let loaded = sample();
    let document = &loaded.document;
    let objects = document.objects();
    let on = |i: usize| document.layer(objects[i].attrs.layer).unwrap().name.clone();

    assert_eq!(
        objects[0].entity,
        Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(10.0, 0.0)
        }
    );
    assert_eq!(on(0), "Walls");
    assert_eq!(objects[1].attrs.color, Attr::Set(Color::Rgb(255, 255, 0)));
    assert_eq!(on(2), "Caf\u{e9}", "the arc");
    assert!(matches!(&objects[2].entity, Entity::Arc { .. }));
    // The rectangle is closed, on the locked layer.
    let Entity::Polyline { points, closed, .. } = &objects[4].entity else {
        panic!("the rectangle should be a polyline");
    };
    assert!(*closed && points.len() == 4);
    assert_eq!(on(4), "Locked");
    // The old-style polyline arrives with its three vertices.
    assert!(matches!(&objects[5].entity, Entity::Polyline { points, .. } if points.len() == 3));
    // Attributes set on an entity, and true colour.
    assert_eq!(
        objects[6].attrs.linetype,
        Attr::Set(od_core::LinetypeId::CENTER)
    );
    assert_eq!(objects[6].attrs.lineweight, Attr::Set(Lineweight(35)));
    assert_eq!(objects[6].attrs.color, Attr::Set(Color::Rgb(0, 0, 255)));
    assert_eq!(objects[7].attrs.color, Attr::Set(Color::Rgb(255, 128, 0)));
}

#[test]
fn what_we_write_from_it_reads_back_the_same() {
    let loaded = sample();
    let bytes = od_files::format_for(Path::new("x.dxf"))
        .unwrap()
        .write(&loaded.document)
        .unwrap()
        .bytes;
    let again = od_files::format_for(Path::new("x.dxf"))
        .unwrap()
        .read(&bytes)
        .unwrap();
    // Block references are written as the objects they draw.
    let entities = |d: &od_core::Document| {
        d.objects()
            .iter()
            .flat_map(|o| d.drawn(o))
            .map(|o| o.entity.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(entities(&again.document), entities(&loaded.document));
    assert!(
        again.warnings.is_empty(),
        "nothing is curved or unsupported now"
    );
}

/// Splines made by ezdxf (see `fixtures/make_ezdxf_splines.py`), checked
/// against the points ezdxf's own evaluator puts on them.
#[test]
fn splines_from_ezdxf_trace_the_same_curves() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ezdxf_splines.dxf");
    let loaded = od_files::load(&path).expect("the sample reads");
    assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
    let splines: Vec<od_core::Spline> = loaded
        .document
        .model_objects()
        .filter_map(|o| match &o.entity {
            Entity::Spline(s) => Some((**s).clone()),
            _ => None,
        })
        .collect();
    assert_eq!(splines.len(), 4);
    // What make_ezdxf_splines.py printed: each curve at 0, 1/4 ... 1 of its domain.
    let expected: [[(f64, f64); 5]; 4] = [
        [
            (0.0, 0.0),
            (2.6875, 2.21875),
            (5.0, 1.25),
            (7.3125, 1.65625),
            (10.0, 0.0),
        ],
        [
            (1.0, 0.0),
            (0.929788301062, 0.368094709562),
            (FRAC_1_SQRT_2, FRAC_1_SQRT_2),
            (0.368094709562, 0.929788301062),
            (0.0, 1.0),
        ],
        [
            (0.0, 10.0),
            (2.425754080618, 12.973047587362),
            (4.5, 11.0),
            (6.574245919382, 9.026952412638),
            (9.0, 12.0),
        ],
        [
            (23.333333333333, 0.666666666667),
            (23.333333333333, 3.333333333333),
            (20.666666666667, 3.333333333333),
            (20.666666666667, 0.666666666667),
            (23.333333333333, 0.666666666667),
        ],
    ];
    for (s, points) in splines.iter().zip(expected) {
        let (lo, hi) = s.domain();
        for (i, (x, y)) in points.iter().enumerate() {
            let at = s.point_at(lo + (hi - lo) * i as f64 / 4.0);
            assert!((at.x - x).hypot(at.y - y) < 1e-9, "{at:?} vs ({x}, {y})");
        }
    }
    assert!(splines[1].is_rational());
    assert!(splines[3].closed && !splines[3].periodic);
    // The block's spline comes through its reference, moved to (40, 0).
    let picture = loaded
        .document
        .model_objects()
        .find_map(|o| match &o.entity {
            Entity::Insert(i) => Some(i.picture()),
            _ => None,
        })
        .expect("the reference");
    let Some(Entity::Spline(wave)) = picture.first() else {
        panic!("a spline in the block, got {picture:?}")
    };
    assert_eq!(wave.start_point(), Point::new(40.0, 0.0));
    assert_eq!(wave.end_point(), Point::new(43.0, 0.0));
}

#[test]
fn dimensions_come_in_with_their_points_styles_and_text() {
    use od_core::{DimKind, DimText, Dimension};

    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ezdxf_dimensions.dxf");
    let loaded = od_files::load(&path).expect("the fixture reads");
    let dims: Vec<Dimension> = loaded
        .document
        .model_objects()
        .filter_map(|o| match &o.entity {
            Entity::Dimension(d) => Some((**d).clone()),
            _ => None,
        })
        .collect();
    assert_eq!(dims.len(), 10, "{:?}", loaded.warnings);
    let measured: Vec<f64> = dims.iter().map(|d| d.measurement().unwrap()).collect();
    let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
    // Horizontal, vertical, aligned, then radius and diameter.
    assert!(near(measured[0], 30.0) && near(measured[1], 20.0) && near(measured[2], 50.0));
    assert!(near(measured[3], 10.0) && near(measured[4], 20.0));
    // Two lines: 45 degrees; three points: 90 degrees; ordinate: the X.
    assert!(near(measured[5], 45.0_f64.to_radians()));
    assert!(near(measured[6], 90.0_f64.to_radians()));
    assert!(near(measured[7], 30.0));
    assert!(matches!(
        dims[0].kind,
        DimKind::Linear { rotation: Some(r), .. } if near(r, 0.0)
    ));
    assert!(matches!(dims[5].kind, DimKind::Angular { lines: true, .. }));
    assert!(matches!(
        dims[7].kind,
        DimKind::Ordinate { x_datum: true, .. }
    ));
    assert_eq!(dims[1].text, DimText::Custom("<> TYP".to_owned()));
    // Styles by name, and the table in the drawing with Standard first.
    assert_eq!(dims[8].style.name, "Big");
    assert_eq!(dims[8].style.text_height, 6.0);
    assert_eq!(dims[0].style.name, "EZDXF");
    let styles = loaded.document.dim_styles();
    assert_eq!(styles[0].name, "Standard");
    assert!(loaded.document.dim_style_named("Big").is_some());
    // The last has its style's name but a text height of its own.
    assert_eq!(dims[9].style.name, "EZDXF");
    assert_eq!(dims[9].style.text_height, 5.0);
    assert_eq!(loaded.document.validate(), Ok(()));
}
