//! LAYOUT, MVIEW and VPSCALE. The sheet maths is in `od_core::layout`; these
//! only ask and record one undo step (`SetLayouts`).

use od_commands::{
    AddCopies, Answer, Cancelled, Editor, Options, PointOptions, RemoveEntities, SetLayouts,
};
use od_core::{
    format_scale, parse_scale, Attributes, Entity, EntityId, LayerId, Layout, LayoutId, Point,
    Viewport, ViewportId, PAPER_SIZES,
};

use crate::layouts::with_viewport;

/// LAYOUT: Copy, Delete, New or Rename a layout, by name. Entering one is
/// done from its tab (or `MSPACE` once there); a command cannot change tabs.
pub fn layout_command(ed: &mut Editor, current: Option<LayoutId>) -> Result<(), Cancelled> {
    let option = ed.get_keyword(
        Options::<String>::new("Enter layout option")
            .keyword("Copy", "C")
            .keyword("Delete", "D")
            .keyword("New", "N")
            .keyword("Rename", "R")
            .default("New".to_owned()),
    )?;
    if option == "New" {
        let name = ed.get_string(Options::<String>::new("Enter new layout name"))?;
        return edit_layouts(ed, "New layout", move |doc| {
            let name = checked_name(doc, &name, None)?;
            let (_, width, height) = PAPER_SIZES[0];
            let mut layouts = doc.layouts().to_vec();
            layouts.push(doc.new_layout(name, width, height));
            Ok(Edit::layouts(layouts))
        });
    }
    let target = ed.get_string(named_layout(&option, current, ed)?)?;
    match option.as_str() {
        "Delete" => edit_layouts(ed, "Delete layout", move |doc| {
            let id = find(doc, &target)?;
            let mut layouts = doc.layouts().to_vec();
            layouts.retain(|l| l.id != id);
            Ok(Edit {
                remove: doc.objects_of_layout(id),
                ..Edit::layouts(layouts)
            })
        }),
        "Copy" => {
            let name = ed.get_string(Options::<String>::new("Enter name of the copy"))?;
            edit_layouts(ed, "Copy layout", move |doc| {
                let id = find(doc, &target)?;
                let name = checked_name(doc, &name, None)?;
                let mut layouts = doc.layouts().to_vec();
                let copy = doc.copied_layout(id, name).ok_or("The layout is gone.")?;
                let drawn = (copy.id, doc.copied_sheet_objects(id));
                layouts.push(copy);
                Ok(Edit {
                    add: Some(drawn),
                    ..Edit::layouts(layouts)
                })
            })
        }
        _ => {
            let name = ed.get_string(Options::<String>::new("Enter new layout name"))?;
            edit_layouts(ed, "Rename layout", move |doc| {
                let id = find(doc, &target)?;
                let name = checked_name(doc, &name, Some(id))?;
                let mut layouts = doc.layouts().to_vec();
                for layout in layouts.iter_mut().filter(|l| l.id == id) {
                    layout.name = name.clone();
                }
                Ok(Edit::layouts(layouts))
            })
        }
    }
}

/// The question "Enter name of layout to delete <Layout1>".
fn named_layout(
    option: &str,
    current: Option<LayoutId>,
    ed: &Editor,
) -> Result<Options<String>, Cancelled> {
    let default = ed.read(move |doc| {
        current
            .and_then(|id| doc.layout(id))
            .map(|l| l.name.clone())
    })?;
    let verb = match option {
        "Copy" => "copy",
        "Delete" => "delete",
        _ => "rename",
    };
    let options = Options::<String>::new(format!("Enter name of layout to {verb}"));
    Ok(match default {
        Some(name) => options.default(name),
        None => options,
    })
}

fn find(doc: &od_core::Document, name: &str) -> Result<LayoutId, String> {
    doc.layouts()
        .iter()
        .find(|l| l.name.eq_ignore_ascii_case(name.trim()))
        .map(|l| l.id)
        .ok_or_else(|| format!("There is no layout called \"{}\".", name.trim()))
}

fn checked_name(
    doc: &od_core::Document,
    name: &str,
    except: Option<LayoutId>,
) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        Err("A layout needs a name.".to_owned())
    } else if doc.layout_name_taken(name, except) {
        Err(format!("A layout called \"{name}\" already exists."))
    } else {
        Ok(name.to_owned())
    }
}

/// What a layout command changes: the list of layouts, the sheet objects to
/// remove (a deleted layout's), and sheet objects to add to a layout (a copy's).
struct Edit {
    layouts: Vec<Layout>,
    remove: Vec<EntityId>,
    add: Option<(LayoutId, Vec<(Entity, Attributes)>)>,
}

