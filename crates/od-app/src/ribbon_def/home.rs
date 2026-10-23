//! The Home tab, laid out as AutoCAD 2020's: Draw, Modify, Annotation, Block,
//! Groups, Utilities, Layers, Properties, Clipboard and View. Items whose
//! command is not built yet name the command they will run, and show dimmed.

use od_ui::icons;
use od_ui::ribbon::{Cell, Column, Item, Panel, Split, Tab};

use crate::ribbon_slots::{COLOR_LIST, LAYER_LIST, LINETYPE_LIST, LINEWEIGHT_LIST};

/// Width of the layer list.
const LAYER_LIST_W: f32 = 210.0;
/// Width of the colour, lineweight and linetype lists, with their icons.
const PROPERTY_LIST_W: f32 = 180.0;

const fn item(command: &'static str, label: &'static str, icon: char) -> Item {
    Item::new(command, label, icon)
}

fn button(command: &'static str, label: &'static str, icon: char) -> Cell {
    Cell::Button(item(command, label, icon))
}

fn icon(command: &'static str, label: &'static str, icon: char) -> Cell {
    Cell::Icon(item(command, label, icon))
}

pub(super) fn home() -> Tab {
    Tab {
        title: "Home",
        panels: vec![
            draw(),
            modify(),
            annotation(),
            block(),
            groups(),
            utilities(),
            layers(),
            properties(),
            clipboard(),
            view(),
        ],
    }
}

fn draw() -> Panel {
    Panel {
        title: "Draw",
        columns: vec![
            Column::Large(item("draw.line", "Line", icons::DRAW_LINE)),
            Column::Large(item("draw.polyline", "Polyline", icons::DRAW_POLYLINE)),
            Column::LargeSplit(Split::labelled(
                "Circle",
                vec![
                    item("draw.circle", "Center, Radius", icons::DRAW_CIRCLE_RADIUS),
                    item(
                        "draw.circle_diameter",
                        "Center, Diameter",
                        icons::DRAW_CIRCLE_DIAMETER,
                    ),
                    item("draw.circle_2p", "2-Point", icons::DRAW_CIRCLE_2P),
                    item("draw.circle_3p", "3-Point", icons::DRAW_CIRCLE_3P),
                    item(
                        "draw.circle_ttr",
                        "Tan, Tan, Radius",
                        icons::DRAW_CIRCLE_TTR,
                    ),
                    item("draw.circle_3t", "Tan, Tan, Tan", icons::DRAW_CIRCLE_3T),
                ],
            )),
            Column::LargeSplit(Split::labelled(
                "Arc",
                vec![
                    item("draw.arc", "3-Point", icons::DRAW_ARC_3P),
                    item("draw.arc_sce", "Start, Center, End", icons::DRAW_ARC_SCE),
                ],
            )),
            Column::Rows(vec![
                vec![Cell::IconSplit(Split::new(vec![
                    item("draw.rectangle", "Rectangle", icons::DRAW_RECTANGLE),
                    item("draw.polygon", "Polygon", icons::DRAW_POLYGON),
                ]))],
                vec![icon("draw.ellipse", "Ellipse", icons::DRAW_ELLIPSE)],
                vec![Cell::IconSplit(Split::new(vec![
                    item("draw.hatch", "Hatch", icons::DRAW_HATCH),
                    item("draw.gradient", "Gradient", icons::DRAW_GRADIENT),
                    item("draw.boundary", "Boundary", icons::DRAW_BOUNDARY),
                ]))],
            ]),
        ],
        extra: vec![
            item("draw.spline", "Spline", icons::DRAW_SPLINE),
            item("draw.divide", "Divide", icons::DRAW_DIVIDE),
            item("draw.measure", "Measure", icons::DRAW_MEASURE),
            item("draw.donut", "Donut", icons::DRAW_DONUT),
            item("draw.xline", "Construction Line", icons::DRAW_XLINE),
            item("draw.ray", "Ray", icons::DRAW_RAY),
            item("draw.point_multiple", "Multiple Point", icons::DRAW_POINT),
            item("draw.point", "Single Point", icons::DRAW_POINT),
            item("format.point_style", "Point Style...", icons::DRAW_POINT),
        ],
    }
}

