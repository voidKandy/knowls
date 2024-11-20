use espx_lsp_server::{
    self,
    config::Config,
    sockets::{
        from_relay_recv_loop, init_serverside_listener_and_stream, start_lsp_relay,
        CLIENTSIDE_RELAY_ADDR, RELAY_TRACING, SERVERSIDE_RELAY_ADDR,
    },
    state::SharedState,
};
use std::sync::{Arc, LazyLock};
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    LazyLock::force(&RELAY_TRACING);
    let config = Config::init_from_pwd();
    tracing::warn!("initializing with config: {config:#?}");
    let state = SharedState::init(config).await.unwrap();

    let unix_thread_state = state.clone();
    tokio::spawn(async move {
        let (unix_listener, unix_stream) =
            init_serverside_listener_and_stream(SERVERSIDE_RELAY_ADDR, CLIENTSIDE_RELAY_ADDR).await;
        let unix_stream = Arc::new(RwLock::new(unix_stream));
        from_relay_recv_loop(unix_stream, unix_listener, unix_thread_state).await
    });

    start_lsp_relay().await
}
