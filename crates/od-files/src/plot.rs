//! Plot files: a layout written as SVG or PDF (vector, millimetres, 1:1).
//!
//! The geometry is `od_core::plot`; this only writes it. The PDF is
//! hand-written (one page, stroked paths), which every viewer and printer
//! accepts and needs no dependency.

use std::fmt::Write as _;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use od_core::plot::{plot_layout, PlotError, PlotFill, PlotPath, PlotSheet};
use od_core::{Document, LayoutId};

use crate::{write_atomically, FileError};

/// Points per millimetre in a PDF.
const POINTS_PER_MM: f64 = 72.0 / 25.4;

/// The name and extensions of the plot files (PDF first, the default).
pub fn plot_filter() -> (&'static str, &'static [&'static str]) {
    ("Plot file (PDF, SVG)", &["pdf", "svg"])
}

/// Write a layout to `path` as PDF or SVG, as its extension says. Returns
/// what the plot could not show faithfully.
pub fn plot_cancellable(
    path: &Path,
    document: &Document,
    layout: LayoutId,
    cancelled: &AtomicBool,
) -> Result<Vec<String>, FileError> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    let sheet = plot_layout(document, layout).map_err(|e| match e {
        PlotError::NoLayout => FileError::Invalid(e.to_string()),
        PlotError::TooDetailed | PlotError::HatchTooDense => FileError::Unsupported(e.to_string()),
    })?;
    let bytes = match extension.as_deref() {
        Some("pdf") => pdf(&sheet),
        Some("svg") => svg(&sheet).into_bytes(),
        _ => return Err(FileError::UnknownFormat(path.to_owned())),
    };
    write_atomically(path, &bytes, cancelled)?;
    Ok(sheet.notes)
}

