//! The Home tab's Modify panel.

use od_ui::icons;
use od_ui::ribbon::{Cell, Column, Panel, Split};

use super::{icon, item};

pub(super) fn modify() -> Panel {
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
            item("modify.pedit", "Edit Polyline", icons::MODIFY_PEDIT),
            item("modify.hatchedit", "Edit Hatch", icons::MODIFY_HATCHEDIT),
        ],
    }
}
