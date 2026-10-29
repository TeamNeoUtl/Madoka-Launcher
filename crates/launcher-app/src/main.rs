mod app;
mod logging;
mod ops;
mod screen;
mod strings;
mod style;

use std::process::ExitCode;

use app::App;
use launcher_core::default_root;
use ops::Services;
use tracing::{error, info};

fn main() -> ExitCode {
    let Some(root) = default_root() else {
        eprintln!("data directory is unavailable");
        return ExitCode::FAILURE;
    };
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
    let result = iced::application(move || App::new(services.clone()), App::update, App::view)
        .title(strings::APP_TITLE)
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
