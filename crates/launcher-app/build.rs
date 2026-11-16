//! Renders `assets/Madoka.svg` into `$OUT_DIR/icon.rgba` (256x256, straight alpha) for the window icon.
//! On Windows targets it also embeds the same image as the executable icon (resource ordinal 1).
//! The SVG renders without text and without raster images.

use std::env;
use std::error::Error;
use std::fmt::Display;
use std::fs;
use std::path::{Path, PathBuf};

use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};

const SOURCE: &str = "../../assets/Madoka.svg";
const WINDOW_ICON_SIDE: u16 = 256;
const EXECUTABLE_ICON_SIDES: [u16; 7] = [16, 24, 32, 48, 64, 128, 256];
const ICO_HEADER_BYTES: u32 = 6;
const ICO_ENTRY_BYTES: u32 = 16;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={SOURCE}");
    let manifest = required_path("CARGO_MANIFEST_DIR")?;
    let out = required_path("OUT_DIR")?;
    let svg = context("read assets/Madoka.svg", fs::read(manifest.join(SOURCE)))?;
    let options = Options::default();
    let tree = context("parse assets/Madoka.svg", Tree::from_data(&svg, &options))?;
    let icon = render(&tree, WINDOW_ICON_SIDE)?;
    context(
        "write icon.rgba",
        fs::write(out.join("icon.rgba"), rgba(&icon)),
    )?;
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_executable_icon(&tree, &out)?;
    }
    Ok(())
}

fn required_path(key: &str) -> Result<PathBuf, String> {
    env::var_os(key)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{key} is unset"))
}

fn context<T>(what: &str, result: Result<T, impl Display>) -> Result<T, String> {
    result.map_err(|error| format!("{what}: {error}"))
}

/// Fits `tree` into a `side` x `side` pixmap, centered, keeping the aspect ratio.
fn render(tree: &Tree, side: u16) -> Result<Pixmap, Box<dyn Error>> {
    let extent = tree.size();
    let edge = f32::from(side);
    let scale = edge / extent.width().max(extent.height());
    let transform = Transform::from_scale(scale, scale).post_translate(
        extent.width().mul_add(-scale, edge) / 2.0,
        extent.height().mul_add(-scale, edge) / 2.0,
    );
    let Some(mut pixmap) = Pixmap::new(u32::from(side), u32::from(side)) else {
        return Err("pixmap with zero side".into());
    };
    resvg::render(tree, transform, &mut pixmap.as_mut());
    Ok(pixmap)
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

fn embed_executable_icon(tree: &Tree, out: &Path) -> Result<(), Box<dyn Error>> {
    let mut images = Vec::new();
    for side in EXECUTABLE_ICON_SIDES {
        images.push((side, render(tree, side)?.encode_png()?));
    }
    let ico = out.join("icon.ico");
    context("write icon.ico", fs::write(&ico, ico_of(&images)?))?;
    let path = ico.display().to_string().replace('\\', "/");
    let rc = out.join("icon.rc");
    context(
        "write icon.rc",
        fs::write(&rc, format!("1 ICON \"{path}\"\n")),
    )?;
    embed_resource::compile(&rc, embed_resource::NONE).manifest_required()?;
    Ok(())
}

/// Packs PNG images into an ICO file. A side of 256 is stored as 0.
fn ico_of(images: &[(u16, Vec<u8>)]) -> Result<Vec<u8>, Box<dyn Error>> {
    let count = u16::try_from(images.len())?;
    let mut ico = vec![0, 0, 1, 0];
    ico.extend(count.to_le_bytes());
    let mut offset = ICO_HEADER_BYTES + ICO_ENTRY_BYTES * u32::from(count);
    for (side, png) in images {
        let dimension = u8::try_from(*side).unwrap_or(0);
        let length = u32::try_from(png.len())?;
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
    Ok(ico)
}
