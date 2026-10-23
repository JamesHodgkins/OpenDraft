//! The OBJECTS section of the R2000 writer: the dictionaries and the LAYOUT
//! object of each space.

use od_core::{paper_size_name, Document};

use super::{hex, Handles, Ids};
use crate::dxf::write::{dxf_name, Out};
use crate::FileError;

/// The OBJECTS section: the root dictionary, the layout dictionary and one
/// `LAYOUT` per space.
pub(super) fn write_objects(
    out: &mut Out,
    h: &mut Handles,
    ids: &Ids,
    document: &Document,
    leaders: bool,
) -> Result<(), FileError> {
    let layouts = document.layouts();
    let (root, groups, layout_dict, plot_settings, plot_styles) =
        (h.next(), h.next(), h.next(), h.next(), h.next());
    out.section("OBJECTS");
    let dictionary = |out: &mut Out, handle: u32, owner: u32, entries: &[(String, u32)]| {
        out.pair(0, "DICTIONARY");
        out.pair(5, hex(handle));
        out.pair(330, hex(owner));
        out.pair(100, "AcDbDictionary");
        out.pair(281, 1);
        for (name, target) in entries {
            out.pair(3, name);
            out.pair(350, hex(*target));
        }
    };
    let mut entries = vec![
        ("ACAD_GROUP".to_owned(), groups),
        ("ACAD_LAYOUT".to_owned(), layout_dict),
    ];
    if leaders {
        entries.push(("ACAD_MLEADERSTYLE".to_owned(), ids.mleader_dict));
    }
    entries.extend([
        ("ACAD_PLOTSETTINGS".to_owned(), plot_settings),
        ("ACAD_PLOTSTYLENAME".to_owned(), plot_styles),
    ]);
    dictionary(out, root, 0, &entries);
    dictionary(out, groups, root, &[]);
    let mut entries = vec![("Model".to_owned(), ids.model_layout)];
    entries.extend(
        layouts
            .iter()
            .enumerate()
            .map(|(i, l)| (dxf_name(&l.name), ids.layouts[i])),
    );
    dictionary(out, layout_dict, root, &entries);
    dictionary(out, plot_settings, root, &[]);
    out.pair(0, "ACDBDICTIONARYWDFLT");
    out.pair(5, hex(plot_styles));
    out.pair(330, hex(root));
    out.pair(100, "AcDbDictionary");
    out.pair(281, 1);
    out.pair(3, "Normal");
    out.pair(350, hex(ids.placeholder));
    out.pair(100, "AcDbDictionaryWithDefault");
    out.pair(340, hex(ids.placeholder));
    out.pair(0, "ACDBPLACEHOLDER");
    out.pair(5, hex(ids.placeholder));
    out.pair(330, hex(plot_styles));

    layout_object(
        out,
        ids.model_layout,
        layout_dict,
        "Model",
        0,
        (420.0, 297.0),
        ids.model_record,
        None,
    );
    for (i, layout) in layouts.iter().enumerate() {
        layout_object(
            out,
            ids.layouts[i],
            layout_dict,
            &dxf_name(&layout.name),
            i32::try_from(i + 1).unwrap_or(1),
            (layout.width, layout.height),
            ids.records[i],
            Some(ids.overall[i]),
        );
    }
    if leaders {
        super::mleader::styles(out, ids, document, root)?;
    }
    out.end_section();
    Ok(())
}

/// A `LAYOUT` object: plot settings (sheet size in millimetres) then layout data.
#[allow(clippy::too_many_arguments)]
fn layout_object(
    out: &mut Out,
    handle: u32,
    dictionary: u32,
    name: &str,
    tab: i32,
    (width, height): (f64, f64),
    record: u32,
    overall: Option<u32>,
) {
    out.pair(0, "LAYOUT");
    out.pair(5, hex(handle));
    out.pair(330, hex(dictionary));
    out.pair(100, "AcDbPlotSettings");
    out.pair(1, "");
    if overall.is_some() {
        out.pair(2, "none_device");
    }
    let paper = paper_size_name(width, height).map_or_else(
        || format!("OD_({width:.2}_x_{height:.2}_MM)"),
        str::to_owned,
    );
    out.pair(
        4,
        if overall.is_some() {
            paper
        } else {
            String::new()
        },
    );
    out.pair(6, "");
    for code in [40, 41, 42, 43] {
        out.pair(code, 0.0);
    }
    out.pair(44, width);
    out.pair(45, height);
    for code in [46, 47, 48, 49, 140, 141] {
        out.pair(code, 0.0);
    }
    out.pair(142, 1.0);
    out.pair(143, 1.0);
    out.pair(70, if overall.is_some() { 688 } else { 1024 });
    out.pair(72, 1);
    out.pair(73, 0);
    out.pair(74, 5);
    out.pair(7, "");
    out.pair(75, 16);
    out.pair(76, 0);
    out.pair(77, 2);
    out.pair(78, 300);
    out.pair(147, 1.0);
    out.pair(148, 0.0);
    out.pair(149, 0.0);
    out.pair(100, "AcDbLayout");
    out.pair(1, name);
    out.pair(70, 1);
    out.pair(71, tab);
    out.pair(10, 0.0);
    out.pair(20, 0.0);
    out.pair(11, width);
    out.pair(21, height);
    out.point3(12, 0.0, 0.0, 0.0);
    out.point3(14, 1e20, 1e20, 1e20);
    out.point3(15, -1e20, -1e20, -1e20);
    out.pair(146, 0.0);
    out.point3(13, 0.0, 0.0, 0.0);
    out.point3(16, 1.0, 0.0, 0.0);
    out.point3(17, 0.0, 1.0, 0.0);
    out.pair(76, 1);
    out.pair(330, hex(record));
    if let Some(overall) = overall {
        out.pair(331, hex(overall));
    }
}
