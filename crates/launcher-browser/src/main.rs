#![cfg_attr(windows, windows_subsystem = "windows")]

//! Web view window. Usage: `Motoyori-Browser <url>`.
//! A download that starts in the page is cancelled, the window closes, and stdout receives
//! one line: `download <file name>\t<url>`. The file name is empty when the web view reports none.
//! A new window request (`window.open`, `target="_blank"`) with an http or https url navigates this window.
//! A failure writes one line to stderr and exits with status 1.

use std::io::Write;
use std::process::ExitCode;

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use wry::{NewWindowResponse, WebViewBuilder};

const TITLE: &str = "Motoyori Browser";
const WIDTH: f64 = 1100.0;
const HEIGHT: f64 = 760.0;
const DOWNLOAD_PREFIX: &str = "download ";

enum Signal {
    Download,
    Navigate(String),
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

fn run() -> Result<(), String> {
    let url = std::env::args()
        .nth(1)
        .ok_or("usage: Motoyori-Browser <url>")?;
    let event_loop = EventLoopBuilder::<Signal>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let navigate_proxy = proxy.clone();
    let window = WindowBuilder::new()
        .with_title(TITLE)
        .with_inner_size(LogicalSize::new(WIDTH, HEIGHT))
        .build(&event_loop)
        .map_err(|error| format!("window: {error}"))?;
    let builder = WebViewBuilder::new()
        .with_url(url)
        .with_download_started_handler(move |url, path| {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut out = std::io::stdout();
            let _ = writeln!(out, "{DOWNLOAD_PREFIX}{name}\t{url}");
            let _ = out.flush();
            let _ = proxy.send_event(Signal::Download);
            false
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
