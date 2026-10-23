use std::time::Duration;

use od_core::{Entity, Point};

use super::*;

struct Area(PathBuf);

impl Area {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("od-previews-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Area(dir)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Area {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn key(path: &Path, ms: u64, len: u64) -> FileKey {
    FileKey {
        path: path.to_owned(),
        modified_ms: ms,
        len,
    }
}

fn drawing() -> Document {
    let mut doc = Document::default();
    doc.add(Entity::Circle {
        center: Point::new(0.0, 0.0),
        radius: 5.0,
    });
    doc
}

fn wait(results: &Receiver<Output>) -> Output {
    results.recv_timeout(Duration::from_secs(20)).unwrap()
}

#[test]
fn a_picture_survives_a_round_trip_through_png_and_stays_small() {
    let picture = preview::render(&drawing()).unwrap();
    let png = encode(&picture).unwrap();
    assert!(png.len() < MAX_PNG_BYTES, "{}", png.len());
    let image = decode(&png).unwrap();
    assert_eq!(image.size, [512, 320]);
    // The first pixel is paper; the picture is the same one that went in.
    let rgba: Vec<u8> = image.pixels.iter().flat_map(|c| c.to_array()).collect();
    assert_eq!(rgba, picture.rgba);
}

#[test]
fn what_is_not_one_of_our_pictures_is_not_decoded() {
    assert!(decode(b"not a png").is_none());
    let small = Preview {
        width: 4,
        height: 4,
        rgba: vec![255; 4 * 4 * 4],
    };
    let mut out = Vec::new();
    let mut e = png::Encoder::new(&mut out, 4, 4);
    e.set_color(png::ColorType::Rgb);
    e.set_depth(png::BitDepth::Eight);
    let mut w = e.write_header().unwrap();
    w.write_image_data(&[255; 4 * 4 * 3]).unwrap();
    w.finish().unwrap();
    assert!(decode(&out).is_none(), "wrong size");
    assert_eq!(small.rgba.len(), 64);
}

#[test]
fn a_picture_is_found_only_by_the_file_it_was_made_for() {
    let area = Area::new("keys");
    let png = encode(&preview::blank()).unwrap();
    let k = key(Path::new("a.odx"), 1000, 50);
    store(&area.0, &k, &png).unwrap();
    assert!(load(&area.0, &k).is_some());
    assert!(
        load(&area.0, &key(Path::new("a.odx"), 1001, 50)).is_none(),
        "date"
    );
    assert!(
        load(&area.0, &key(Path::new("a.odx"), 1000, 51)).is_none(),
        "length"
    );
    assert!(
        load(&area.0, &key(Path::new("b.odx"), 1000, 50)).is_none(),
        "path"
    );
    let leftovers = std::fs::read_dir(&area.0)
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x == "tmp")
        })
        .count();
    assert_eq!(leftovers, 0, "no temporary file is left");
}

#[test]
fn the_folder_keeps_the_newest_within_its_count() {
    let area = Area::new("trim");
    let png = encode(&preview::blank()).unwrap();
    for i in 0..MAX_FILES as u64 + 6 {
        store(&area.0, &key(Path::new("x.odx"), i, 1), &png).unwrap();
        // File dates only differ if the clock moves between writes.
        std::thread::sleep(Duration::from_millis(15));
    }
    let count = std::fs::read_dir(&area.0).unwrap().count();
    assert_eq!(count, MAX_FILES);
    assert!(
        load(&area.0, &key(Path::new("x.odx"), 0, 1)).is_none(),
        "oldest gone"
    );
    assert!(load(&area.0, &key(Path::new("x.odx"), MAX_FILES as u64 + 5, 1)).is_some());
}

#[test]
fn a_saved_drawing_gets_a_picture_and_pending_ends() {
    let area = Area::new("worker");
    let file = area.path("plan.odx");
    std::fs::write(&file, "the file as written").unwrap();
    let (handle, results) = start(area.path("cache"));
    handle.request(&file, drawing());
    assert!(handle.is_pending(&identity(&file)));
    let Output::Rendered(path) = wait(&results) else {
        panic!("expected the render's report");
    };
    assert_eq!(path, identity(&file));
    assert!(!handle.is_pending(&path));
    // Keyed by the file as it is after the write.
    let meta = std::fs::metadata(&file).unwrap();
    let k = FileKey::new(&file, meta.modified().unwrap(), meta.len());
    handle.load(RecentId(1), k.clone());
    let Output::Loaded { id, key, image } = wait(&results) else {
        panic!("expected the picture");
    };
    assert_eq!((id, key), (RecentId(1), k));
    assert!(image.is_some());
    // A change by another program no longer matches.
    std::fs::write(&file, "edited elsewhere, longer").unwrap();
    let meta = std::fs::metadata(&file).unwrap();
    handle.load(
        RecentId(1),
        FileKey::new(&file, meta.modified().unwrap(), meta.len()),
    );
    let Output::Loaded { image, .. } = wait(&results) else {
        panic!("expected a report");
    };
    assert!(image.is_none());
}

#[test]
fn an_opened_drawing_with_a_picture_already_is_not_drawn_again() {
    let area = Area::new("stored");
    let file = area.path("plan.odx");
    std::fs::write(&file, "x").unwrap();
    let (handle, results) = start(area.path("cache"));
    let mut asked = 0;
    handle.request_unless_stored(&file, || {
        asked += 1;
        drawing()
    });
    assert!(matches!(wait(&results), Output::Rendered(_)));
    handle.request_unless_stored(&file, || {
        asked += 1;
        drawing()
    });
    assert_eq!(asked, 1, "the second open found the picture");
    assert!(!handle.is_pending(&identity(&file)));
}

#[test]
fn a_render_that_cannot_find_its_file_reports_and_ends_pending() {
    let area = Area::new("gone");
    let (handle, results) = start(area.path("cache"));
    handle.request(&area.path("never-written.odx"), drawing());
    assert!(matches!(wait(&results), Output::Rendered(_)));
    assert!(!handle.is_pending(&identity(&area.path("never-written.odx"))));
    assert!(
        !area.path("cache").exists() || std::fs::read_dir(area.path("cache")).unwrap().count() == 0
    );
}
