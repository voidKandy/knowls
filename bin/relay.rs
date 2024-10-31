use espx_lsp_server::server::{start_lsp, RELAY_TRACING};
use std::sync::LazyLock;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    LazyLock::force(&RELAY_TRACING);
    start_lsp().await
}
