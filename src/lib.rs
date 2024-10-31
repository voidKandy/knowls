// use std::sync::LazyLock;
// mod relay;

// use espx_lsp_server::start_lsp;
// use telemetry::TRACING;
// use tracing_log::log::info;

// fn main() -> anyhow::Result<()> {
//     LazyLock::force(&TRACING);
//     info!("Tracing Initialized");
//     start_lsp()?;
//     Ok(())
// }
