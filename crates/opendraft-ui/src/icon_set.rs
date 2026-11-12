//! Icons drawn from SVG files read when the app starts, so a redrawn icon
//! shows on the next launch with no rebuild. Each file is `<name>.svg` in the
//! icon folder, named after its constant in [`crate::icons`] (`SQUARE_SLASH`
//! is `square-slash.svg`); `scripts/export_icons.py` writes the library. An
//! icon with no file, or a file that cannot be read, is drawn from the Lucide
//! font as before.
//!
//! The same drawings are built into the program, so an install with no icon
//! folder, or a folder missing some files, still shows every icon; a file in
//! the folder replaces the built-in drawing of that name.
//!
//! A drawing is either plain or coloured. A plain one (all its colours are
//! grey or white, as Lucide's are) counts only for where it is opaque, and is
//! painted in the colour asked for, so it follows the theme and dims when
//! disabled. A coloured one (any clearly coloured part, such as a green point)
//! is drawn in its own colours, only fading when disabled; `currentColor` in it
//! is white. Each icon is rasterised the first time it is drawn at a size.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use egui::{pos2, Align2, Color32, ColorImage, Context, FontId, Painter, Pos2, Rect, Vec2};
use resvg::{tiny_skia, usvg};

/// One icon's SVG file.
struct Drawing {
    data: Vec<u8>,
    /// Drawn in its own colours, not the theme's.
    coloured: bool,
}

struct IconSet {
    /// The SVG files read, by icon.
    files: HashMap<char, Drawing>,
    /// Rasterised icons, by icon and size in pixels.
    textures: Mutex<HashMap<(char, u32), egui::TextureHandle>>,
}

static SET: OnceLock<IconSet> = OnceLock::new();

include!(concat!(env!("OUT_DIR"), "/embedded_icons.rs"));

/// The drawing built into the program for a file name.
fn embedded(name: &str) -> Option<&'static [u8]> {
    let at = EMBEDDED.binary_search_by_key(&name, |(n, _)| n).ok()?;
    Some(EMBEDDED[at].1)
}

/// What [`load`] found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Loaded {
    /// The folder whose files replace the built-in drawings, if there is one.
    pub folder: Option<PathBuf>,
    /// How many icons have a drawing.
    pub count: usize,
    /// Files there that could not be used, with why.
    pub failed: Vec<(String, String)>,
}

/// The file an icon's drawing is read from: `SQUARE_SLASH` is
/// `square-slash.svg`.
pub fn file_name(constant: &str) -> String {
    format!("{}.svg", constant.to_ascii_lowercase().replace('_', "-"))
}

/// Read the drawings: those in `folder` where it has one, else the built-in
/// ones. The first call wins, as the app reads them once when it starts.
pub fn load(folder: Option<&Path>) -> Loaded {
    let (files, loaded) = read(folder);
    let _ = SET.set(IconSet {
        files,
        textures: Mutex::new(HashMap::new()),
    });
    loaded
}

fn read(folder: Option<&Path>) -> (HashMap<char, Drawing>, Loaded) {
    let mut loaded = Loaded {
        folder: folder.map(Path::to_owned),
        ..Loaded::default()
    };
    let mut files = HashMap::new();
    for (constant, icon) in crate::icons::ALL {
        let name = file_name(constant);
        let mut found = None;
        if let Some(folder) = folder {
            match std::fs::read(folder.join(&name)) {
                Ok(data) => match drawing(data) {
                    Ok(d) => found = Some(d),
                    Err(why) => loaded.failed.push((name.clone(), why)),
                },
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => loaded.failed.push((name.clone(), e.to_string())),
            }
        }
        // A missing or unusable file in the folder leaves the built-in one.
        let found = found.or_else(|| embedded(&name).and_then(|d| drawing(d.to_vec()).ok()));
        if let Some(d) = found {
            files.insert(*icon, d);
            loaded.count += 1;
        }
    }
    (files, loaded)
}

/// A usable drawing from an SVG file's bytes.
fn drawing(data: Vec<u8>) -> Result<Drawing, String> {
    let pixmap = render(&data, 32)?;
    Ok(Drawing {
        coloured: is_coloured(&pixmap),
        data,
    })
}

/// How far apart the strongest and weakest of a pixel's colour channels are
/// for it to count as coloured, not grey.
const COLOURED: u8 = 48;

