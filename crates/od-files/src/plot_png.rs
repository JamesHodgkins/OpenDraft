//! A plot sheet as a PNG: the same paths and fills the SVG and PDF writers
//! take, drawn with anti-aliasing on white paper. It is for looking at (a
//! model or a person cannot see an SVG's coordinates as a picture), not for
//! printing: line widths never go below one pixel, so a fine line stays
//! visible at the size of a screenshot.

use od_core::plot::PlotSheet;
use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform,
};

/// Why no picture could be made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PngError {
    /// A side of nothing, or too big to hold in memory.
    Size,
    /// The encoder failed.
    Encode(String),
}

impl std::fmt::Display for PngError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PngError::Size => f.write_str("the picture has no size, or is too large to draw"),
            PngError::Encode(why) => write!(f, "the picture could not be encoded: {why}"),
        }
    }
}

impl std::error::Error for PngError {}

/// The pixel size of `sheet` drawn `width` pixels wide: the sheet's own shape
/// (a side is never below one pixel).
pub fn png_size(sheet: &PlotSheet, width: u32) -> (u32, u32) {
    let aspect = if sheet.width > 0.0 {
        sheet.height / sheet.width
    } else {
        1.0
    };
    // Saturating: an absurd shape asks for an absurd height, which the caller
    // refuses by its limits.
    let height = (f64::from(width) * aspect).round().max(1.0) as u32;
    (width.max(1), height)
}

/// `sheet` as a PNG of `width` by `height` pixels, the sheet scaled to the
/// width.
pub fn png(sheet: &PlotSheet, width: u32, height: u32) -> Result<Vec<u8>, PngError> {
    let mut pixmap = Pixmap::new(width, height).ok_or(PngError::Size)?;
    pixmap.fill(Color::WHITE);
    if !(sheet.width.is_finite() && sheet.width > 0.0) {
        return encode(&pixmap);
    }
    // Millimetres to pixels, and up is up: the sheet's y runs from the
    // bottom.
    let scale = (f64::from(width) / sheet.width) as f32;
    let to_pixels = Transform::from_row(scale, 0.0, 0.0, -scale, 0.0, height as f32);
    let one_pixel = 1.0 / scale;

    for path in &sheet.paths {
        let Some(first) = path.points.first() else {
            continue;
        };
        let mut builder = PathBuilder::new();
        builder.move_to(first.x as f32, first.y as f32);
        for p in path.points.iter().skip(1) {
            builder.line_to(p.x as f32, p.y as f32);
        }
        if path.points.len() == 1 {
            // A point: a zero-length stroke with round caps is a dot.
            builder.line_to(first.x as f32, first.y as f32);
        }
        if path.closed {
            builder.close();
        }
        let Some(shape) = builder.finish() else {
            continue;
        };
        let stroke = Stroke {
            width: (path.width as f32).max(one_pixel),
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        pixmap.stroke_path(&shape, &ink(path.rgb), &stroke, to_pixels, None);
    }
    for fill in &sheet.fills {
        let mut builder = PathBuilder::new();
        for contour in &fill.contours {
            let Some((first, rest)) = contour.split_first() else {
                continue;
            };
            builder.move_to(first.x as f32, first.y as f32);
            for p in rest {
                builder.line_to(p.x as f32, p.y as f32);
            }
            builder.close();
        }
        if let Some(shape) = builder.finish() {
            pixmap.fill_path(&shape, &ink(fill.rgb), FillRule::Winding, to_pixels, None);
        }
    }
    encode(&pixmap)
}

fn ink(rgb: [u8; 3]) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(rgb[0], rgb[1], rgb[2], 255);
    paint.anti_alias = true;
    paint
}

fn encode(pixmap: &Pixmap) -> Result<Vec<u8>, PngError> {
    pixmap
        .encode_png()
        .map_err(|e| PngError::Encode(e.to_string()))
}

#[cfg(test)]
mod tests {
    use od_core::plot::{PlotFill, PlotPath};
    use od_core::Point;

    use super::*;

    fn sheet(paths: Vec<PlotPath>, fills: Vec<PlotFill>) -> PlotSheet {
        PlotSheet {
            width: 100.0,
            height: 50.0,
            paths,
            fills,
            notes: Vec::new(),
        }
    }

    fn path(points: &[(f64, f64)], rgb: [u8; 3], width: f64) -> PlotPath {
        PlotPath {
            points: points.iter().map(|p| Point::new(p.0, p.1)).collect(),
            closed: false,
            rgb,
            width,
        }
    }

