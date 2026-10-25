//! Draws `assets/badge_logo.svg` at the sizes Windows asks for and writes them
//! as `assets/app.ico` (PNG pictures in an ICO file), which `build.rs` embeds
//! in the Windows program, and as one PNG per size in `assets/app-icons/`,
//! which the Linux packages install as the application icon. Run it again when
//! the badge changes: `cargo run -p od-app --example export_app_icon`.

use std::io::Write;
use std::path::Path;

const SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];
/// The sizes of the icon theme the Linux packages install (`hicolor`).
const LINUX_SIZES: [u32; 6] = [16, 32, 48, 64, 128, 256];

fn png(rgba: &[u8], side: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, side, side);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(rgba)?;
    Ok(out)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let svg = std::fs::read(assets.join("badge_logo.svg"))?;
    let mut pictures = Vec::new();
    for side in SIZES {
        let rgba = od_ui::icon_set::rgba(&svg, side)?;
        pictures.push(png(&rgba, side)?);
    }
    // ICONDIR, then one 16-byte ICONDIRENTRY per picture, then the pictures.
    let mut file = Vec::new();
    file.extend_from_slice(&[0, 0, 1, 0]);
    file.extend_from_slice(&u16::try_from(SIZES.len())?.to_le_bytes());
    let mut offset = 6 + 16 * SIZES.len();
    for (side, picture) in SIZES.iter().zip(&pictures) {
        // A side of 256 is written as 0.
        let byte = u8::try_from(*side).unwrap_or(0);
        file.extend_from_slice(&[byte, byte, 0, 0, 1, 0, 32, 0]);
        file.extend_from_slice(&u32::try_from(picture.len())?.to_le_bytes());
        file.extend_from_slice(&u32::try_from(offset)?.to_le_bytes());
        offset += picture.len();
    }
    for picture in &pictures {
        file.extend_from_slice(picture);
    }
    let folder = assets.join("app-icons");
    std::fs::create_dir_all(&folder)?;
    for side in LINUX_SIZES {
        let rgba = od_ui::icon_set::rgba(&svg, side)?;
        std::fs::write(folder.join(format!("{side}x{side}.png")), png(&rgba, side)?)?;
    }
    let target = assets.join("app.ico");
    std::fs::write(&target, &file)?;
    writeln!(
        std::io::stdout(),
        "{}: {} bytes, sizes {SIZES:?}",
        target.display(),
        file.len()
    )?;
    Ok(())
}
