//! Deep link (`madoka://`) URL parsing and IPC-based single-instance enforcement.
//!
//! Supported URI formats:
//! - `madoka://package/<id>`           → navigate to the package detail view
//! - `madoka://package/<id>?install=true` → auto-install the package
//! - `madoka://updates`                → jump to the update screen
//! - Legacy `aviutl2-catalog://` is also accepted for compatibility.

use std::io::{BufRead, BufReader, Write};
use std::sync::Mutex;

use interprocess::local_socket::{
    GenericFilePath, GenericNamespaced, ListenerOptions, Name, NameType, Stream, prelude::*,
};
use tracing::{debug, info, warn};

/// Name of the local socket. One launcher process owns it.
/// A named pipe on Windows, an abstract socket on Linux,
/// and a socket file in the temporary directory where namespaced names are unsupported (macOS).
const IPC_NAME: &str = "madoka-launcher.ipc";

fn ipc_socket_path() -> std::path::PathBuf {
    std::env::temp_dir().join(IPC_NAME)
}

fn ipc_name() -> std::io::Result<Name<'static>> {
    if GenericNamespaced::is_supported() {
        IPC_NAME.to_ns_name::<GenericNamespaced>()
    } else {
        ipc_socket_path().to_fs_name::<GenericFilePath>()
    }
}

static IPC_RX: Mutex<Option<std::sync::mpsc::Receiver<String>>> = Mutex::new(None);

/// Initializes the IPC listener and registers the URL scheme with the OS.
pub fn init() {
    register_url_scheme();
    if let (Some(rx), Ok(mut lock)) = (start_ipc_listener(), IPC_RX.lock()) {
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

/// Sends `url` to the running launcher process.
/// Returns `true` when the message was delivered and the running process handles it.
pub fn try_send_to_running(url: &str) -> bool {
    let Ok(name) = ipc_name() else {
        warn!("local socket name is unsupported");
        return false;
    };
    let mut stream = match Stream::connect(name) {
        Ok(stream) => stream,
        Err(err) => {
            debug!("no running instance: {err}");
            return false;
        }
    };
    let delivered = stream
        .write_all(format!("{url}\n").as_bytes())
        .and_then(|()| stream.flush());
    match delivered {
        Ok(()) => {
            info!("sent deep link to running instance: {url}");
            true
        }
        Err(err) => {
            warn!("failed to write to IPC socket: {err}");
            false
        }
    }
}

/// Starts the IPC server that listens for deep link URLs sent by new processes.
/// Returns `None` when another process owns the socket or the OS rejects the name.
pub fn start_ipc_listener() -> Option<std::sync::mpsc::Receiver<String>> {
    let name = ipc_name()
        .inspect_err(|err| warn!("invalid IPC name: {err}"))
        .ok()?;
    if !GenericNamespaced::is_supported() && Stream::connect(name.clone()).is_err() {
        // No process listens on the socket file: it is left over from a crashed process.
        let _ = std::fs::remove_file(ipc_socket_path());
    }
    let listener = ListenerOptions::new()
        .name(name)
        .create_sync()
        .inspect_err(|err| warn!("failed to create IPC listener: {err}"))
        .ok()?;
    info!("IPC listener started");

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for connection in listener.incoming() {
            match connection {
                Ok(stream) => {
                    for line in BufReader::new(stream).lines().map_while(Result::ok) {
                        let trimmed = line.trim().to_owned();
                        if trimmed.is_empty() {
                            continue;
                        }
                        info!("IPC received: {trimmed}");
                        if tx.send(trimmed).is_err() {
                            return;
                        }
                    }
                }
                Err(err) => warn!("IPC accept error: {err}"),
            }
        }
    });
    Some(rx)
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
                .map(std::path::PathBuf::from)
                .or_else(|| {
                    std::env::var_os("HOME")
                        .map(|home| std::path::PathBuf::from(home).join(".local").join("share"))
                })
                .unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
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
