mod app;
mod deeplink;
mod i18n;
mod logging;
mod ops;
mod screen;
mod strings;
mod style;

use std::process::ExitCode;

use app::App;
use iced::window;
use launcher_core::default_root;
use ops::Services;
use tracing::{error, info, warn};

const ICON_RGBA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/icon.rgba"));
#[cfg(target_os = "linux")]
const APPLICATION_ID: &str = "Madoka-Launcher";

fn main() -> ExitCode {
    let Some(root) = default_root() else {
        eprintln!("data directory is unavailable");
        return ExitCode::FAILURE;
    };

    // If launched with a deep link URL, try sending it to an already running instance.
    if let Some(url) =
        deeplink::url_from_args().filter(|url| deeplink::try_send_to_running(&root, url))
    {
        info!("forwarded URL to running instance, exiting: {url}");
        return ExitCode::SUCCESS;
    }

    // Initialize IPC listener and OS scheme handler
    deeplink::init(&root);

    let log = logging::init(&root);
    info!(
        version = env!("CARGO_PKG_VERSION"),
        root = %root.display(),
        log = %log.display(),
        "start"
    );
    let services = match Services::new(root) {
        Ok(services) => services,
        Err(error) => {
            error!(%error, "initialization failed");
            return ExitCode::FAILURE;
        }
    };
    let font = style::font_of(services.font_name());
    let result = iced::application(move || App::new(services.clone()), App::update, App::view)
        .title(strings::APP_TITLE)
        .window(window_settings())
        .font(style::ICON_FONT_BYTES)
        .default_font(font)
        .theme(App::theme)
        .subscription(App::subscription)
        .run();
    match result {
        Ok(()) => {
            info!("exit");
            ExitCode::SUCCESS
        }
        Err(error) => {
            error!(%error, "application failed");
            ExitCode::FAILURE
        }
    }
}

fn window_settings() -> window::Settings {
    let side = (ICON_RGBA.len() / 4).isqrt() as u32;
    let icon = window::icon::from_rgba(ICON_RGBA.to_vec(), side, side)
        .inspect_err(|error| warn!(%error, "window icon"))
        .ok();
    window::Settings {
        icon,
        #[cfg(target_os = "linux")]
        platform_specific: window::settings::PlatformSpecific {
            application_id: APPLICATION_ID.to_owned(),
            ..Default::default()
        },
        ..Default::default()
    }
}
