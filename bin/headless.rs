use espx_lsp_server::{
    self,
    config::Config,
    sockets::{
        from_relay_recv_loop, init_serverside_listener_and_stream, start_lsp_relay,
        CLIENTSIDE_RELAY_ADDR, CLI_TRACING, SERVERSIDE_RELAY_ADDR,
    },
    state::SharedState,
    MainResult,
};
use std::sync::{Arc, LazyLock};
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> MainResult<()> {
    LazyLock::force(&CLI_TRACING);
    tracing::warn!("spinning up headless Language Server");
    let config = Config::init_from_global_config().expect("failed to build config");
    tracing::warn!("initializing with config: {config:#?}");
    let state = SharedState::init(config).await.unwrap();
    tracing::warn!("succesfully created shared state instance");

    let unix_thread_state = state.clone();
    tokio::spawn(async move {
        let (unix_listener, unix_stream) =
            init_serverside_listener_and_stream(SERVERSIDE_RELAY_ADDR, CLIENTSIDE_RELAY_ADDR).await;
        let unix_stream = Arc::new(RwLock::new(unix_stream));
        from_relay_recv_loop(unix_stream, unix_listener, unix_thread_state).await
    });

    start_lsp_relay().await
}
