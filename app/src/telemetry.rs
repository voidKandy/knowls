use clap::Parser;
use std::{
    fs::File,
    path::PathBuf,
    str::FromStr,
    sync::{LazyLock, Mutex},
};
use tracing::{subscriber::set_global_default, Subscriber};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{fmt::MakeWriter, layer::SubscriberExt, EnvFilter, Registry};

// pub static LOG_FILE_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
//     let log_file_dir = PathBuf::from_str("/tmp/espx_tui/").unwrap();
//     if !log_file_dir.exists() {
//         std::fs::create_dir_all(&log_file_dir).unwrap();
//     }
//     let log_file_name = format!("{}.log", env!("CARGO_PKG_NAME"));
//     let log_file_path = log_file_dir.join(log_file_name);
//     log_file_path
// });

pub static TRACING: LazyLock<()> = LazyLock::new(|| {
    let default_filter_level = "debug".to_string();
    let subscriber_name = "lsp".to_string();
    // let log_file_path = LazyLock::force(&LOG_FILE_PATH);

    // let log_file = File::options()
    //     .create(true)
    //     .append(true)
    //     .open(log_file_path)
    //     .unwrap();
    // let sub = get_subscriber(subscriber_name, default_filter_level, Mutex::new(log_file));
    let sub = get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
    init_subscriber(sub);
});

pub fn get_subscriber<Sink>(
    name: String,
    env_filter: String,
    sink: Sink,
) -> impl Subscriber + Send + Sync
where
    Sink: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter));
    let formatting_layer = BunyanFormattingLayer::new(name, sink);

    Registry::default()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(formatting_layer)
}

pub fn init_subscriber(subscriber: impl Subscriber + Send + Sync) {
    LogTracer::init().expect("Failed to set logger");
    set_global_default(subscriber).expect("Failed to set subscriber.");
}
