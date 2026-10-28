use std::path::PathBuf;

use directories::ProjectDirs;

pub fn default_root() -> Option<PathBuf> {
    ProjectDirs::from("", "", "MLAYN-Launcher").map(|dirs| dirs.data_dir().to_path_buf())
}