    /// Decode a PNG into (width, height, RGBA bytes).
    fn decode(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
        let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .unwrap();
        let mut data = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut data).unwrap();
        assert_eq!(info.color_type, png::ColorType::Rgba);
        (info.width, info.height, data)
    }

    fn pixel(image: &(u32, u32, Vec<u8>), x: u32, y: u32) -> [u8; 3] {
        let at = ((y * image.0 + x) * 4) as usize;
        [image.2[at], image.2[at + 1], image.2[at + 2]]
    }

    #[test]
    fn the_size_keeps_the_sheets_shape() {
        assert_eq!(png_size(&sheet(vec![], vec![]), 400), (400, 200));
        let mut tall = sheet(vec![], vec![]);
        tall.height = 400.0;
        assert_eq!(png_size(&tall, 100), (100, 400));
        let mut flat = sheet(vec![], vec![]);
        flat.height = 0.0;
        assert_eq!(png_size(&flat, 100), (100, 1), "a side is never nothing");
    }

    #[test]
    fn it_is_a_png_of_the_size_asked_on_white_paper() {
        let bytes = png(&sheet(vec![], vec![]), 200, 100).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        let image = decode(&bytes);
        assert_eq!((image.0, image.1), (200, 100));
        assert_eq!(pixel(&image, 0, 0), [255, 255, 255]);
        assert_eq!(pixel(&image, 199, 99), [255, 255, 255]);
    }

    #[test]
    fn a_line_is_drawn_in_its_colour_with_up_at_the_top() {
        // A red line along the sheet's bottom edge (y = 5 of 50), 200 px wide
        // for 100 mm, so 2 px to the mm: the line is near the bottom.
        let line = path(&[(0.0, 5.0), (100.0, 5.0)], [200, 30, 30], 2.0);
        let image = decode(&png(&sheet(vec![line], vec![]), 200, 100).unwrap());
        assert_eq!(pixel(&image, 100, 90), [200, 30, 30], "5 mm up is row 90");
        assert_eq!(pixel(&image, 100, 10), [255, 255, 255], "the top is empty");
    }

    #[test]
    fn a_fine_line_never_goes_below_a_pixel() {
        // 0.01 mm at 2 px per mm would be a fiftieth of a pixel.
        let hair = path(&[(0.0, 25.0), (100.0, 25.0)], [0, 0, 0], 0.01);
        let image = decode(&png(&sheet(vec![hair], vec![]), 200, 100).unwrap());
        let dark = (0..100).filter(|y| pixel(&image, 100, *y)[0] < 128).count();
        assert!(dark >= 1, "a hairline is still seen");
    }

    #[test]
    fn a_fill_covers_what_it_encloses_and_a_hole_stays_open() {
        let square = vec![
            Point::new(20.0, 10.0),
            Point::new(80.0, 10.0),
            Point::new(80.0, 40.0),
            Point::new(20.0, 40.0),
        ];
        // A hole turned the other way, as a letter O has.
        let hole = vec![
            Point::new(40.0, 20.0),
            Point::new(40.0, 30.0),
            Point::new(60.0, 30.0),
            Point::new(60.0, 20.0),
        ];
        let fill = PlotFill {
            contours: vec![square, hole],
            rgb: [0, 0, 255],
        };
        let image = decode(&png(&sheet(vec![], vec![fill]), 200, 100).unwrap());
        assert_eq!(pixel(&image, 60, 50), [0, 0, 255], "inside the shape");
        assert_eq!(pixel(&image, 100, 50), [255, 255, 255], "inside the hole");
        assert_eq!(pixel(&image, 5, 50), [255, 255, 255], "outside it");
    }

    #[test]
    fn a_single_point_is_a_dot_and_nonsense_is_skipped_not_fatal() {
        let dot = path(&[(50.0, 25.0)], [0, 0, 0], 4.0);
        let nothing = path(&[], [0, 0, 0], 1.0);
        let broken = path(&[(f64::NAN, 0.0), (1.0, f64::INFINITY)], [0, 0, 0], 1.0);
        let image = decode(&png(&sheet(vec![dot, nothing, broken], vec![]), 200, 100).unwrap());
        assert!(pixel(&image, 100, 50)[0] < 128, "the dot is there");
    }

    #[test]
    fn a_picture_with_no_size_is_refused() {
        assert_eq!(
            png(&sheet(vec![], vec![]), 0, 10).unwrap_err(),
            PngError::Size
        );
        assert_eq!(
            png(&sheet(vec![], vec![]), 10, 0).unwrap_err(),
            PngError::Size
        );
    }
}
