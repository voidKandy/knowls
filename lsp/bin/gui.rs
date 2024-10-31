use espx_lsp_server::{
    self,
    config::Config,
    server::{init_socket_listener_and_stream, unix_socket_loop, RELAY_TRACING},
    state::SharedState,
    ui::run_gui,
};
use std::sync::{Arc, LazyLock};
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> eframe::Result<()> {
    LazyLock::force(&RELAY_TRACING);
    let config = Config::init();
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
