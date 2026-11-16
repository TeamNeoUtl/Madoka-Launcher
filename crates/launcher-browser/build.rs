//! Copies `WebView2Loader.dll` next to the executables when the target links it dynamically.
//! `webview2-com-sys` links `WebView2LoaderStatic` for `msvc` and `WebView2Loader.dll` for other
//! Windows targets. The crate leaves the DLL in its `OUT_DIR`, so the loader fails to start
//! `Madoka-Browser.exe` with status 0xc0000135 without this copy.

use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

const LOADER: &str = "WebView2Loader.dll";
const SOURCE: &str = "redist/x64/WebView2Loader.dll";

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={SOURCE}");
    let target = |key| env::var(key).unwrap_or_default();
    if target("CARGO_CFG_TARGET_OS") != "windows" || target("CARGO_CFG_TARGET_ENV") == "msvc" {
        return Ok(());
    }
    assert_eq!(
        target("CARGO_CFG_TARGET_ARCH"),
        "x86_64",
        "{SOURCE} supports x86_64 only"
    );
    let manifest = required_path("CARGO_MANIFEST_DIR")?;
    let out = required_path("OUT_DIR")?;
    // OUT_DIR is `<profile dir>/build/<package>-<hash>/out`.
    let Some(profile) = out.ancestors().nth(3) else {
        return Err("profile directory above OUT_DIR is missing".into());
    };
    fs::copy(manifest.join(SOURCE), profile.join(LOADER))
        .map_err(|error| format!("copy {LOADER}: {error}"))?;
    Ok(())
}

fn required_path(key: &str) -> Result<PathBuf, String> {
    env::var_os(key)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{key} is unset"))
}
