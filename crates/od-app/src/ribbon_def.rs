//! The application's ribbon layout. Items name command ids; what they do is
//! decided elsewhere (the command registry arrives in Phase 3).

use od_ui::icons;
use od_ui::ribbon::{Column, Item, Panel, Ribbon, Tab};

mod home;

pub fn default_ribbon() -> Ribbon {
    Ribbon {
        tabs: vec![home::home(), insert(), annotate(), view(), manage()],
    }
}

/// The title of the contextual tab shown while the Block Editor is open.
pub const BLOCK_EDITOR_TAB: &str = "Block Editor";

/// The ribbon while the Block Editor is open: the usual tabs and, last, a
/// contextual Block Editor tab, as in AutoCAD.
pub fn block_editor_ribbon() -> Ribbon {
    let mut ribbon = default_ribbon();
    ribbon.tabs.push(Tab {
        title: BLOCK_EDITOR_TAB,
        panels: vec![
            Panel {
                title: "Open/Save",
                columns: vec![Column::Large(Item::new(
                    "block.save",
                    "Save Block",
                    icons::BLOCK_SAVE,
                ))],
                ..Default::default()
            },
            Panel {
                title: "Close",
                columns: vec![Column::Large(Item::new(
                    "block.close",
                    "Close Block Editor",
                    icons::BLOCK_CLOSE,
                ))],
                ..Default::default()
            },
        ],
    });
    ribbon
}

fn insert() -> Tab {
    Tab {
        title: "Insert",
        panels: vec![
            Panel {
                title: "Block",
                columns: vec![
                    Column::Large(Item::new("block.insert", "Insert", icons::BLOCK_INSERT)),
                    Column::Large(Item::new("block.create", "Create", icons::BLOCK_CREATE)),
                    Column::Large(Item::new("block.edit", "Edit", icons::BLOCK_EDIT)),
                    Column::Large(Item::new("block.purge", "Purge", icons::BLOCK_PURGE)),
                ],
                ..Default::default()
            },
            Panel {
                title: "Attributes",
                columns: vec![
                    Column::Large(Item::new(
                        "block.attdef",
                        "Define Attributes",
                        icons::BLOCK_ATTDEF,
                    )),
                    Column::Large(Item::new(
                        "block.attedit",
                        "Edit Attributes",
                        icons::BLOCK_ATTEDIT,
                    )),
                    Column::Large(Item::new(
                        "block.attsync",
                        "Synchronize",
                        icons::BLOCK_ATTSYNC,
                    )),
                ],
                ..Default::default()
            },
            Panel {
                title: "Data",
                columns: vec![
                    Column::Large(Item::new("block.field", "Field", icons::INSERT_FIELD)),
                    Column::Large(Item::new(
                        "block.updatefield",
                        "Update Fields",
                        icons::INSERT_UPDATEFIELD,
                    )),
                ],
                ..Default::default()
            },
            Panel {
                title: "Import",
                columns: vec![
                    Column::Large(Item::new(
                        "insert.import",
                        "Import File",
                        icons::INSERT_IMPORT,
                    )),
                    Column::Large(Item::new(
                        "insert.export",
                        "Export File",
                        icons::INSERT_EXPORT,
                    )),
                ],
                ..Default::default()
            },
        ],
    }
}

