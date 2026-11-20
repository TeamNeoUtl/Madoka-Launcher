use std::path::Path;
use std::process::{Command, Stdio};

use tracing::{info, warn};

use crate::Error;

const VERIFY_ARGS: [&str; 3] = ["--verify", "--deep", "--strict"];
/// `-s -` selects the ad-hoc identity.
const AD_HOC_ARGS: [&str; 4] = ["--force", "--deep", "-s", "-"];

/// Leaves `bundle` with a valid code signature.
/// A bundle that fails verification gets an ad-hoc signature, because Apple Silicon
/// kills unsigned code. The quarantine attribute is not touched: files that the launcher
/// extracts itself do not carry it.
///
/// # Errors
///
/// Returns `Error::Io` when `codesign` cannot start and `Error::CodeSignFailed`
/// when the ad-hoc signature fails.
pub fn ensure_signed(bundle: &Path) -> Result<(), Error> {
    if run(&VERIFY_ARGS, bundle)? {
        info!(bundle = %bundle.display(), "signature valid");
        return Ok(());
    }
    warn!(bundle = %bundle.display(), "signature invalid, signing ad hoc");
    if run(&AD_HOC_ARGS, bundle)? {
        Ok(())
    } else {
        Err(Error::CodeSignFailed(bundle.to_owned()))
    }
}

fn run(args: &[&str], bundle: &Path) -> Result<bool, Error> {
    Command::new("codesign")
        .args(args)
        .arg(bundle)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .map_err(|e| Error::io(bundle, e))
}
