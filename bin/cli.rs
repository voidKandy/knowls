use clap::Parser;
use espx_lsp_server::{
    config::Config,
    sockets::{
        from_relay_recv_loop, init_serverside_listener_and_stream, CLIENTSIDE_RELAY_ADDR,
        CLI_TRACING, SERVERSIDE_RELAY_ADDR,
    },
    state::SharedState,
    ui::cli::CliArgs,
};
use std::sync::{Arc, LazyLock};
use tokio::sync::RwLock;
use tracing::warn;

#[tokio::main]
async fn main() {
    LazyLock::force(&CLI_TRACING);
    color_eyre::install().expect("failed to prepare color_eyre");
    let terminal = ratatui::init();
    let args = CliArgs::parse();
    warn!("{args:?}");

    let config = Config::init_from_global_config().expect("could not get config from given path");
    tracing::warn!("initializing with config: {config:#?}");
    let state = SharedState::init(config).unwrap();

    // LSP relay connection
    let lsp_thread_state = state.clone();
    tokio::spawn(async move {
        let (unix_listener, unix_stream) =
            init_serverside_listener_and_stream(SERVERSIDE_RELAY_ADDR, CLIENTSIDE_RELAY_ADDR).await;
        let unix_stream = Arc::new(RwLock::new(unix_stream));
        from_relay_recv_loop(unix_stream, unix_listener, lsp_thread_state).await
    });

    if let Some(app) = args.command.handle(state) {
        warn!("app gotten from command handler");
        app.run(terminal).unwrap();
    }
    ratatui::restore();
}
