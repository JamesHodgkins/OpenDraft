//! The OBJECTS section of the R2000 writer: the dictionaries and the LAYOUT
//! object of each space.

use opendraft_core::{
    paper_size_name, Document, PaperUnits, PlotArea, PlotRotation, PlotScale, PlotSettings,
    ShadePlotMode, ShadeResolution, StandardScale,
};

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
    let layouts = super::sheets(document);
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

    let mut model_plot = document.model_plot().clone();
    model_plot.flags |= opendraft_core::plot_settings::flags::MODEL_TYPE;
    layout_object(
        out,
        ids.model_layout,
        layout_dict,
        "Model",
        0,
        &model_plot,
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
            &layout.plot,
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
    plot: &PlotSettings,
    record: u32,
    overall: Option<u32>,
) {
    out.pair(0, "LAYOUT");
    out.pair(5, hex(handle));
    out.pair(330, hex(dictionary));
    out.pair(100, "AcDbPlotSettings");
    out.pair(1, "");
    out.pair(2, &plot.device_name);
    let paper = if plot.canonical_media_name.is_empty() {
        paper_size_name(plot.paper_width_mm, plot.paper_height_mm).map_or_else(
            || {
                format!(
                    "OD_({:.2}_x_{:.2}_MM)",
                    plot.paper_width_mm, plot.paper_height_mm
                )
            },
            str::to_owned,
        )
    } else {
        plot.canonical_media_name.clone()
    };
    out.pair(4, paper);
    let view = match &plot.area {
        PlotArea::View(name) => name.as_str(),
        _ => "",
    };
    out.pair(6, view);
    for (code, value) in [40, 41, 42, 43].into_iter().zip(plot.margins_mm) {
        out.pair(code, value);
    }
    out.pair(44, plot.paper_width_mm);
    out.pair(45, plot.paper_height_mm);
    out.pair(46, plot.origin_mm.x);
    out.pair(47, plot.origin_mm.y);
    let (first, second) = match plot.area {
        PlotArea::Window { first, second } => (first, second),
        _ => (
            opendraft_core::Point::new(0.0, 0.0),
            opendraft_core::Point::new(0.0, 0.0),
        ),
    };
    out.pair(48, first.x);
    out.pair(49, second.x);
    out.pair(140, first.y);
    out.pair(141, second.y);
    let (numerator, denominator, standard, standard_value) = match plot.scale {
        PlotScale::Standard { kind, value } => (
            value,
            1.0,
            match kind {
                StandardScale::Fit => 0,
                StandardScale::Ratio(code) => i32::from(code),
            },
            value,
        ),
        PlotScale::Custom {
            paper_units,
            drawing_units,
        } => (paper_units, drawing_units, 0, paper_units / drawing_units),
    };
    out.pair(142, numerator);
    out.pair(143, denominator);
    out.pair(70, i32::from(plot.flags));
    out.pair(
        72,
        match plot.paper_units {
            PaperUnits::Inches => 0,
            PaperUnits::Millimeters => 1,
            PaperUnits::Pixels => 2,
        },
    );
    out.pair(
        73,
        match plot.rotation {
            PlotRotation::Degrees0 => 0,
            PlotRotation::CounterClockwise90 => 1,
            PlotRotation::Degrees180 => 2,
            PlotRotation::Clockwise90 => 3,
        },
    );
    out.pair(74, plot_area_code(&plot.area));
    out.pair(7, &plot.style_sheet);
    out.pair(75, standard);
    out.pair(
        76,
        match plot.shade.mode {
            ShadePlotMode::AsDisplayed => 0,
            ShadePlotMode::Wireframe => 1,
            ShadePlotMode::Hidden => 2,
            ShadePlotMode::Rendered => 3,
        },
    );
    out.pair(
        77,
        match plot.shade.resolution {
            ShadeResolution::Draft => 0,
            ShadeResolution::Preview => 1,
            ShadeResolution::Normal => 2,
            ShadeResolution::Presentation => 3,
            ShadeResolution::Maximum => 4,
            ShadeResolution::Custom => 5,
        },
    );
    out.pair(78, i32::from(plot.shade.custom_dpi));
    out.pair(147, standard_value);
    out.pair(148, plot.paper_image_origin_mm.x);
    out.pair(149, plot.paper_image_origin_mm.y);
    out.pair(100, "AcDbLayout");
    out.pair(1, name);
    out.pair(70, 1);
    out.pair(71, tab);
    out.pair(10, 0.0);
    out.pair(20, 0.0);
    out.pair(11, plot.paper_width_mm);
    out.pair(21, plot.paper_height_mm);
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

fn plot_area_code(area: &PlotArea) -> i32 {
    match area {
        PlotArea::Display => 0,
        PlotArea::Extents => 1,
        PlotArea::Limits => 2,
        PlotArea::View(_) => 3,
        PlotArea::Window { .. } => 4,
        PlotArea::Layout => 5,
    }
}
