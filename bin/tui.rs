#![allow(unused, dead_code)]
use clap::Parser;
use espx_lsp_server::{
    config::Config,
    sockets::{
        init_serverside_listener_and_stream, CLIENTSIDE_RELAY_ADDR, CLI_TRACING,
        CLI_TRACING_LOG_FILE, SERVERSIDE_RELAY_ADDR,
    },
    state::SharedState,
};
use std::sync::{Arc, LazyLock};
use tokio::sync::RwLock;

#[cfg(feature = "tui")]
use espx_lsp_server::ui::tui::{
    cli::{CliArgs, CliCommand},
    Tui,
};
#[tokio::main]
#[cfg(feature = "tui")]
async fn main() {
    let args = CliArgs::parse();
    if let CliCommand::Logs { clear } = args.command {
        let log_file_content =
            std::fs::read_to_string(LazyLock::force(&CLI_TRACING_LOG_FILE)).unwrap();
        if clear {
            std::fs::write(LazyLock::force(&CLI_TRACING_LOG_FILE), b"").unwrap();
        }
        println!("{log_file_content}");
        return;
    }

    LazyLock::force(&CLI_TRACING);
    color_eyre::install().expect("failed to prepare color_eyre");
    let terminal = ratatui::init();
    let config = Config::init_from_global_config().expect("could not get config from given path");
    tracing::warn!("initializing with config: {config:#?}");
    let state = SharedState::init(config).await.unwrap();
    let lsp_thread_state = state.clone();

    // LSP relay connection
    tokio::spawn(async move {
        let (unix_listener, unix_stream) =
            init_serverside_listener_and_stream(SERVERSIDE_RELAY_ADDR, CLIENTSIDE_RELAY_ADDR).await;
        let unix_stream = Arc::new(RwLock::new(unix_stream));
        from_relay_recv_loop(unix_stream, unix_listener, lsp_thread_state).await
    });

    let app = Tui::new(state).await;
    app.run(terminal).unwrap();
    ratatui::restore();
}

#[cfg(not(feature = "tui"))]
fn main() {
    panic!("you should pass the 'tui' feature flag when running this binary");
}