impl Edit {
    fn layouts(layouts: Vec<Layout>) -> Self {
        Self {
            layouts,
            remove: Vec::new(),
            add: None,
        }
    }
}

/// Run `build` on the drawing, then record what it asks for as one undo step;
/// a refusal is said and changes nothing.
fn edit_layouts(
    ed: &Editor,
    name: &'static str,
    build: impl FnOnce(&od_core::Document) -> Result<Edit, String> + Send + 'static,
) -> Result<(), Cancelled> {
    let refusal = ed.transaction(name, move |tx| match build(tx.document()) {
        Ok(edit) => {
            if !edit.remove.is_empty() {
                tx.perform(RemoveEntities::new(edit.remove));
            }
            tx.perform(SetLayouts::new(name, edit.layouts));
            if let Some((layout, drawn)) = edit.add.filter(|(_, d)| !d.is_empty()) {
                tx.perform(AddCopies::new(name, drawn).in_layout(Some(layout)));
            }
            Ok(None)
        }
        Err(why) => Ok(Some(why)),
    })?;
    if let Some(why) = refusal {
        ed.say(why);
    }
    Ok(())
}

/// VPLAYER: freeze or thaw layers in one viewport (the active or picked one)
/// or in all the layout's viewports; Reset thaws everything. Model space and
/// other viewports still show a layer frozen here.
pub fn vplayer(
    ed: &mut Editor,
    layout: LayoutId,
    current: Option<ViewportId>,
) -> Result<(), Cancelled> {
    let option = ed.get_keyword(
        Options::<String>::new("Enter an option")
            .keyword("Freeze", "F")
            .keyword("Thaw", "T")
            .keyword("Reset", "R")
            .default("Freeze".to_owned()),
    )?;
    let names = if option == "Reset" {
        String::new()
    } else {
        let verb = option.to_lowercase();
        ed.get_string(Options::<String>::new(format!(
            "Enter layer name(s) to {verb}, separated by commas, or * for all"
        )))?
    };
    let all = match current {
        Some(_) => {
            let scope = ed.get_keyword(
                Options::<String>::new("Viewports")
                    .keyword("Current", "C")
                    .keyword("All", "A")
                    .default("Current".to_owned()),
            )?;
            scope == "All"
        }
        None => true,
    };
    edit_layouts(ed, "VPLAYER", move |doc| {
        let layers = if option == "Reset" {
            Vec::new()
        } else {
            resolve_layers(doc, &names)?
        };
        let mut layouts = doc.layouts().to_vec();
        let sheet = layouts
            .iter_mut()
            .find(|l| l.id == layout)
            .ok_or("The layout is gone.")?;
        for vp in sheet
            .viewports
            .iter_mut()
            .filter(|v| all || Some(v.id) == current)
        {
            match option.as_str() {
                "Reset" => vp.frozen.clear(),
                "Freeze" => vp.set_frozen(&layers, true),
                _ => vp.set_frozen(&layers, false),
            }
        }
        Ok(Edit::layouts(layouts))
    })
}

/// Layers named in a comma-separated list (`*` is all of them), ignoring case.
fn resolve_layers(doc: &od_core::Document, names: &str) -> Result<Vec<LayerId>, String> {
    let mut found = Vec::new();
    for name in names.split(',').map(str::trim).filter(|n| !n.is_empty()) {
        if name == "*" {
            found.extend(doc.layers().iter().map(|l| l.id));
            continue;
        }
        let layer = doc
            .layers()
            .iter()
            .find(|l| l.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| format!("There is no layer called \"{name}\"."))?;
        found.push(layer.id);
    }
    if found.is_empty() {
        return Err("Name at least one layer.".to_owned());
    }
    Ok(found)
}

/// MVIEW: two corners make a viewport on the sheet showing the whole drawing.
pub fn mview(ed: &mut Editor, layout: LayoutId) -> Result<(), Cancelled> {
    let a = ed.get_point(PointOptions::new("Specify first corner of viewport"))?;
    let b = ed.get_corner(a, "Specify opposite corner")?;
    let (width, height) = ((b.x - a.x).abs(), (b.y - a.y).abs());
    if width < 1e-6 || height < 1e-6 {
        ed.say("The viewport needs some size.");
        return Ok(());
    }
    let center = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
    edit_layouts(ed, "MVIEW", move |doc| {
        let (_, next) = doc.next_layout_ids();
        let mut layouts = doc.layouts().to_vec();
        let sheet = layouts
            .iter_mut()
            .find(|l| l.id == layout)
            .ok_or("The layout is gone.")?;
        let viewport =
            Viewport::framing(ViewportId(next), center, width, height, doc.model_bounds());
        sheet.viewports.push(viewport);
        Ok(Edit::layouts(layouts))
    })
}