fn modify() -> Panel {
    Panel {
        title: "Modify",
        columns: vec![
            Column::Stack(vec![
                item("modify.move", "Move", icons::MODIFY_MOVE),
                item("modify.copy", "Copy", icons::MODIFY_COPY),
                item("modify.stretch", "Stretch", icons::MODIFY_STRETCH),
            ]),
            Column::Stack(vec![
                item("modify.rotate", "Rotate", icons::MODIFY_ROTATE),
                item("modify.mirror", "Mirror", icons::MODIFY_MIRROR),
                item("modify.scale", "Scale", icons::MODIFY_SCALE),
            ]),
            Column::Rows(vec![
                vec![Cell::Split(Split::new(vec![
                    item("modify.trim", "Trim", icons::MODIFY_TRIM),
                    item("modify.extend", "Extend", icons::MODIFY_EXTEND),
                ]))],
                vec![Cell::Split(Split::new(vec![
                    item("modify.fillet", "Fillet", icons::MODIFY_FILLET),
                    item("modify.chamfer", "Chamfer", icons::MODIFY_CHAMFER),
                    item("modify.blend", "Blend Curves", icons::MODIFY_BLEND),
                ]))],
                vec![Cell::Split(Split::labelled(
                    "Array",
                    vec![
                        item(
                            "modify.array_rect",
                            "Rectangular Array",
                            icons::MODIFY_ARRAY_RECT,
                        ),
                        item("modify.array_path", "Path Array", icons::MODIFY_ARRAY_PATH),
                        item(
                            "modify.array_polar",
                            "Polar Array",
                            icons::MODIFY_ARRAY_POLAR,
                        ),
                    ],
                ))],
            ]),
            Column::Rows(vec![
                vec![icon("modify.erase", "Erase", icons::MODIFY_ERASE)],
                vec![icon("modify.explode", "Explode", icons::MODIFY_EXPLODE)],
                vec![icon("modify.offset", "Offset", icons::MODIFY_OFFSET)],
            ]),
        ],
        extra: vec![
            item("modify.break", "Break", icons::MODIFY_BREAK),
            item("modify.join", "Join", icons::MODIFY_JOIN),
            item("modify.lengthen", "Lengthen", icons::MODIFY_LENGTHEN),
            item("modify.hatchedit", "Edit Hatch", icons::MODIFY_HATCHEDIT),
        ],
    }
}

fn annotation() -> Panel {
    Panel {
        title: "Annotation",
        columns: vec![
            Column::LargeSplit(Split::labelled(
                "Text",
                vec![
                    item("annotate.mtext", "Multiline Text", icons::ANNOTATE_MTEXT),
                    item("annotate.text", "Single Line", icons::ANNOTATE_TEXT),
                ],
            )),
            Column::Large(item("dim.dim", "Dimension", icons::DIM_DIM)),
            Column::Rows(vec![
                vec![Cell::Split(Split::new(vec![
                    item("dim.linear", "Linear", icons::DIM_LINEAR),
                    item("dim.aligned", "Aligned", icons::DIM_ALIGNED),
                    item("dim.angular", "Angular", icons::DIM_ANGULAR),
                    item("dim.arc", "Arc Length", icons::DIM_ARC),
                    item("dim.radius", "Radius", icons::DIM_RADIUS),
                    item("dim.diameter", "Diameter", icons::DIM_DIAMETER),
                ]))],
                vec![Cell::Split(Split::labelled(
                    "Leader",
                    vec![
                        item("annotate.mleader", "Multileader", icons::ANNOTATE_MLEADER),
                        item(
                            "annotate.mleader_add",
                            "Add Leader",
                            icons::ANNOTATE_MLEADER_ADD,
                        ),
                        item(
                            "annotate.mleader_remove",
                            "Remove Leader",
                            icons::ANNOTATE_MLEADER_REMOVE,
                        ),
                        item(
                            "annotate.mleader_align",
                            "Align",
                            icons::ANNOTATE_MLEADER_ALIGN,
                        ),
                        item(
                            "annotate.mleader_collect",
                            "Collect",
                            icons::ANNOTATE_MLEADER_COLLECT,
                        ),
                    ],
                ))],
                vec![button("annotate.table", "Table", icons::ANNOTATE_TABLE)],
            ]),
        ],
        extra: vec![
            item(
                "annotate.textstyle",
                "Text Style",
                icons::ANNOTATE_TEXTSTYLE,
            ),
            item("dim.style", "Dimension Style", icons::DIM_STYLE),
            item(
                "annotate.mleaderstyle",
                "Multileader Style",
                icons::ANNOTATE_MLEADERSTYLE,
            ),
            item(
                "annotate.tablestyle",
                "Table Style",
                icons::ANNOTATE_TABLESTYLE,
            ),
        ],
    }
}