/// The drawing as a `px` square picture in its own colours. `currentColor` is
/// white, so Lucide's strokes are white.
fn render(data: &[u8], px: u32) -> Result<tiny_skia::Pixmap, String> {
    let text = String::from_utf8_lossy(data).replace("currentColor", "#ffffff");
    let tree = usvg::Tree::from_data(text.as_bytes(), &usvg::Options::default())
        .map_err(|e| e.to_string())?;
    let size = tree.size();
    let side = size.width().max(size.height());
    let mut pixmap = tiny_skia::Pixmap::new(px, px).ok_or("no size")?;
    if side <= 0.0 {
        return Err("the drawing has no size".to_owned());
    }
    // Fit the drawing's box into the square, centred.
    let scale = px as f32 / side;
    let dx = (px as f32 - size.width() * scale) / 2.0;
    let dy = (px as f32 - size.height() * scale) / 2.0;
    let transform = tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, dx, dy);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Ok(pixmap)
}

/// The drawing as `px` square straight-alpha RGBA bytes, for a window icon.
pub fn rgba(data: &[u8], px: u32) -> Result<Vec<u8>, String> {
    let pixmap = render(data, px)?;
    Ok(pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect())
}

/// Whether the picture has a clearly coloured part (mostly opaque, so the
/// soft edge of a grey shape never counts).
fn is_coloured(pixmap: &tiny_skia::Pixmap) -> bool {
    pixmap.pixels().iter().any(|p| {
        let c = p.demultiply();
        let (hi, lo) = ([c.red(), c.green(), c.blue()].into_iter())
            .fold((0, 255), |(h, l), v| (h.max(v), l.min(v)));
        c.alpha() > 128 && hi - lo > COLOURED
    })
}

/// The drawing as a `px` square image: its own colours, or for a plain one a
/// white mask, opaque where the drawing is.
fn rasterise(data: &[u8], px: u32, coloured: bool) -> Result<ColorImage, String> {
    let pixmap = render(data, px)?;
    let mut image = ColorImage::new([px as usize; 2], Color32::TRANSPARENT);
    for (to, from) in image.pixels.iter_mut().zip(pixmap.pixels()) {
        *to = if coloured {
            // tiny-skia's pixels are already premultiplied; egui's
            // `from_rgba_unmultiplied` would premultiply in linear light and
            // make translucent parts far more opaque than drawn.
            Color32::from_rgba_premultiplied(from.red(), from.green(), from.blue(), from.alpha())
        } else {
            Color32::from_white_alpha(from.alpha())
        };
    }
    Ok(image)
}

/// `icon` rasterised `px` square, if it has a drawing, and whether that
/// drawing is coloured.
fn texture(ctx: &Context, icon: char, px: u32) -> Option<(egui::TextureId, bool)> {
    let set = SET.get()?;
    let drawing = set.files.get(&icon)?;
    let mut textures = set.textures.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(handle) = textures.get(&(icon, px)) {
        return Some((handle.id(), drawing.coloured));
    }
    let image = rasterise(&drawing.data, px, drawing.coloured).ok()?;
    let name = format!("icon-{:x}-{px}", u32::from(icon));
    let handle = ctx.load_texture(name, image, egui::TextureOptions::LINEAR);
    let id = handle.id();
    textures.insert((icon, px), handle);
    Some((id, drawing.coloured))
}

/// The tint for a coloured drawing asked for in `color`: its own colours,
/// faded as far as `color` is darker than normal text (a disabled button).
fn fade(color: Color32) -> Color32 {
    let lum =
        0.299 * f32::from(color.r()) + 0.587 * f32::from(color.g()) + 0.114 * f32::from(color.b());
    Color32::from_white_alpha((lum / 200.0).clamp(0.0, 1.0).mul_add(255.0, 0.0) as u8)
}

/// The size in pixels of an icon `size` points square.
fn pixels(ctx: &Context, size: f32) -> u32 {
    (size * ctx.pixels_per_point()).round().clamp(1.0, 512.0) as u32
}

/// Draw `icon` `size` points square, placed at `at` by `align`, in `color`:
/// its drawing if it has one, else its glyph. Returns where it went.
pub fn paint(
    painter: &Painter,
    at: Pos2,
    align: Align2,
    icon: char,
    size: f32,
    color: Color32,
) -> Rect {
    let ctx = painter.ctx();
    let Some((id, coloured)) = texture(ctx, icon, pixels(ctx, size)) else {
        return painter.text(
            at,
            align,
            crate::icons::glyph(icon),
            FontId::new(size, crate::icon_family()),
            color,
        );
    };
    // On whole pixels, so the mask is not blurred by resampling.
    let ppp = ctx.pixels_per_point();
    let rect = align.anchor_size(at, Vec2::splat(size));
    let min = pos2(
        (rect.min.x * ppp).round() / ppp,
        (rect.min.y * ppp).round() / ppp,
    );
    let rect = Rect::from_min_size(min, rect.size());
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    painter.image(id, rect, uv, if coloured { fade(color) } else { color });
    rect
}

