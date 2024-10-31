use espx_app::{
    self,
    socket::{init_socket_listener_and_stream, unix_socket_loop},
};
use std::sync::{Arc, LazyLock};
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> eframe::Result<()> {
    LazyLock::force(&espx_app::telemetry::TRACING);
    let config = espx_app::config::Config::init();
    tracing::warn!("initializing with config: {config:#?}");
    let state = espx_app::state::SharedState::init(config).unwrap();

    let unix_thread_state = state.clone();
    tokio::spawn(async move {
        let (unix_listener, unix_stream) = init_socket_listener_and_stream().await;
        let unix_stream = Arc::new(RwLock::new(unix_stream));
        unix_socket_loop(unix_stream, unix_listener, unix_thread_state).await
    });

    espx_app::ui::run_gui(state)
}
