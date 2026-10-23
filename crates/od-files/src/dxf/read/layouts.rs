//! Layouts and viewports in a DXF being read.
//!
//! R2000 and later name their layouts in `LAYOUT` objects, each tied to a
//! block record (`*Paper_Space`, `*Paper_Space0`...) that owns its entities,
//! which are either in `ENTITIES` with group 67 = 1 (owned by 330) or inside
//! the block. `Spaces::scan` reads the layouts first; the main loop then asks
//! `space_of` where each entity goes, and `finish` shifts sheet coordinates
//! (a layout's origin is its printable area) and stores the layouts.
//! Older files have one unnamed paper space: it becomes "Layout1".

use std::collections::HashMap;

use od_core::{Document, Layout, LayoutId, Point, Viewport, MIN_SHEET};

use super::{decode, Importer, Record};

/// Where an entity belongs.
pub(super) enum Space {
    Model,
    Sheet(LayoutId),
    /// A paper space entity with no layout to put it on.
    Skip,
}

impl Space {
    pub(super) fn layout(&self) -> Option<LayoutId> {
        match self {
            Space::Sheet(id) => Some(*id),
            _ => None,
        }
    }
}

/// A4 landscape, for a layout whose size the file does not give.
const DEFAULT_SHEET: (f64, f64) = (297.0, 210.0);

#[derive(Default)]
pub(super) struct Spaces {
    layouts: Vec<Layout>,
    /// Lower-left corner of each sheet's printable area, added to its coordinates.
    offsets: HashMap<LayoutId, (f64, f64)>,
    by_record: HashMap<String, LayoutId>,
    by_block: HashMap<String, LayoutId>,
    by_name: HashMap<String, LayoutId>,
    /// The layout of `*Paper_Space`, where entities with no owner go.
    first: Option<LayoutId>,
    /// Layer names by the handle of their table record (for frozen layers).
    layer_names: HashMap<String, String>,
    /// Frozen layer names to resolve once every layer is known.
    frozen: Vec<(LayoutId, usize, Vec<String>)>,
    unsupported_views: usize,
    clipped: usize,
}

/// What a `LAYOUT` object says before the layouts are built.
struct Raw {
    name: String,
    tab: i64,
    record: String,
    size: Option<(f64, f64)>,
    margins: (f64, f64),
}

fn raw_layout(record: &Record) -> Raw {
    let mut raw = Raw {
        name: String::new(),
        tab: 0,
        record: String::new(),
        size: None,
        margins: (0.0, 0.0),
    };
    let mut sub = String::new();
    let (mut paper, mut limits) = ([None; 2], [None; 4]);
    for group in &record.groups {
        let number = group.value.parse::<f64>().ok().filter(|v| v.is_finite());
        match (sub.as_str(), group.code) {
            (_, 100) => sub.clone_from(&group.value),
            ("AcDbPlotSettings", 40) => raw.margins.0 = number.unwrap_or(0.0),
            ("AcDbPlotSettings", 41) => raw.margins.1 = number.unwrap_or(0.0),
            ("AcDbPlotSettings", 44) => paper[0] = number,
            ("AcDbPlotSettings", 45) => paper[1] = number,
            ("AcDbLayout", 1) => raw.name = decode(&group.value),
            ("AcDbLayout", 71) => raw.tab = group.value.parse().unwrap_or(0),
            ("AcDbLayout", 330) => raw.record = group.value.to_uppercase(),
            ("AcDbLayout", 10) => limits[0] = number,
            ("AcDbLayout", 20) => limits[1] = number,
            ("AcDbLayout", 11) => limits[2] = number,
            ("AcDbLayout", 21) => limits[3] = number,
            _ => {}
        }
    }
    // The paper size in millimetres, else the size of the layout's limits.
    let fits = |w: f64, h: f64| w >= MIN_SHEET && h >= MIN_SHEET;
    raw.size = match (paper, limits) {
        ([Some(w), Some(h)], _) if fits(w, h) => Some((w, h)),
        (_, [Some(x0), Some(y0), Some(x1), Some(y1)]) if fits(x1 - x0, y1 - y0) => {
            Some((x1 - x0, y1 - y0))
        }
        _ => None,
    };
    raw
}

