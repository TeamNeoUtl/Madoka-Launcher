//! Copies `WebView2Loader.dll` next to the executables when the target links it dynamically.
//! `webview2-com-sys` links `WebView2LoaderStatic` for `msvc` and `WebView2Loader.dll` for other
//! Windows targets. The crate leaves the DLL in its `OUT_DIR`, so the loader fails to start
//! `Madoka-Browser.exe` with status 0xc0000135 without this copy.

use std::env;
use std::fs;
use std::path::PathBuf;

const LOADER: &str = "WebView2Loader.dll";
const SOURCE: &str = "redist/x64/WebView2Loader.dll";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={SOURCE}");
    let target = |key| env::var(key).unwrap_or_default();
    if target("CARGO_CFG_TARGET_OS") != "windows" || target("CARGO_CFG_TARGET_ENV") == "msvc" {
        return;
    }
    assert_eq!(
        target("CARGO_CFG_TARGET_ARCH"),
        "x86_64",
        "{SOURCE} supports x86_64 only"
    );
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    // OUT_DIR is `<profile dir>/build/<package>-<hash>/out`.
    let profile = out.ancestors().nth(3).expect("profile dir above OUT_DIR");
    fs::copy(manifest.join(SOURCE), profile.join(LOADER)).expect("copy WebView2Loader.dll");
}
