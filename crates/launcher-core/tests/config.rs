use std::fs;

use launcher_core::{Config, Error};
use tempfile::tempdir;

#[test]
fn missing_file_yields_default() {
    let dir = tempdir().unwrap();
    assert_eq!(Config::load(dir.path()).unwrap(), Config::default());
}

#[test]
fn reads_github_token() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("config.toml"), "github_token = \"abc\"\n").unwrap();
    let config = Config::load(dir.path()).unwrap();
    assert_eq!(config.github_token.as_deref(), Some("abc"));
}

#[test]
fn invalid_toml_is_parse_error() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("config.toml"), "github_token = [").unwrap();
    assert!(matches!(Config::load(dir.path()), Err(Error::Parse { .. })));
}