impl Spaces {
    /// Read the layouts of a file (and make an implicit one for old files whose
    /// paper space holds entities).
    pub(super) fn scan(records: &[Record], document: &mut Document) -> Self {
        let mut spaces = Spaces::default();
        let (mut section, mut table) = (String::new(), String::new());
        let mut block_names: HashMap<String, String> = HashMap::new();
        let mut raws = Vec::new();
        let (mut layout_objects, mut paper_entities) = (0, 0);
        for record in records {
            match record.name.as_str() {
                "SECTION" => section = record.text(2).unwrap_or("").to_owned(),
                "ENDSEC" => section.clear(),
                "TABLE" => table = record.text(2).unwrap_or("").to_owned(),
                "ENDTAB" => table.clear(),
                "BLOCK_RECORD" if table == "BLOCK_RECORD" => {
                    if let (Some(handle), Some(name)) = (record.text(5), record.text(2)) {
                        block_names.insert(handle.to_uppercase(), name.to_lowercase());
                    }
                }
                "LAYER" if table == "LAYER" => {
                    if let (Some(handle), Some(name)) = (record.text(5), record.text(2)) {
                        spaces
                            .layer_names
                            .insert(handle.to_uppercase(), decode(name));
                    }
                }
                "LAYOUT" if section == "OBJECTS" => {
                    layout_objects += 1;
                    raws.push(raw_layout(record));
                }
                _ if section == "ENTITIES" && record.int(67) == Some(1) => paper_entities += 1,
                _ => {}
            }
        }
        raws.retain(|r| {
            block_names
                .get(&r.record)
                .is_none_or(|b| b != "*model_space")
                && !r.name.eq_ignore_ascii_case("model")
        });
        raws.sort_by_key(|r| r.tab);
        if layout_objects == 0 && paper_entities > 0 {
            raws.push(Raw {
                name: "Layout1".into(),
                tab: 1,
                record: String::new(),
                size: None,
                margins: (0.0, 0.0),
            });
        }
        for raw in raws {
            let id = document.allocate_layout_id();
            let mut name = if raw.name.trim().is_empty() {
                format!("Layout{}", spaces.layouts.len() + 1)
            } else {
                raw.name.clone()
            };
            while spaces.by_name.contains_key(&name.to_lowercase()) {
                name.push_str(" (2)");
            }
            let (width, height) = raw.size.unwrap_or(DEFAULT_SHEET);
            spaces.by_name.insert(name.to_lowercase(), id);
            if let Some(block) = block_names.get(&raw.record) {
                spaces.by_block.insert(block.clone(), id);
            }
            spaces.by_record.insert(raw.record.clone(), id);
            spaces.offsets.insert(id, raw.margins);
            if spaces.first.is_none()
                || block_names
                    .get(&raw.record)
                    .is_some_and(|b| b == "*paper_space")
            {
                spaces.first = Some(id);
            }
            spaces.layouts.push(Layout {
                id,
                name,
                width,
                height,
                viewports: Vec::new(),
            });
        }
        spaces
    }

    /// Whether a block holds a sheet's entities, so the main loop reads it.
    pub(super) fn is_sheet_block(&self, block: &str) -> bool {
        self.by_block.contains_key(&block.to_lowercase())
    }

    /// Where an entity goes: model space, a layout's sheet, or nowhere.
    pub(super) fn space_of(&self, record: &Record, block: &str) -> Space {
        if !block.is_empty() {
            return self
                .by_block
                .get(&block.to_lowercase())
                .map_or(Space::Skip, |id| Space::Sheet(*id));
        }
        if record.int(67) != Some(1) {
            return Space::Model;
        }
        let owner = record
            .text(330)
            .and_then(|h| self.by_record.get(&h.to_uppercase()));
        let named = record
            .text(410)
            .and_then(|n| self.by_name.get(&decode(n).to_lowercase()));
        owner
            .or(named)
            .or(self.first.as_ref())
            .map_or(Space::Skip, |id| Space::Sheet(*id))
    }

