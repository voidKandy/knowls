use std::sync::LazyLock;

use espx_lsp_server::start_lsp;
use telemetry::TRACING;
use tracing_log::log::info;
mod telemetry;

fn main() -> anyhow::Result<()> {
    LazyLock::force(&TRACING);
    info!("Tracing Initialized");
    start_lsp()?;
    Ok(())
}