fn block() -> Panel {
    Panel {
        title: "Block",
        columns: vec![
            Column::Large(item("block.insert", "Insert", icons::BLOCK_INSERT)),
            Column::Rows(vec![
                vec![button("block.create", "Create", icons::BLOCK_CREATE)],
                vec![button("block.edit", "Edit", icons::BLOCK_EDIT)],
                vec![Cell::Split(Split::labelled(
                    "Edit Attributes",
                    vec![
                        item("block.attedit", "Single", icons::BLOCK_ATTEDIT),
                        item("block.eattedit", "Multiple", icons::BLOCK_EATTEDIT),
                    ],
                ))],
            ]),
        ],
        ..Default::default()
    }
}

fn groups() -> Panel {
    Panel {
        title: "Groups",
        columns: vec![
            Column::Large(item("group.group", "Group", icons::GROUP_GROUP)),
            Column::Rows(vec![
                vec![icon("group.ungroup", "Ungroup", icons::GROUP_UNGROUP)],
                vec![icon("group.edit", "Group Edit", icons::GROUP_EDIT)],
                vec![icon(
                    "group.selection",
                    "Group Selection On/Off",
                    icons::GROUP_SELECTION,
                )],
            ]),
        ],
        extra: vec![
            item("group.manager", "Group Manager", icons::GROUP_MANAGER),
            item(
                "group.bounding_box",
                "Group Bounding Box",
                icons::GROUP_BOUNDING_BOX,
            ),
        ],
    }
}

/// AutoCAD keeps the select tool and its polygon and fence modes off the
/// ribbon; OpenDraft keeps them in reach under Utilities.
fn utilities() -> Panel {
    Panel {
        title: "Utilities",
        columns: vec![
            Column::LargeSplit(Split::labelled(
                "Measure",
                vec![
                    item("inquiry.distance", "Distance", icons::INQUIRY_DISTANCE),
                    item("inquiry.radius", "Radius", icons::INQUIRY_RADIUS),
                    item("inquiry.angle", "Angle", icons::INQUIRY_ANGLE),
                    item("inquiry.area", "Area", icons::INQUIRY_AREA),
                    item("inquiry.volume", "Volume", icons::INQUIRY_VOLUME),
                ],
            )),
            Column::Rows(vec![
                vec![icon("select.power", "Quick Select", icons::SELECT_POWER)],
                vec![icon(
                    "edit.select_all",
                    "Select All",
                    icons::EDIT_SELECT_ALL,
                )],
                vec![icon(
                    "tools.quickcalc",
                    "Calculator",
                    icons::TOOLS_QUICKCALC,
                )],
            ]),
        ],
        extra: vec![
            item("inquiry.id", "ID Point", icons::INQUIRY_ID),
            item("tool.select", "Select", icons::TOOL_SELECT),
            item(
                "select.window_polygon",
                "Window Polygon",
                icons::SELECT_WINDOW_POLYGON,
            ),
            item(
                "select.crossing_polygon",
                "Crossing Polygon",
                icons::SELECT_CROSSING_POLYGON,
            ),
            item("select.fence", "Fence", icons::SELECT_FENCE),
        ],
    }
}

fn layers() -> Panel {
    Panel {
        title: "Layers",
        columns: vec![
            Column::Large(item(
                "layer.properties",
                "Layer Properties",
                icons::LAYER_PROPERTIES,
            )),
            Column::Rows(vec![
                vec![Cell::Slot {
                    id: LAYER_LIST,
                    width: LAYER_LIST_W,
                }],
                vec![
                    icon("layer.hide", "Off", icons::LAYER_HIDE),
                    icon("layer.isolate", "Isolate", icons::LAYER_ISOLATE),
                    icon("layer.freeze", "Freeze", icons::LAYER_FREEZE),
                    icon("layer.lock", "Lock", icons::LAYER_LOCK),
                    button(
                        "layer.make_current",
                        "Make Current",
                        icons::LAYER_MAKE_CURRENT,
                    ),
                ],
                vec![
                    icon("layer.show", "Turn All Layers On", icons::LAYER_SHOW),
                    icon("layer.unisolate", "Unisolate", icons::LAYER_UNISOLATE),
                    icon("layer.thaw_all", "Thaw All Layers", icons::LAYER_THAW_ALL),
                    icon("layer.unlock", "Unlock", icons::LAYER_UNLOCK),
                    button("layer.match", "Match Layer", icons::LAYER_MATCH),
                ],
            ]),
        ],
        extra: vec![item("layer.states", "Layer States", icons::LAYER_STATES)],
    }
}

