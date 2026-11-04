use std::path::PathBuf;

use directories::ProjectDirs;

#[must_use]
pub fn default_root() -> Option<PathBuf> {
    // 11-37: Portable mode check - if "data" directory exists beside executable or in cwd
    if let Some(parent) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
    {
        let portable_dir = parent.join("data");
        if portable_dir.is_dir() {
            return Some(portable_dir);
        }
    }
    let local_data = PathBuf::from("data");
    if local_data.is_dir() {
        return Some(local_data);
    }

    ProjectDirs::from("", "", "Madoka-Launcher").map(|dirs| dirs.data_dir().to_path_buf())
}
