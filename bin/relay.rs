use espx_lsp_server::sockets::{start_lsp_relay, RELAY_TRACING};
use std::sync::LazyLock;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    LazyLock::force(&RELAY_TRACING);
    start_lsp_relay().await
}
