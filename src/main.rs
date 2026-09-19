fn main() -> iced::Result {
    let paths = match andromeda_client::platform::AppPaths::discover() {
        Ok(paths) => paths,
        Err(error) => {
            eprintln!("Could not determine application directories: {error}");
            std::process::exit(1);
        }
    };

    let _logging = match andromeda_client::telemetry::init(&paths.logs) {
        Ok(guard) => Some(guard),
        Err(error) => {
            eprintln!("File logging is unavailable: {error}");
            None
        }
    };

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "starting Andromeda Client"
    );
    andromeda_client::ui::run(paths)
}
