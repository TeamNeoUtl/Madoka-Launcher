use std::path::PathBuf;

use directories::ProjectDirs;

#[must_use]
pub fn default_root() -> Option<PathBuf> {
    ProjectDirs::from("", "", "Motoyori-Launcher").map(|dirs| dirs.data_dir().to_path_buf())
}