/// `icon` as an image `size` points square, for buttons and labels (tint it
/// with `Image::tint`); `None` when it has no drawing, to use the glyph
/// ([`crate::widgets::icon_text`]).
pub fn image(ctx: &Context, icon: char, size: f32) -> Option<egui::Image<'static>> {
    let (id, _) = texture(ctx, icon, pixels(ctx, size))?;
    Some(egui::Image::new(egui::load::SizedTexture::new(
        id,
        Vec2::splat(size),
    )))
}

/// An icon glyph as rich text, sized and in the icon font family: for egui
/// widgets that take only text, when the icon has no drawing ([`image`]).
pub fn icon_text(icon: char, size: f32) -> egui::RichText {
    egui::RichText::new(crate::icons::glyph(icon).to_string())
        .family(crate::icon_family())
        .size(size)
}

/// Square, frameless icon button with a tooltip. The frame appears on hover.
pub fn icon_button(
    ui: &mut egui::Ui,
    theme: &crate::Theme,
    icon: char,
    tooltip: &str,
) -> egui::Response {
    let side = theme.control_height;
    ui.scope(|ui| {
        let inactive = &mut ui.style_mut().visuals.widgets.inactive;
        inactive.weak_bg_fill = Color32::TRANSPARENT;
        inactive.bg_stroke = egui::Stroke::NONE;
        let padding = ((side - 15.0) / 2.0).max(0.0);
        ui.add(Icon::new(icon, 15.0).button().padding(padding))
            .on_hover_text(tooltip)
    })
    .inner
}

/// How an [`Icon`] widget looks behind its icon.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Look {
    /// Nothing: a label, which may still be clicked.
    Plain,
    /// Lit when hovered or selected, as a selectable label.
    Toggle(bool),
    /// A button's frame.
    Button,
}

/// An icon as a widget, `size` points square with `padding` round it: its
/// drawing, or its glyph. Disabled in a disabled `Ui` (`add_enabled`).
#[derive(Clone, Copy, Debug)]
pub struct Icon {
    icon: char,
    size: f32,
    padding: f32,
    sense: egui::Sense,
    look: Look,
    color: Option<Color32>,
}

impl Icon {
    /// A plain icon that does nothing when clicked.
    pub fn new(icon: char, size: f32) -> Self {
        Self {
            icon,
            size,
            padding: 2.0,
            sense: egui::Sense::hover(),
            look: Look::Plain,
            color: None,
        }
    }

    /// Clickable, still with no background.
    pub fn clickable(mut self) -> Self {
        self.sense = egui::Sense::click();
        self
    }

    /// Clickable with a button's frame.
    pub fn button(mut self) -> Self {
        self.sense = egui::Sense::click();
        self.look = Look::Button;
        self
    }

    /// Clickable and lit while `selected`, as a selectable label.
    pub fn toggle(mut self, selected: bool) -> Self {
        self.sense = egui::Sense::click();
        self.look = Look::Toggle(selected);
        self
    }

    /// The icon's colour; the widget text colour otherwise.
    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }
}