fn annotate() -> Tab {
    Tab {
        title: "Annotate",
        panels: vec![
            Panel {
                title: "Annotation",
                columns: vec![
                    Column::Large(Item::new(
                        "annotate.mtext",
                        "Multiline Text",
                        icons::ANNOTATE_MTEXT,
                    )),
                    Column::Large(Item::new(
                        "annotate.mleader",
                        "Multileader",
                        icons::ANNOTATE_MLEADER,
                    )),
                    Column::Large(Item::new(
                        "annotate.leader",
                        "Leader",
                        icons::ANNOTATE_LEADER,
                    )),
                    Column::Large(Item::new(
                        "annotate.textstyle",
                        "Text Style",
                        icons::ANNOTATE_TEXTSTYLE,
                    )),
                    Column::Large(Item::new(
                        "annotate.mleaderstyle",
                        "Multileader Style",
                        icons::ANNOTATE_MLEADERSTYLE,
                    )),
                ],
                ..Default::default()
            },
            Panel {
                title: "Dimensions",
                columns: vec![
                    Column::Large(Item::new("dim.linear", "Linear", icons::DIM_LINEAR)),
                    Column::Large(Item::new("dim.aligned", "Aligned", icons::DIM_ALIGNED)),
                    Column::Large(Item::new("dim.angular", "Angular", icons::DIM_ANGULAR)),
                    Column::Stack(vec![
                        Item::new("dim.radius", "Radius", icons::DIM_RADIUS),
                        Item::new("dim.diameter", "Diameter", icons::DIM_DIAMETER),
                        Item::new("dim.arc", "Arc Length", icons::DIM_ARC),
                    ]),
                    Column::Stack(vec![
                        Item::new("dim.jogged", "Jogged", icons::DIM_JOGGED),
                        Item::new("dim.ordinate", "Ordinate", icons::DIM_ORDINATE),
                        Item::new("dim.centre", "Center Mark", icons::DIM_CENTRE),
                    ]),
                    Column::Stack(vec![
                        Item::new("dim.baseline", "Baseline", icons::DIM_BASELINE),
                        Item::new("dim.continue", "Continue", icons::DIM_CONTINUE),
                        Item::new("dim.space", "Space", icons::DIM_SPACE),
                    ]),
                    Column::Stack(vec![
                        Item::new("dim.edit", "Edit", icons::DIM_EDIT),
                        Item::new("dim.textedit", "Text Edit", icons::DIM_TEXTEDIT),
                        Item::new("dim.jogline", "Jog Line", icons::DIM_JOGLINE),
                    ]),
                    Column::Stack(vec![
                        Item::new("dim.reassociate", "Reassociate", icons::DIM_REASSOCIATE),
                        Item::new("dim.disassociate", "Disassociate", icons::DIM_DISASSOCIATE),
                    ]),
                    Column::Large(Item::new("dim.style", "Dimension Style", icons::DIM_STYLE)),
                ],
                ..Default::default()
            },
        ],
    }
}

fn view() -> Tab {
    Tab {
        title: "View",
        panels: vec![
            Panel {
                title: "Navigate",
                columns: vec![
                    Column::Large(Item::new("view.pan", "Pan", icons::VIEW_PAN)),
                    Column::Stack(vec![
                        Item::new("view.zoom_in", "Zoom In", icons::VIEW_ZOOM_IN),
                        Item::new("view.zoom_out", "Zoom Out", icons::VIEW_ZOOM_OUT),
                        Item::new(
                            "view.zoom_extents",
                            "Zoom Extents",
                            icons::VIEW_ZOOM_EXTENTS,
                        ),
                    ]),
                ],
                ..Default::default()
            },
            Panel {
                title: "Drafting Aids",
                columns: vec![Column::Stack(vec![
                    Item::new("view.grid", "Grid", icons::VIEW_GRID),
                    Item::new("view.snap", "Snap", icons::VIEW_SNAP),
                ])],
                ..Default::default()
            },
            Panel {
                title: "Palettes",
                columns: vec![
                    Column::Large(Item::new(
                        "view.panel_properties",
                        "Properties",
                        icons::VIEW_PANEL_PROPERTIES,
                    )),
                    Column::Large(Item::new(
                        "view.panel_layers",
                        "Layers",
                        icons::VIEW_PANEL_LAYERS,
                    )),
                    Column::Large(Item::new(
                        "view.command_line",
                        "Command Line",
                        icons::VIEW_COMMAND_LINE,
                    )),
                    Column::Large(Item::new(
                        "view.assistant",
                        "Assistant",
                        icons::VIEW_ASSISTANT,
                    )),
                    Column::Large(Item::new(
                        "view.reset_layout",
                        "Reset Layout",
                        icons::VIEW_RESET_LAYOUT,
                    )),
                ],
                ..Default::default()
            },
        ],
    }
}

fn manage() -> Tab {
    Tab {
        title: "Manage",
        panels: vec![
            Panel {
                title: "Drawing",
                columns: vec![
                    Column::Large(Item::new("format.units", "Units", icons::FORMAT_UNITS)),
                    Column::Large(Item::new(
                        "format.properties",
                        "Properties",
                        icons::FORMAT_PROPERTIES,
                    )),
                    Column::Large(Item::new(
                        "format.linetype",
                        "Linetypes",
                        icons::FORMAT_LINETYPE,
                    )),
                ],
                ..Default::default()
            },
            Panel {
                title: "Customization",
                columns: vec![Column::Large(Item::new(
                    "manage.settings",
                    "Settings",
                    icons::MANAGE_SETTINGS,
                ))],
                ..Default::default()
            },
        ],
    }
}
