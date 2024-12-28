use crate::test_docs::test_doc_1;

use super::config::test_config;
use espx_lsp_server::{
    handle::buffer_operations::BufferOpChannelHandler, interact::parsing::lexer::Lexer,
    state::SharedState,
};
use std::sync::LazyLock;
use tracing::{info, subscriber::set_global_default, Subscriber};
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{fmt::MakeWriter, layer::SubscriberExt, EnvFilter, Registry};

pub static TEST_TRACING: LazyLock<()> = LazyLock::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "lsp".to_string();

    let sub = get_subscriber(subscriber_name, default_filter_level, std::io::stdout);
    init_subscriber(sub);
    info!("test tracing initialized");
});

pub async fn test_state(database: bool) -> SharedState<'static> {
    SharedState::init(test_config(database).unwrap())
        .await
        .unwrap()
}

pub fn test_buff_op_channel() -> BufferOpChannelHandler {
    BufferOpChannelHandler::new()
}

pub async fn handler_tests_state() -> SharedState<'static> {
    let mut state = test_state(false).await;
    let mut update_state = || {
        let mut w = state.0.try_write().unwrap();
        let (uri, content) = test_doc_1();
        let uri_str = uri.to_string();

        let ext = uri_str
            .rsplit_once('.')
            .expect("uri does not have extension")
            .1;
        let mut lexer = Lexer::new(&content, ext);
        let new_tokens = lexer.lex_input();

        match w.documents.get_mut(&uri) {
            Some(tokens) => {
                *tokens = new_tokens;
            }
            None => {
                w.documents.insert(uri, new_tokens);
            }
        }
        // w.update_docs_from_text(uri, content).unwrap();
    };

    update_state();
    state
}

fn get_subscriber<Sink>(
    name: String,
    env_filter: String,
    sink: Sink,
) -> impl Subscriber + Send + Sync
where
    Sink: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter));
    let formatting_layer = BunyanFormattingLayer::new(name, sink);

    Registry::default()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(formatting_layer)
}

fn init_subscriber(subscriber: impl Subscriber + Send + Sync) {
    LogTracer::init().expect("Failed to set logger");
    set_global_default(subscriber).expect("Failed to set subscriber.");
}
