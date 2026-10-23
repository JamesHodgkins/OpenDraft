#![allow(clippy::print_stdout)] // a command-line tool
//! Time the Home page's preview renderer on drawings given as arguments (or
//! the sample drawings in `private_material/` when there are none; files it
//! cannot find are skipped). Prints the load time, the render time and the PNG
//! size of each (set `OD_PREVIEW_OUT` to a folder to keep the pictures), for choosing the preview cache budgets. Run with `--release`.

use std::path::PathBuf;
use std::time::Instant;

fn png(picture: &od_canvas::preview::Preview) -> Option<Vec<u8>> {
    let rgb: Vec<u8> = picture
        .rgba
        .chunks(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, picture.width, picture.height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::High);
    let mut writer = encoder.write_header().ok()?;
    writer.write_image_data(&rgb).ok()?;
    writer.finish().ok()?;
    Some(out)
}

fn main() {
    let mut paths: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if paths.is_empty() {
        let dir = PathBuf::from("private_material");
        if let Ok(read) = std::fs::read_dir(&dir) {
            paths = read
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.extension()
                        .is_some_and(|x| x == "dwg" || x == "dxf" || x == "odx")
                })
                .collect();
        }
    }
    paths.sort();
    for path in paths {
        let started = Instant::now();
        let Ok(loaded) = od_files::load(&path) else {
            continue;
        };
        let load = started.elapsed();
        let started = Instant::now();
        if std::env::var_os("OD_PREVIEW_BOUNDS").is_some() {
            println!("  bounds {:?}", loaded.document.model_bounds());
        }
        let picture = od_canvas::preview::render(&loaded.document);
        let render = started.elapsed();
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        // OD_PREVIEW_OUT=<folder> also keeps each picture to look at.
        if let (Ok(picture), Some(out)) = (&picture, std::env::var_os("OD_PREVIEW_OUT")) {
            if let Some(bytes) = png(picture) {
                let _ = std::fs::write(PathBuf::from(out).join(format!("{name}.png")), bytes);
            }
        }
        match picture {
            Ok(picture) => println!(
                "{name}: {} objects, load {load:.0?}, render {render:.0?}, png {} bytes",
                loaded.document.len(),
                png(&picture).map_or(0, |p| p.len())
            ),
            Err(error) => println!(
                "{name}: {} objects, load {load:.0?}, render refused: {error}",
                loaded.document.len()
            ),
        }
    }
}
