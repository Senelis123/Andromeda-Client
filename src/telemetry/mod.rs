mod secret;
pub use secret::Secret;

use std::{fs, path::Path};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init(log_dir: &Path) -> Result<WorkerGuard, std::io::Error> {
    fs::create_dir_all(log_dir)?;
    let appender = tracing_appender::rolling::daily(log_dir, "andromeda.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("andromeda_client=info")),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(writer)
                .with_ansi(false),
        )
        .init();
    Ok(guard)
}
