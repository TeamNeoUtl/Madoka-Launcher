mod app;
mod ops;
mod screen;
mod strings;

use std::process::ExitCode;

use app::App;
use ops::Services;

fn main() -> ExitCode {
    let services = match Services::new() {
        Ok(services) => services,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let result = iced::application(move || App::new(services.clone()), App::update, App::view)
        .title(strings::APP_TITLE)
        .subscription(App::subscription)
        .run();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
