#![cfg_attr(windows, windows_subsystem = "windows")]

//! Web view window. Usage: `Madoka-Browser <url> [--profile <dir>] [--save-dir <dir>]`.
//!
//! `--profile` keeps cookies and storage in `<dir>`, so a login (BOOTH) lasts across launches.
//!
//! Without `--save-dir`, a download that starts in the page is cancelled, the window closes,
//! and stdout receives one line: `download <file name>\t<url>`.
//! The file name is empty when the web view reports none.
//!
//! With `--save-dir`, the web view saves the download into `<dir>` with its own session,
//! so a download that needs the login of the page succeeds. stdout receives
//! `download <file name>\t<url>` at the start and `saved <absolute path>\t<url>` after the file
//! is complete, then the window closes. A failed download writes `failed <url>` to stdout
//! and keeps the window open.
//!
//! A new window request (`window.open`, `target="_blank"`) with an http or https url
//! navigates this window. A failure writes one line to stderr and exits with status 1.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use wry::{NewWindowResponse, WebContext, WebViewBuilder};

const TITLE: &str = "Madoka Browser";
const WIDTH: f64 = 1100.0;
const HEIGHT: f64 = 760.0;
const DOWNLOAD_PREFIX: &str = "download ";
const SAVED_PREFIX: &str = "saved ";
const FAILED_PREFIX: &str = "failed ";
const PROFILE_FLAG: &str = "--profile";
const SAVE_DIR_FLAG: &str = "--save-dir";
const FALLBACK_NAME: &str = "download";

enum Signal {
    Download,
    Navigate(String),
}

struct Options {
    url: String,
    profile: Option<PathBuf>,
    save_dir: Option<PathBuf>,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            let _ = writeln!(std::io::stderr(), "{message}");
            ExitCode::FAILURE
        }
    }
}

fn parse_options() -> Result<Options, String> {
    let usage = "usage: Madoka-Browser <url> [--profile <dir>] [--save-dir <dir>]";
    let mut args = std::env::args().skip(1);
    let url = args.next().ok_or(usage)?;
    let mut options = Options {
        url,
        profile: None,
        save_dir: None,
    };
    while let Some(flag) = args.next() {
        let value = args.next().map(PathBuf::from).ok_or(usage)?;
        match flag.as_str() {
            PROFILE_FLAG => options.profile = Some(value),
            SAVE_DIR_FLAG => options.save_dir = Some(value),
            _ => return Err(usage.to_owned()),
        }
    }
    Ok(options)
}

/// `<dir>/<name>`, or `<dir>/<stem> (<n>)<.ext>` for the smallest `n >= 1` that is free.
fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let name = Path::new(name)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| FALLBACK_NAME.to_owned());
    let first = dir.join(&name);
    if !first.exists() {
        return first;
    }
    let path = Path::new(&name);
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.clone());
    let extension = path
        .extension()
        .map(|extension| format!(".{}", extension.to_string_lossy()))
        .unwrap_or_default();
    (1..)
        .map(|n| dir.join(format!("{stem} ({n}){extension}")))
        .find(|candidate| !candidate.exists())
        .unwrap_or(first)
}

fn emit(line: &str) {
    let mut out = std::io::stdout();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

fn run() -> Result<(), String> {
    let options = parse_options()?;
    if let Some(dir) = &options.save_dir {
        std::fs::create_dir_all(dir)
            .map_err(|error| format!("save dir {}: {error}", dir.display()))?;
    }
    if let Some(dir) = &options.profile {
        std::fs::create_dir_all(dir)
            .map_err(|error| format!("profile {}: {error}", dir.display()))?;
    }
    let event_loop = EventLoopBuilder::<Signal>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let navigate_proxy = proxy.clone();
    let completed_proxy = proxy.clone();
    let window = WindowBuilder::new()
        .with_title(TITLE)
        .with_inner_size(LogicalSize::new(WIDTH, HEIGHT))
        .build(&event_loop)
        .map_err(|error| format!("window: {error}"))?;
    let mut context = WebContext::new(options.profile.clone());
    let save_dir = options.save_dir.clone();
    let saves = save_dir.is_some();
    let builder = WebViewBuilder::new_with_web_context(&mut context)
        .with_url(options.url)
        .with_download_started_handler(move |url, path| {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            emit(&format!("{DOWNLOAD_PREFIX}{name}\t{url}"));
            match &save_dir {
                Some(dir) => {
                    *path = unique_path(dir, &name);
                    true
                }
                None => {
                    let _ = proxy.send_event(Signal::Download);
                    false
                }
            }
        })
        .with_download_completed_handler(move |url, path, success| {
            if !saves {
                return;
            }
            match path.filter(|_| success) {
                Some(path) => {
                    emit(&format!("{SAVED_PREFIX}{}\t{url}", path.display()));
                    let _ = completed_proxy.send_event(Signal::Download);
                }
                None => emit(&format!("{FAILED_PREFIX}{url}")),
            }
        })
        .with_new_window_req_handler(move |url, _| {
            if url.starts_with("https://") || url.starts_with("http://") {
                let _ = navigate_proxy.send_event(Signal::Navigate(url));
            }
            NewWindowResponse::Deny
        });
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    let webview = builder.build(&window);
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        let vbox = window.default_vbox().ok_or("window has no gtk box")?;
        builder.build_gtk(vbox)
    };
    let webview = webview.map_err(|error| format!("web view: {error}"))?;
    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(Signal::Navigate(url)) => {
                let _ = webview.load_url(&url);
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            }
            | Event::UserEvent(Signal::Download) => *flow = ControlFlow::Exit,
            _ => {}
        }
    })
}
