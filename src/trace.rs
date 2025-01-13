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

#[derive(Parser, Debug)]
#[clap(name = "waxwing-lsp")]
pub struct LspConfig {
    /// The file to pipe logs out to
    #[clap(short, long)]
    pub file: Option<String>,

    /// The log level to use, defaults to INFO
    /// Valid values are: TRACE, DEBUG, INFO, WARN, ERROR
    #[clap(short, long, default_value = "DEBUG")]
    pub level: String,
}

pub const CLI_TRACING_LOG_FILE: LazyLock<PathBuf> = LazyLock::new(|| {
    let home = std::env::var("HOME").expect("No $HOME env variable");
    let path_str = format!("{home}/.espx/logs.log");
    let path = PathBuf::from_str(&path_str).expect("could not build path buf");
    if !path.exists() {
        std::fs::write(path.clone(), b"").unwrap();
    }
    path
});

pub static CLI_TRACING: LazyLock<()> = LazyLock::new(|| {
    let default_filter_level = "debug".to_string();
    let subscriber_name = "lsp".to_string();

    if let Some(parent) = LazyLock::force(&CLI_TRACING_LOG_FILE).parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .expect("Failed to create parent directory for log file");
        }
    }

    let log_file = File::options()
        .create(true)
        // dangerous!
        .append(true)
        .open(LazyLock::force(&CLI_TRACING_LOG_FILE))
        .expect("Log file could not be created/referenced");

    let sub = get_subscriber(subscriber_name, default_filter_level, Mutex::new(log_file));
    init_subscriber(sub);
});

pub static APP_TRACING: LazyLock<()> = LazyLock::new(|| {
    let default_filter_level = "debug".to_string();
    let subscriber_name = "lsp".to_string();

    let sub = get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
    init_subscriber(sub);
});

pub static RELAY_TRACING: LazyLock<()> = LazyLock::new(|| {
    let config = LspConfig::parse();
    let default_filter_level = "debug".to_string();
    let subscriber_name = "lsp".to_string();

    match &config.file {
        Some(file) => {
            let log_file = File::options()
                .create(true)
                .append(true)
                .open(file)
                .unwrap();
            let sub = get_subscriber(subscriber_name, default_filter_level, Mutex::new(log_file));
            init_subscriber(sub);
        }
        None => {
            //                                          setting this to stdout fixed bunayn issues
            //                                          but it also prevents the lsp from
            //                                          attaching????
            let sub = get_subscriber(subscriber_name, default_filter_level, std::io::stderr);
            init_subscriber(sub);
        }
    };
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
