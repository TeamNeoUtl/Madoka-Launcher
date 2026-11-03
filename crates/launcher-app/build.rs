//! Renders `assets/Madoka.svg` into `$OUT_DIR/icon.rgba` (256x256, straight alpha) for the window icon.
//! On Windows targets it also embeds the same image as the executable icon (resource ordinal 1).
//! The SVG renders without text and without raster images.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};

const SOURCE: &str = "../../assets/Madoka.svg";
const WINDOW_ICON_SIDE: u32 = 256;
const EXECUTABLE_ICON_SIDES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];
const ICO_HEADER_BYTES: u32 = 6;
const ICO_ENTRY_BYTES: u32 = 16;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={SOURCE}");
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let svg = fs::read(manifest.join(SOURCE)).expect("read assets/Madoka.svg");
    let tree = Tree::from_data(&svg, &Options::default()).expect("parse assets/Madoka.svg");
    fs::write(
        out.join("icon.rgba"),
        rgba(&render(&tree, WINDOW_ICON_SIDE)),
    )
    .expect("write icon.rgba");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_executable_icon(&tree, &out);
    }
}

/// Fits `tree` into a `side` x `side` pixmap, centered, keeping the aspect ratio.
fn render(tree: &Tree, side: u32) -> Pixmap {
    let size = tree.size();
    let scale = side as f32 / size.width().max(size.height());
    let transform = Transform::from_scale(scale, scale).post_translate(
        (side as f32 - size.width() * scale) / 2.0,
        (side as f32 - size.height() * scale) / 2.0,
    );
    let mut pixmap = Pixmap::new(side, side).expect("pixmap with non-zero side");
    resvg::render(tree, transform, &mut pixmap.as_mut());
    pixmap
}

fn rgba(pixmap: &Pixmap) -> Vec<u8> {
    pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect()
}

fn embed_executable_icon(tree: &Tree, out: &Path) {
    let images: Vec<(u32, Vec<u8>)> = EXECUTABLE_ICON_SIDES
        .iter()
        .map(|&side| {
            let png = render(tree, side).encode_png().expect("encode png");
            (side, png)
        })
        .collect();
    let ico = out.join("icon.ico");
    fs::write(&ico, ico_of(&images)).expect("write icon.ico");
    let path = ico.display().to_string().replace('\\', "/");
    let rc = out.join("icon.rc");
    fs::write(&rc, format!("1 ICON \"{path}\"\n")).expect("write icon.rc");
    embed_resource::compile(&rc, embed_resource::NONE)
        .manifest_required()
        .expect("compile icon.rc");
}

/// Packs PNG images into an ICO file. A side of 256 is stored as 0.
fn ico_of(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let count = u16::try_from(images.len()).expect("at most 65535 images");
    let mut ico = vec![0, 0, 1, 0];
    ico.extend(count.to_le_bytes());
    let mut offset = ICO_HEADER_BYTES + ICO_ENTRY_BYTES * u32::from(count);
    for (side, png) in images {
        let dimension = u8::try_from(*side).unwrap_or(0);
        let length = u32::try_from(png.len()).expect("png under 4 GiB");
        ico.extend([dimension, dimension, 0, 0]);
        ico.extend(1u16.to_le_bytes());
        ico.extend(32u16.to_le_bytes());
        ico.extend(length.to_le_bytes());
        ico.extend(offset.to_le_bytes());
        offset += length;
    }
    for (_, png) in images {
        ico.extend(png);
    }
    ico
}
