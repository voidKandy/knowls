use espx_lsp_server::{
    self,
    config::Config,
    sockets::{
        relay::{RelayClient, RelayServer},
        SocketGuard, CLI_TRACING,
    },
    state::SharedState,
};
use std::sync::LazyLock;

#[tokio::main]
async fn main() {
    LazyLock::force(&CLI_TRACING);
    std::panic::set_hook(Box::new(|v| {
        tracing::error!(
            "thread panicked at: {:#?}\npayload: {:#?}",
            v.location(),
            v.payload()
                .downcast_ref::<Box<dyn ToString>>()
                .map(|v| v.to_string())
        )
    }));
    tracing::warn!("spinning up headless Language Server");
    let config = Config::init_from_global_config().expect("failed to build config");
    tracing::warn!("initializing with config: {config:#?}");
    let state = SharedState::init(config).await.unwrap();
    tracing::warn!("succesfully created shared state instance");

    let guard = SocketGuard::from("/tmp/espx_relay.sock");

    match RelayClient::new(&guard).await {
        Ok(relay_client) => relay_client.main_loop().await.unwrap(),
        Err(_) => {
            tracing::warn!("server is not running, need to spin up");
            let server = RelayServer::new(&guard, state).await;
            tokio::spawn(async move { server.main_loop().await });
            let client = RelayClient::new(&guard)
                .await
                .expect("should not fail to start relay client");
            client.main_loop().await.unwrap()
        }
    }
}