    /// Add a `VIEWPORT` entity to its layout. The sheet-wide viewport (id 1) is
    /// not a window, and views that are not plan views cannot be shown.
    pub(super) fn viewport(&mut self, record: &Record, layout: LayoutId, document: &mut Document) {
        if record.int(69) == Some(1) {
            return;
        }
        let number = |code, default| record.number(code).unwrap_or(default);
        let flags = record.int(90).unwrap_or(0);
        let looks_down =
            number(16, 0.0).abs() < 1e-9 && number(26, 0.0).abs() < 1e-9 && number(36, 1.0) > 0.0;
        let untwisted = number(51, 0.0).abs() < 1e-9;
        let view_height = number(45, 0.0);
        let (width, height) = (number(40, 0.0), number(41, 0.0));
        if !looks_down
            || !untwisted
            || flags & 1 != 0
            || view_height <= 0.0
            || width <= 0.0
            || height <= 0.0
        {
            self.unsupported_views += 1;
            return;
        }
        self.clipped += usize::from(flags & 0x1_0000 != 0);
        let viewport = Viewport {
            id: document.allocate_viewport_id(),
            center: Point::new(number(10, 0.0), number(20, 0.0)),
            width,
            height,
            // The view centre is relative to the target, in the view's own axes.
            view_center: Point::new(
                number(17, 0.0) + number(12, 0.0),
                number(27, 0.0) + number(22, 0.0),
            ),
            scale: height / view_height,
            locked: flags & 0x4000 != 0,
            on: record.int(68).unwrap_or(1) != 0,
            frozen: Vec::new(),
        };
        let frozen: Vec<String> = record
            .groups
            .iter()
            .filter(|g| g.code == 331)
            .filter_map(|g| self.layer_names.get(&g.value.to_uppercase()).cloned())
            .collect();
        if let Some(sheet) = self.layouts.iter_mut().find(|l| l.id == layout) {
            self.frozen.push((layout, sheet.viewports.len(), frozen));
            sheet.viewports.push(viewport);
        }
    }

    /// Resolve frozen layers, move sheet coordinates to the sheet's corner and
    /// store the layouts.
    pub(super) fn finish(mut self, importer: &mut Importer) {
        for (layout, index, names) in std::mem::take(&mut self.frozen) {
            let layers: Vec<_> = names.iter().map(|n| importer.layer_named(n)).collect();
            if let Some(vp) = self
                .layouts
                .iter_mut()
                .find(|l| l.id == layout)
                .and_then(|l| l.viewports.get_mut(index))
            {
                vp.set_frozen(&layers, true);
            }
        }
        let document = &mut importer.document;
        for layout in &mut self.layouts {
            let (dx, dy) = self.offsets.get(&layout.id).copied().unwrap_or((0.0, 0.0));
            if dx == 0.0 && dy == 0.0 {
                continue;
            }
            for vp in &mut layout.viewports {
                vp.center = vp.center.translated(dx, dy);
            }
            for id in document.objects_of_layout(layout.id) {
                if let Some(entity) = document.get(id).map(|e| e.translated(dx, dy)) {
                    document.replace(id, entity);
                }
            }
        }
        document.set_layouts(self.layouts);
        document.set_active_layout(None);
    }

    pub(super) fn warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        if self.unsupported_views > 0 {
            warnings.push(format!(
                "Skipped {} viewport{} that show a rotated, perspective or 3D view.",
                self.unsupported_views,
                if self.unsupported_views == 1 { "" } else { "s" }
            ));
        }
        if self.clipped > 0 {
            warnings.push(format!(
                "{} viewport{} with a non-rectangular outline came in as rectangles.",
                self.clipped,
                if self.clipped == 1 { "" } else { "s" }
            ));
        }
        warnings
    }
}