fn properties() -> Panel {
    let list = |id| {
        vec![Cell::Slot {
            id,
            width: PROPERTY_LIST_W,
        }]
    };
    Panel {
        title: "Properties",
        columns: vec![
            Column::Large(item(
                "modify.matchprop",
                "Match Properties",
                icons::MODIFY_MATCHPROP,
            )),
            Column::Rows(vec![
                list(COLOR_LIST),
                list(LINEWEIGHT_LIST),
                list(LINETYPE_LIST),
            ]),
        ],
        ..Default::default()
    }
}

fn clipboard() -> Panel {
    Panel {
        title: "Clipboard",
        columns: vec![
            Column::LargeSplit(Split::labelled(
                "Paste",
                vec![
                    item("edit.paste", "Paste", icons::EDIT_PASTE),
                    item(
                        "edit.paste_block",
                        "Paste as Block",
                        icons::EDIT_PASTE_BLOCK,
                    ),
                    item(
                        "edit.paste_original",
                        "Paste to Original Coordinates",
                        icons::EDIT_PASTE_ORIGINAL,
                    ),
                ],
            )),
            Column::Rows(vec![
                vec![icon("edit.cut", "Cut", icons::EDIT_CUT)],
                vec![Cell::IconSplit(Split::new(vec![
                    item("edit.copy_clip", "Copy Clip", icons::EDIT_COPY_CLIP),
                    item(
                        "edit.copy_base",
                        "Copy with Base Point",
                        icons::EDIT_COPY_BASE,
                    ),
                ]))],
            ]),
        ],
        ..Default::default()
    }
}

fn view() -> Panel {
    Panel {
        title: "View",
        columns: vec![Column::LargeSplit(Split::labelled(
            "Base",
            vec![
                item(
                    "view.base_model",
                    "From Model Space",
                    icons::VIEW_BASE_MODEL,
                ),
                item(
                    "view.base_inventor",
                    "From Inventor",
                    icons::VIEW_BASE_INVENTOR,
                ),
            ],
        ))],
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::build_registry;

    /// AutoCAD's Home items whose commands are not built yet; they show
    /// dimmed. Take an id off when its command lands.
    const NOT_BUILT: &[&str] = &[
        "draw.circle_3t",
        "draw.gradient",
        "modify.blend",
        "annotate.text",
        "dim.angular",
        "annotate.mleader_add",
        "annotate.mleader_remove",
        "annotate.mleader_align",
        "annotate.mleader_collect",
        "annotate.table",
        "annotate.tablestyle",
        "block.attedit",
        "block.eattedit",
        "group.group",
        "group.ungroup",
        "group.edit",
        "group.selection",
        "group.manager",
        "group.bounding_box",
        "inquiry.distance",
        "inquiry.radius",
        "inquiry.angle",
        "inquiry.area",
        "inquiry.volume",
        "inquiry.id",
        "tools.quickcalc",
        "layer.isolate",
        "layer.unisolate",
        "layer.states",
        "view.base_model",
        "view.base_inventor",
    ];

    fn commands(tab: &Tab) -> Vec<&'static str> {
        let mut ids = Vec::new();
        for panel in &tab.panels {
            for column in &panel.columns {
                match column {
                    Column::Large(item) => ids.push(item.command),
                    Column::LargeSplit(split) => ids.extend(split.items.iter().map(|i| i.command)),
                    Column::Stack(items) => ids.extend(items.iter().map(|i| i.command)),
                    Column::Rows(rows) => {
                        for cell in rows.iter().flatten() {
                            match cell {
                                Cell::Button(item) | Cell::Icon(item) => ids.push(item.command),
                                Cell::Split(split) | Cell::IconSplit(split) => {
                                    ids.extend(split.items.iter().map(|i| i.command))
                                }
                                Cell::Slot { .. } => {}
                            }
                        }
                    }
                }
            }
            ids.extend(panel.extra.iter().map(|i| i.command));
        }
        ids
    }

    #[test]
    fn every_home_item_runs_a_command_or_waits_for_one() {
        let registry = build_registry();
        let ids = commands(&home());
        for id in &ids {
            let built = registry.get(id).is_some();
            let waiting = NOT_BUILT.contains(id);
            assert!(
                built != waiting,
                "{id}: registered {built}, listed {waiting}"
            );
        }
        for id in NOT_BUILT {
            assert!(ids.contains(id), "{id} is not on the Home tab");
        }
    }

    #[test]
    fn the_home_tab_has_autocads_panels() {
        let titles: Vec<&str> = home().panels.iter().map(|p| p.title).collect();
        assert_eq!(
            titles,
            [
                "Draw",
                "Modify",
                "Annotation",
                "Block",
                "Groups",
                "Utilities",
                "Layers",
                "Properties",
                "Clipboard",
                "View"
            ]
        );
    }
}