/// VPSCALE: set a viewport's scale from a ratio such as 1:50, keeping the
/// model point at its middle.
pub fn vpscale(ed: &mut Editor, layout: LayoutId, viewport: ViewportId) -> Result<(), Cancelled> {
    let state = ed.read(move |doc| {
        let vp = doc.layout(layout)?.viewport(viewport)?;
        Some((vp.scale, vp.locked))
    })?;
    let Some((scale, locked)) = state else {
        return Ok(());
    };
    if locked {
        ed.say("The viewport is locked; unlock it with VPLOCK.");
        return Ok(());
    }
    let text = ed.ask_string(
        Options::<String>::new("Enter scale as paper:model, for example 1:50")
            .default(format_scale(scale)),
    )?;
    let text = match text {
        Answer::Value(text) => text,
        Answer::Enter => format_scale(scale),
        Answer::Keyword(_) => return Err(Cancelled),
    };
    let Some(new_scale) = parse_scale(&text) else {
        ed.say(format!("\"{}\" is not a scale.", text.trim()));
        return Ok(());
    };
    edit_layouts(ed, "VPSCALE", move |doc| {
        with_viewport(doc.layouts(), layout, viewport, |v| v.scale = new_scale)
            .map(Edit::layouts)
            .ok_or_else(|| "The viewport is gone.".to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use od_canvas::Space;

    fn new_layout_state() -> (AppState, LayoutId) {
        let mut state = AppState::new();
        state.add_entity(Entity::Line {
            start: Point::new(0.0, 0.0),
            end: Point::new(1000.0, 500.0),
        });
        state.new_layout();
        let Space::Paper(id) = state.space() else {
            panic!("new layout is entered");
        };
        (state, id)
    }

    fn layouts(state: &AppState) -> &[Layout] {
        state.session().document().layouts()
    }

    #[test]
    fn mview_adds_a_viewport_in_one_undo_step() {
        let (mut state, id) = new_layout_state();
        state.start_command("MVIEW", move |ed| mview(ed, id));
        state.settle_command();
        state.answer_point(Point::new(20.0, 20.0));
        state.answer_point(Point::new(100.0, 80.0));
        state.settle_command();
        let sheet = &layouts(&state)[0];
        assert_eq!(sheet.viewports.len(), 2);
        let made = &sheet.viewports[1];
        assert_eq!(
            (made.center, made.width, made.height),
            (Point::new(60.0, 50.0), 80.0, 60.0)
        );
        state.undo();
        assert_eq!(layouts(&state)[0].viewports.len(), 1);
    }

    #[test]
    fn vpscale_sets_the_scale_and_keeps_the_view_centre() {
        let (mut state, id) = new_layout_state();
        state.mspace();
        let (layout, viewport) = state.target_viewport().unwrap();
        let before = layouts(&state)[0].viewports[0].clone();
        state.start_command("VPSCALE", move |ed| vpscale(ed, layout, viewport));
        state.settle_command();
        assert!(state.answer_typed("1:100"));
        state.settle_command();
        let after = &layouts(&state)[0].viewports[0];
        assert!((after.scale - 0.01).abs() < 1e-12);
        assert_eq!(after.view_center, before.view_center);
        assert_eq!(layouts(&state)[0].id, id);
    }

    #[test]
    fn vpscale_refuses_a_locked_viewport_and_a_bad_scale() {
        let (mut state, _) = new_layout_state();
        state.mspace();
        let (layout, viewport) = state.target_viewport().unwrap();
        let scale = layouts(&state)[0].viewports[0].scale;
        state.start_command("VPSCALE", move |ed| vpscale(ed, layout, viewport));
        state.settle_command();
        assert!(state.answer_typed("fast"));
        state.settle_command();
        assert_eq!(layouts(&state)[0].viewports[0].scale, scale);
        state.toggle_viewport_lock();
        state.start_command("VPSCALE", move |ed| vpscale(ed, layout, viewport));
        state.settle_command();
        assert!(!state.command_running(), "it ended without asking");
    }

    /// A drawing with a line on layer "Walls", and a layout with two viewports.
    fn two_viewport_state() -> (AppState, LayoutId) {
        let (mut state, id) = new_layout_state();
        state.enter_model();
        let walls = state.new_layer();
        state.rename_layer(walls, "Walls");
        state.set_current_layer(walls);
        state.add_entity(Entity::Line {
            start: Point::new(1.0, 1.0),
            end: Point::new(2.0, 2.0),
        });
        state.enter_layout(id);
        let mut sheet = layouts(&state)[0].clone();
        let mut second = sheet.viewports[0].clone();
        second.id = ViewportId(second.id.0 + 100);
        sheet.viewports.push(second);
        state.perform(SetLayouts::new("test", vec![sheet]));
        (state, id)
    }

    #[test]
    fn vplayer_freezes_a_layer_in_one_viewport_only_and_undoes() {
        let (mut state, id) = two_viewport_state();
        state.mspace();
        let (_, current) = state.target_viewport().unwrap();
        state.start_command("VPLAYER", move |ed| vplayer(ed, id, Some(current)));
        state.settle_command();
        state.answer_keyword("Freeze");
        state.settle_command();
        assert!(state.answer_typed("walls"));
        state.settle_command();
        state.answer_keyword("Current");
        state.settle_command();
        let sheet = &layouts(&state)[0];
        let walls = state.session().document().layers()[1].id;
        let frozen: Vec<_> = sheet.viewports.iter().map(|v| v.is_frozen(walls)).collect();
        assert_eq!(frozen.iter().filter(|f| **f).count(), 1, "{frozen:?}");
        assert!(sheet.viewport(current).unwrap().is_frozen(walls));

        state.docs[state.active].keep_space_valid();
        let document = &state.session().document();
        // The layer 0 line stays; the Walls line is hidden in this viewport.
        assert_eq!(document.visible_objects().count(), 1, "Walls hidden here");
        assert_eq!(document.selectable_objects().count(), 1);
        assert_eq!(document.visible_in(None).count(), 2, "model space has both");
        state.undo();
        state.docs[state.active].keep_space_valid();
        assert_eq!(state.session().document().visible_objects().count(), 2);
    }

    #[test]
    fn vplayer_all_thaw_reset_and_unknown_layers() {
        let (mut state, id) = two_viewport_state();
        let run = |state: &mut AppState, answers: &[(&str, &str)]| {
            state.start_command("VPLAYER", move |ed| vplayer(ed, id, None));
            state.settle_command();
            for (kind, answer) in answers {
                if *kind == "k" {
                    state.answer_keyword(answer);
                } else {
                    assert!(state.answer_typed(answer));
                }
                state.settle_command();
            }
        };
        run(&mut state, &[("k", "Freeze"), ("t", "Walls, 0")]);
        let counts = |s: &AppState| -> Vec<usize> {
            layouts(s)[0]
                .viewports
                .iter()
                .map(|v| v.frozen.len())
                .collect()
        };
        assert_eq!(counts(&state), [2, 2], "no viewport picked: all of them");
        run(&mut state, &[("k", "Thaw"), ("t", "walls")]);
        assert_eq!(counts(&state), [1, 1]);
        run(&mut state, &[("k", "Freeze"), ("t", "Nope")]);
        assert_eq!(counts(&state), [1, 1], "an unknown layer changes nothing");
        run(&mut state, &[("k", "Reset")]);
        assert_eq!(counts(&state), [0, 0]);
    }

    #[test]
    fn layout_new_rename_copy_and_delete_by_name() {
        let (mut state, id) = new_layout_state();
        let run = |state: &mut AppState, option: &str, answers: &[&str]| {
            state.start_command("LAYOUT", move |ed| layout_command(ed, Some(id)));
            state.settle_command();
            state.answer_keyword(option);
            for answer in answers {
                state.settle_command();
                assert!(state.answer_typed(answer));
            }
            state.settle_command();
        };
        run(&mut state, "New", &["Plan"]);
        run(&mut state, "Rename", &["Plan", "Elevation"]);
        run(&mut state, "Copy", &["elevation", "Section"]);
        let names = |s: &AppState| {
            layouts(s)
                .iter()
                .map(|l| l.name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&state), ["Layout1", "Elevation", "Section"]);
        run(&mut state, "Rename", &["Section", "layout1"]);
        assert_eq!(
            names(&state),
            ["Layout1", "Elevation", "Section"],
            "name taken"
        );
        run(&mut state, "Delete", &["Elevation"]);
        assert_eq!(names(&state), ["Layout1", "Section"]);
        assert_eq!(state.session().document().validate(), Ok(()));
    }
}
