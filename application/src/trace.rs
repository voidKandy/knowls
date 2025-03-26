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

pub const TRACING_LOG_FILE: LazyLock<PathBuf> = LazyLock::new(|| {
    let home = std::env::var("HOME").expect("No $HOME env variable");
    let path_str = format!("{home}/.knowls/logs.log");
    let path = PathBuf::from_str(&path_str).expect("could not build path buf");
    if !path.parent().unwrap().exists() {
        std::fs::create_dir(path.parent().unwrap()).expect("failed to create .knowls dir");
    }
    if !path.exists() {
        std::fs::write(path.clone(), b"").unwrap();
    }
    path
});

pub static TRACING: LazyLock<()> = LazyLock::new(|| {
    let default_filter_level = "debug".to_string();
    let subscriber_name = "lsp".to_string();

    if let Some(parent) = LazyLock::force(&TRACING_LOG_FILE).parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .expect("Failed to create parent directory for log file");
        }
    }

    let log_file = File::options()
        .create(true)
        // dangerous!
        .append(true)
        .open(LazyLock::force(&TRACING_LOG_FILE))
        .expect("Log file could not be created/referenced");

    let sub = get_subscriber(subscriber_name, default_filter_level, Mutex::new(log_file));
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
