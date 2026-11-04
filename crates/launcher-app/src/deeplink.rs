//! Deep link (`madoka://`) URL parsing and IPC-based single-instance enforcement.
//!
//! Supported URI formats:
//! - `madoka://package/<id>`           → navigate to the package detail view
//! - `madoka://package/<id>?install=true` → auto-install the package
//! - `madoka://updates`                → jump to the update screen
//! - Legacy `aviutl2-catalog://` is also accepted for compatibility.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tracing::{debug, info, warn};

static IPC_RX: Mutex<Option<std::sync::mpsc::Receiver<String>>> = Mutex::new(None);

/// Initializes the IPC listener and registers the URL scheme with the OS.
pub fn init(root: &Path) {
    register_url_scheme();
    if let (Some(rx), Ok(mut lock)) = (start_ipc_listener(root), IPC_RX.lock()) {
        *lock = Some(rx);
    }
}

/// Checks if a deep link URL has been received via IPC.
pub fn poll_incoming() -> Option<String> {
    let Ok(lock) = IPC_RX.lock() else {
        return None;
    };
    let rx = lock.as_ref()?;
    rx.try_recv().ok()
}

/// A deep link action parsed from a `madoka://` URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeepLinkAction {
    /// Open the detail view for a specific package.
    ViewPackage(String),
    /// Install a specific package into the active instance.
    InstallPackage(String),
    /// Navigate to the update screen.
    ShowUpdates,
}

/// Parse a `madoka://` or `aviutl2-catalog://` URL into an action.
///
/// Returns `None` if the URL does not match any recognized pattern.
pub fn parse(url: &str) -> Option<DeepLinkAction> {
    let stripped = url
        .strip_prefix("madoka://")
        .or_else(|| url.strip_prefix("aviutl2-catalog://"))?;

    let (path, query) = match stripped.split_once('?') {
        Some((p, q)) => (p, q),
        None => (stripped, ""),
    };
    let path = path.trim_end_matches('/');

    if path == "updates" {
        return Some(DeepLinkAction::ShowUpdates);
    }

    if let Some(id) = path.strip_prefix("package/") {
        let id = id.trim_matches('/');
        if id.is_empty() {
            return None;
        }
        let auto_install = query
            .split('&')
            .any(|param| param == "install=true" || param == "install=1");
        return if auto_install {
            Some(DeepLinkAction::InstallPackage(id.to_owned()))
        } else {
            Some(DeepLinkAction::ViewPackage(id.to_owned()))
        };
    }

    None
}

/// Returns the IPC socket path for single-instance enforcement.
pub fn ipc_path(root: &Path) -> PathBuf {
    root.join("madoka.sock")
}

/// Tries to connect to an existing instance via the IPC socket and send the URL.
/// Returns `true` if the message was delivered (the running instance will handle it).
pub fn try_send_to_running(root: &Path, url: &str) -> bool {
    let path = ipc_path(root);
    if !path.exists() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::net::UnixStream;
        match UnixStream::connect(&path) {
            Ok(mut stream) => {
                let message = format!("{url}\n");
                if stream.write_all(message.as_bytes()).is_ok() {
                    info!("sent deep link to running instance: {url}");
                    return true;
                }
                warn!("failed to write to IPC socket");
                false
            }
            Err(_) => {
                // Socket file exists but no one is listening — stale.
                debug!("stale IPC socket, ignoring");
                false
            }
        }
    }
    #[cfg(windows)]
    {
        // On Windows, use a named pipe instead of a Unix socket.
        let pipe_name = r"\\.\pipe\madoka-launcher-ipc";
        match std::fs::OpenOptions::new().write(true).open(pipe_name) {
            Ok(mut pipe) => {
                let message = format!("{url}\n");
                if pipe.write_all(message.as_bytes()).is_ok() {
                    info!("sent deep link to running instance via named pipe: {url}");
                    return true;
                }
                warn!("failed to write to named pipe");
                false
            }
            Err(_) => false,
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (root, url);
        false
    }
}

/// Starts the IPC server that listens for incoming deep link URLs.
/// The returned receiver yields parsed URLs sent by new process instances.
///
/// On Unix this creates a Unix domain socket at `<root>/madoka.sock`.
/// On Windows this uses a named pipe.
#[cfg(unix)]
pub fn start_ipc_listener(root: &Path) -> Option<std::sync::mpsc::Receiver<String>> {
    use std::os::unix::net::UnixListener;

    let path = ipc_path(root);
    // Remove stale socket file if it exists.
    let _ = std::fs::remove_file(&path);

    let listener = match UnixListener::bind(&path) {
        Ok(listener) => listener,
        Err(err) => {
            warn!("failed to create IPC socket {}: {err}", path.display());
            return None;
        }
    };
    info!("IPC listener started at {}", path.display());

    let (tx, rx) = std::sync::mpsc::channel();
    let socket_path = path.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let reader = BufReader::new(stream);
                    for line in reader.lines().map_while(Result::ok) {
                        let trimmed = line.trim().to_owned();
                        if !trimmed.is_empty() {
                            info!("IPC received: {trimmed}");
                            let _ = tx.send(trimmed);
                        }
                    }
                }
                Err(err) => {
                    warn!("IPC accept error: {err}");
                }
            }
        }
        // Clean up when the listener thread exits.
        let _ = std::fs::remove_file(&socket_path);
    });
    Some(rx)
}

