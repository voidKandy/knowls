use espx_lsp_server::trace::RELAY_TRACING;
use std::sync::LazyLock;

#[tokio::main]
async fn main() {
    LazyLock::force(&RELAY_TRACING);
    // start_lsp_relay().await
}