impl egui::Widget for Icon {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let side = self.size + 2.0 * self.padding;
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), self.sense);
        if !ui.is_rect_visible(rect) {
            return response;
        }
        let selected = matches!(self.look, Look::Toggle(true));
        let visuals = ui.style().interact_selectable(&response, selected);
        let lit = match self.look {
            Look::Plain => false,
            Look::Toggle(on) => on || response.hovered(),
            Look::Button => true,
        };
        if lit {
            ui.painter().rect(
                rect,
                visuals.corner_radius,
                visuals.weak_bg_fill,
                visuals.bg_stroke,
                egui::StrokeKind::Inside,
            );
        }
        let color = self.color.unwrap_or_else(|| visuals.text_color());
        paint(
            ui.painter(),
            rect.center(),
            Align2::CENTER_CENTER,
            self.icon,
            self.size,
            color,
        );
        crate::widgets::focus_ring(ui, &crate::Theme::default(), &response, rect);
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RING: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"
        fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/></svg>"#;

    const GREEN_DOT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
        <path d="M2 12H22" stroke="rgb(243,243,243)" stroke-width="2"/>
        <circle cx="12" cy="12" r="5" fill="rgb(0,199,154)"/></svg>"#;

    #[test]
    fn a_drawing_with_colour_keeps_it_and_a_grey_one_follows_the_theme() {
        assert!(is_coloured(&render(GREEN_DOT.as_bytes(), 24).unwrap()));
        assert!(
            !is_coloured(&render(RING.as_bytes(), 24).unwrap()),
            "currentColor is white"
        );
        let grey = GREEN_DOT.replace("rgb(0,199,154)", "rgb(200,200,200)");
        assert!(!is_coloured(&render(grey.as_bytes(), 24).unwrap()));

        let image = rasterise(GREEN_DOT.as_bytes(), 24, true).unwrap();
        assert_eq!(image.pixels[12 * 24 + 12], Color32::from_rgb(0, 199, 154));
        assert_eq!(image.pixels[12 * 24 + 4], Color32::from_rgb(243, 243, 243));
        assert!(fade(Color32::from_rgb(220, 220, 220)).a() == 255);
        assert!(
            fade(Color32::from_rgb(100, 100, 100)).a() < 140,
            "disabled fades"
        );
    }

    #[test]
    fn the_library_has_a_file_for_every_icon_and_an_icon_for_every_file() {
        let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/icons");
        let names: Vec<String> = crate::icons::ALL
            .iter()
            .map(|(n, _)| file_name(n))
            .collect();
        for name in &names {
            assert!(
                folder.join(name).is_file(),
                "{name} is missing: run scripts/export_icons.py"
            );
        }
        let files = std::fs::read_dir(&folder).unwrap().flatten();
        for file in files.map(|f| f.file_name().to_string_lossy().into_owned()) {
            if file.ends_with(".svg") {
                assert!(names.contains(&file), "{file} is not an icon in icons.rs");
            }
        }
        let (_, loaded) = read(Some(&folder));
        assert_eq!(loaded.count, names.len());
        assert!(loaded.failed.is_empty(), "{:?}", loaded.failed);
    }

    #[test]
    fn icons_have_their_own_identifier_and_a_font_glyph() {
        let all = crate::icons::ALL;
        let ids: std::collections::HashSet<char> = all.iter().map(|(_, c)| *c).collect();
        assert_eq!(ids.len(), all.len(), "two icons share an identifier");
        for (name, icon) in all {
            let glyph = crate::icons::glyph(*icon);
            assert_ne!(glyph, *icon, "{name} has no glyph");
            assert!(
                ('\u{e000}'..='\u{f8ff}').contains(&glyph),
                "{name}: {glyph:?}"
            );
        }
        assert_eq!(crate::icons::glyph('x'), 'x');
    }

    #[test]
    fn files_are_named_after_their_constants() {
        assert_eq!(file_name("DRAW_HATCH"), "draw-hatch.svg");
        assert_eq!(file_name("GRID_2X2"), "grid-2x2.svg");
    }

    #[test]
    fn a_drawing_becomes_a_white_mask_opaque_only_where_it_is_drawn() {
        let image = rasterise(RING.as_bytes(), 24, false).unwrap();
        assert_eq!(image.size, [24, 24]);
        let at = |x: usize, y: usize| image.pixels[y * 24 + x];
        assert_eq!(at(12, 12), Color32::TRANSPARENT, "inside the ring");
        assert_eq!(at(12, 2).a(), 255, "on the ring");
        assert_eq!(at(12, 2), Color32::WHITE);
    }

    #[test]
    fn a_file_in_the_folder_replaces_the_built_in_drawing_and_a_bad_one_is_reported() {
        let folder = std::env::temp_dir().join(format!("od_icons_{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("draw-circle-radius.svg"), RING).unwrap();
        std::fs::write(folder.join("draw-rectangle.svg"), "not a drawing").unwrap();
        std::fs::write(folder.join("unknown-name.svg"), RING).unwrap();
        let (files, loaded) = read(Some(&folder));
        std::fs::remove_dir_all(&folder).unwrap();
        assert_eq!(loaded.count, crate::icons::ALL.len());
        assert_eq!(
            files[&crate::icons::DRAW_CIRCLE_RADIUS].data,
            RING.as_bytes()
        );
        let rectangle = &files[&crate::icons::DRAW_RECTANGLE].data;
        assert_ne!(
            rectangle.as_slice(),
            b"not a drawing",
            "the built-in one stays"
        );
        assert_eq!(loaded.failed.len(), 1);
        assert_eq!(loaded.failed[0].0, "draw-rectangle.svg");
    }

    #[test]
    fn with_no_folder_every_icon_has_its_built_in_drawing() {
        let (files, loaded) = read(None);
        assert_eq!(loaded.count, crate::icons::ALL.len());
        assert_eq!(files.len(), crate::icons::ALL.len());
        assert!(loaded.failed.is_empty(), "{:?}", loaded.failed);
        assert_eq!(loaded.folder, None);
    }

    #[test]
    fn the_built_in_table_is_sorted_for_lookup() {
        assert!(EMBEDDED.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(embedded("draw-line.svg").is_some());
        assert!(embedded("nothing.svg").is_none());
    }
}