#[cfg(windows)]
pub fn start_ipc_listener(_root: &Path) -> Option<std::sync::mpsc::Receiver<String>> {
    // Named-pipe listener for Windows — simplified implementation.
    // Full implementation would use `windows-sys` for proper named pipe server.
    // For now we accept URLs via CLI args on the second launch.
    None
}

#[cfg(not(any(unix, windows)))]
pub fn start_ipc_listener(_root: &Path) -> Option<std::sync::mpsc::Receiver<String>> {
    None
}

/// Extracts a `madoka://` or `aviutl2-catalog://` URL from CLI arguments.
pub fn url_from_args() -> Option<String> {
    std::env::args()
        .skip(1)
        .find(|arg| arg.starts_with("madoka://") || arg.starts_with("aviutl2-catalog://"))
}

/// Registers the `madoka://` URL scheme handler with the OS.
///
/// - **Linux**: Creates/updates a `.desktop` file in `~/.local/share/applications/`.
/// - **Windows**: Registers `HKCU\Software\Classes\madoka` with URL Protocol.
pub fn register_url_scheme() {
    #[cfg(target_os = "linux")]
    {
        if let Ok(exe) = std::env::current_exe() {
            let data_dir = std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var_os("HOME")
                        .map(|home| PathBuf::from(home).join(".local").join("share"))
                })
                .unwrap_or_else(|| PathBuf::from("/tmp"));
            let app_dir = data_dir.join("applications");
            let _ = std::fs::create_dir_all(&app_dir);
            let desktop_path = app_dir.join("madoka-launcher.desktop");
            let content = format!(
                "[Desktop Entry]\n\
                 Type=Application\n\
                 Name=Madoka Launcher\n\
                 Exec={} %u\n\
                 Terminal=false\n\
                 MimeType=x-scheme-handler/madoka;x-scheme-handler/aviutl2-catalog;\n\
                 NoDisplay=true\n",
                exe.display()
            );
            match std::fs::write(&desktop_path, content) {
                Ok(()) => info!(
                    "registered URL scheme handler at {}",
                    desktop_path.display()
                ),
                Err(err) => warn!("failed to register URL scheme: {err}"),
            }
            // Attempt to run xdg-mime to set the default handler
            let _ = std::process::Command::new("xdg-mime")
                .args([
                    "default",
                    "madoka-launcher.desktop",
                    "x-scheme-handler/madoka",
                ])
                .status();
            let _ = std::process::Command::new("xdg-mime")
                .args([
                    "default",
                    "madoka-launcher.desktop",
                    "x-scheme-handler/aviutl2-catalog",
                ])
                .status();
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(exe) = std::env::current_exe() {
            let exe_str = exe.to_string_lossy().into_owned();
            // Register madoka:// protocol
            let _ = register_windows_protocol("madoka", &exe_str);
            let _ = register_windows_protocol("aviutl2-catalog", &exe_str);
        }
    }
}

#[cfg(target_os = "windows")]
fn register_windows_protocol(protocol: &str, exe_path: &str) -> std::io::Result<()> {
    use std::process::Command;
    // Use reg.exe to add registry keys for the URL protocol.
    let key = format!(r"HKCU\Software\Classes\{protocol}");
    Command::new("reg")
        .args([
            "add",
            &key,
            "/ve",
            "/d",
            &format!("URL:{protocol} Protocol"),
            "/f",
        ])
        .status()?;
    Command::new("reg")
        .args(["add", &key, "/v", "URL Protocol", "/d", "", "/f"])
        .status()?;
    let cmd_key = format!(r"{key}\shell\open\command");
    Command::new("reg")
        .args([
            "add",
            &cmd_key,
            "/ve",
            "/d",
            &format!(r#""{exe_path}" "%1""#),
            "/f",
        ])
        .status()?;
    info!("registered Windows URL protocol: {protocol}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_view_package() {
        assert_eq!(
            parse("madoka://package/rigaya.x264guiEx"),
            Some(DeepLinkAction::ViewPackage("rigaya.x264guiEx".to_owned()))
        );
    }

    #[test]
    fn parse_install_package() {
        assert_eq!(
            parse("madoka://package/rigaya.x264guiEx?install=true"),
            Some(DeepLinkAction::InstallPackage(
                "rigaya.x264guiEx".to_owned()
            ))
        );
    }

    #[test]
    fn parse_updates() {
        assert_eq!(parse("madoka://updates"), Some(DeepLinkAction::ShowUpdates));
    }

    #[test]
    fn parse_legacy_scheme() {
        assert_eq!(
            parse("aviutl2-catalog://package/test.plugin"),
            Some(DeepLinkAction::ViewPackage("test.plugin".to_owned()))
        );
    }

    #[test]
    fn parse_invalid() {
        assert_eq!(parse("https://example.com"), None);
        assert_eq!(parse("madoka://"), None);
        assert_eq!(parse("madoka://unknown/path"), None);
    }
}
