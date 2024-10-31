use clap::Parser;
use espx_lsp_server::{
    self,
    config::Config,
    server::{init_socket_listener_and_stream, unix_socket_loop},
    state::SharedState,
    telemetry::TRACING,
    ui::run_gui,
};
use std::sync::{Arc, LazyLock};
use tokio::sync::RwLock;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short = 'c', long)]
    config_file: String,
}

#[tokio::main]
async fn main() -> eframe::Result<()> {
    LazyLock::force(&TRACING);
    let args = Args::parse();
    println!("{args:?}");
    let config = Config::init_from_file_path(&args.config_file)
        .expect("could not get config from given path");
    tracing::warn!("initializing with config: {config:#?}");
    let state = SharedState::init(config).unwrap();

    let unix_thread_state = state.clone();
    tokio::spawn(async move {
        let (unix_listener, unix_stream) = init_socket_listener_and_stream().await;
        let unix_stream = Arc::new(RwLock::new(unix_stream));
        unix_socket_loop(unix_stream, unix_listener, unix_thread_state).await
    });

    run_gui(state)
}
