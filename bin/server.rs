use espx_lsp_server::{self, config::Config, scratch::Server, sockets::trace::APP_TRACING};
use std::sync::LazyLock;

#[tokio::main]
async fn main() {
    LazyLock::force(&APP_TRACING);
    std::panic::set_hook(Box::new(|v| {
        let payload = v.payload();
        let str = payload.downcast_ref::<String>().cloned().unwrap_or(
            payload
                .downcast_ref::<&'static str>()
                .map(|str| str.to_string())
                .unwrap_or("Any".to_string()),
        );

        tracing::error!("thread panicked at: {:#?}\npayload: {str:#?}", v.location(),)
    }));

    let config = Config::init_from_global_config().expect("failed to build config");

    let addr = "127.0.0.1:1598";
    let mut server = Server::new(config, addr).await;
    server.main_loop().await
}
