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

#[test]
fn save_then_load_returns_the_same_value() {
    let dir = tempdir().unwrap();
    let config = Config {
        github_token: Some("abc".to_owned()),
        theme: Some("Tokyo Night".to_owned()),
        font: Some("Noto Sans".to_owned()),
        locale: Some("ja".to_owned()),
        dns: vec!["94.140.14.14".parse().unwrap()],
        hide_deprecated_notice: true,
    };
    config.save(dir.path()).unwrap();
    assert_eq!(Config::load(dir.path()).unwrap(), config);
}

#[test]
fn save_creates_the_directory_and_omits_unset_values() {
    let dir = tempdir().unwrap();
    let root = dir.path().join("nested");
    Config::default().save(&root).unwrap();
    assert_eq!(
        fs::read_to_string(root.join("config.toml")).unwrap().trim(),
        ""
    );
    assert!(!root.join("config.toml.tmp").exists());
}

#[test]
fn updating_one_value_keeps_the_others() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("config.toml"), "github_token = \"abc\"\n").unwrap();
    let mut config = Config::load(dir.path()).unwrap();
    config.theme = Some("Dark".to_owned());
    config.save(dir.path()).unwrap();

    let reloaded = Config::load(dir.path()).unwrap();
    assert_eq!(reloaded.github_token.as_deref(), Some("abc"));
    assert_eq!(reloaded.theme.as_deref(), Some("Dark"));
}

#[test]
fn deprecated_notice_is_shown_by_default_and_omitted_when_shown() {
    let dir = tempdir().unwrap();
    assert!(!Config::load(dir.path()).unwrap().hide_deprecated_notice);
    Config::default().save(dir.path()).unwrap();
    let text = fs::read_to_string(dir.path().join("config.toml")).unwrap();
    assert!(!text.contains("hide_deprecated_notice"));
}