/// A number with at most three decimals and no trailing zeros.
fn num(v: f64) -> String {
    let text = format!("{v:.3}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text.is_empty() || text == "-0" {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

/// The sheet as an SVG document, millimetres, white paper, Y flipped to up.
pub fn svg(sheet: &PlotSheet) -> String {
    let (w, h) = (num(sheet.width), num(sheet.height));
    let mut out = String::new();
    let _ = writeln!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\">"
    );
    let _ = writeln!(out, "<rect width=\"{w}\" height=\"{h}\" fill=\"#ffffff\"/>");
    let _ = writeln!(out, "<g transform=\"translate(0 {h}) scale(1 -1)\" fill=\"none\" stroke-linecap=\"round\" stroke-linejoin=\"round\">");
    for path in &sheet.paths {
        let points: Vec<String> = path
            .points
            .iter()
            .map(|p| format!("{},{}", num(p.x), num(p.y)))
            .collect();
        let tag = if path.closed { "polygon" } else { "polyline" };
        let _ = writeln!(
            out,
            "<{tag} points=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>",
            points.join(" "),
            hex(path.rgb),
            num(path.width)
        );
    }
    for fill in &sheet.fills {
        let mut d = String::new();
        for contour in &fill.contours {
            for (i, p) in contour.iter().enumerate() {
                let verb = if i == 0 { "M" } else { "L" };
                let _ = write!(d, "{verb}{},{} ", num(p.x), num(p.y));
            }
            d.push_str("Z ");
        }
        let _ = writeln!(
            out,
            "<path d=\"{}\" fill=\"{}\" fill-rule=\"nonzero\" stroke=\"none\"/>",
            d.trim_end(),
            hex(fill.rgb)
        );
    }
    out.push_str("</g>\n</svg>\n");
    out
}

/// The sheet as a one-page PDF.
pub fn pdf(sheet: &PlotSheet) -> Vec<u8> {
    let mut content = format!("q\n{0:.6} 0 0 {0:.6} 0 0 cm\n1 J 1 j\n", POINTS_PER_MM);
    for path in &sheet.paths {
        content.push_str(&pdf_path(path));
    }
    for fill in &sheet.fills {
        content.push_str(&pdf_fill(fill));
    }
    content.push_str("Q\n");
    let (w, h) = (sheet.width * POINTS_PER_MM, sheet.height * POINTS_PER_MM);
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << >> /Contents 4 0 R >>",
            num(w),
            num(h)
        ),
        format!(
            "<< /Length {} >>\nstream\n{content}endstream",
            content.len()
        ),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{body}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for offset in offsets {
        out.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

fn pdf_fill(fill: &PlotFill) -> String {
    let [r, g, b] = fill.rgb.map(|c| f64::from(c) / 255.0);
    let mut out = format!("{} {} {} rg\n", num(r), num(g), num(b));
    for contour in &fill.contours {
        for (i, p) in contour.iter().enumerate() {
            let verb = if i == 0 { "m" } else { "l" };
            let _ = writeln!(out, "{} {} {verb}", num(p.x), num(p.y));
        }
        out.push_str("h\n");
    }
    out.push_str("f\n");
    out
}

fn pdf_path(path: &PlotPath) -> String {
    let Some((first, rest)) = path.points.split_first() else {
        return String::new();
    };
    let [r, g, b] = path.rgb.map(|c| f64::from(c) / 255.0);
    let mut out = format!(
        "{} {} {} RG\n{} w\n{} {} m\n",
        num(r),
        num(g),
        num(b),
        num(path.width),
        num(first.x),
        num(first.y)
    );
    for p in rest {
        let _ = writeln!(out, "{} {} l", num(p.x), num(p.y));
    }
    out.push_str(if path.closed { "s\n" } else { "S\n" });
    out
}

#[cfg(test)]
mod tests {
    use od_core::Point;

    use super::*;

    fn sheet() -> PlotSheet {
        PlotSheet {
            width: 297.0,
            height: 210.0,
            paths: vec![
                PlotPath {
                    points: vec![Point::new(10.0, 10.5), Point::new(100.25, 20.0)],
                    closed: false,
                    rgb: [0, 0, 0],
                    width: 0.25,
                },
                PlotPath {
                    points: vec![
                        Point::new(0.0, 0.0),
                        Point::new(5.0, 0.0),
                        Point::new(5.0, 5.0),
                    ],
                    closed: true,
                    rgb: [200, 30, 30],
                    width: 0.5,
                },
            ],
            fills: vec![PlotFill {
                contours: vec![vec![
                    Point::new(0.0, 0.0),
                    Point::new(4.0, 0.0),
                    Point::new(4.0, 4.0),
                ]],
                rgb: [1, 2, 3],
            }],
            notes: Vec::new(),
        }
    }

    #[test]
    fn fills_are_written_as_nonzero_paths_in_both_formats() {
        let text = svg(&sheet());
        assert!(text.contains(
            "<path d=\"M0,0 L4,0 L4,4 Z\" fill=\"#010203\" fill-rule=\"nonzero\" stroke=\"none\"/>"
        ));
        let bytes = pdf(&sheet());
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("0.004 0.008 0.012 rg\n0 0 m\n4 0 l\n4 4 l\nh\nf\n"));
    }

    #[test]
    fn numbers_are_short_and_never_negative_zero() {
        assert_eq!(num(0.25), "0.25");
        assert_eq!(num(100.0), "100");
        assert_eq!(num(-0.0001), "0");
        assert_eq!(num(1.23456), "1.235");
    }

    #[test]
    fn svg_is_millimetres_with_one_element_per_path() {
        let text = svg(&sheet());
        assert!(text.contains("width=\"297mm\" height=\"210mm\" viewBox=\"0 0 297 210\""));
        assert_eq!(text.matches("<polyline").count(), 1);
        assert_eq!(text.matches("<polygon").count(), 1);
        assert!(text.contains("points=\"10,10.5 100.25,20\""));
        assert!(text.contains("stroke=\"#c81e1e\" stroke-width=\"0.5\""));
    }

    #[test]
    fn pdf_has_a_valid_cross_reference_table() {
        let bytes = pdf(&sheet());
        let text = String::from_utf8_lossy(&bytes).into_owned();
        assert!(text.starts_with("%PDF-1.4") && text.trim_end().ends_with("%%EOF"));
        let start: usize = text
            .rsplit("startxref\n")
            .next()
            .and_then(|t| t.lines().next())
            .and_then(|n| n.parse().ok())
            .expect("startxref");
        assert!(text[start..].starts_with("xref\n0 5\n"));
        let table: Vec<&str> = text[start..].lines().skip(3).take(4).collect();
        for (i, entry) in table.iter().enumerate() {
            let offset: usize = entry[..10].parse().unwrap();
            assert!(
                text[offset..].starts_with(&format!("{} 0 obj", i + 1)),
                "entry {i} points at its object"
            );
        }
        // The page is A4 landscape in points and the stream's length is right.
        assert!(text.contains("/MediaBox [0 0 841.89 595.276]"), "{text}");
        let stream = text
            .split("\nstream\n")
            .nth(1)
            .unwrap()
            .split("endstream")
            .next()
            .unwrap();
        assert!(text.contains(&format!("/Length {}", stream.len())));
        assert!(stream.contains("10 10.5 m\n100.25 20 l\nS"));
        assert!(stream.contains("0.784 0.118 0.118 RG\n0.5 w"));
        assert!(stream.contains("s\n"), "closed path is closed");
    }

    #[test]
    fn plotting_writes_the_file_and_refuses_other_extensions() {
        let dir = std::env::temp_dir().join(format!("od-plot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut document = Document::default();
        let id = document.allocate_layout_id();
        document.set_layouts(vec![od_core::Layout {
            id,
            name: "A".into(),
            width: 297.0,
            height: 210.0,
            viewports: Vec::new(),
        }]);
        let never = AtomicBool::new(false);
        for name in ["sheet.pdf", "sheet.SVG"] {
            let path = dir.join(name);
            assert_eq!(
                plot_cancellable(&path, &document, id, &never).unwrap(),
                Vec::<String>::new()
            );
            assert!(std::fs::metadata(&path).unwrap().len() > 100);
        }
        assert!(matches!(
            plot_cancellable(&dir.join("sheet.png"), &document, id, &never),
            Err(FileError::UnknownFormat(_))
        ));
        assert!(matches!(
            plot_cancellable(&dir.join("x.pdf"), &document, LayoutId(99), &never),
            Err(FileError::Invalid(_))
        ));
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            2,
            "no temporary left"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
