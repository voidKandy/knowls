use crate::{
    helpers::{handler_tests_state, test_buff_op_channel, TEST_TRACING},
    test_docs::test_doc_1,
};
use espx_app::{agents::AgentID, handle::notifications::handle_didSave};
use lsp_types::{DidSaveTextDocumentParams, TextDocumentIdentifier, Uri};
use serde::Serialize;
use std::sync::LazyLock;
use tracing::warn;

pub fn into_lsp_notification<P: Serialize>(params: P, method: &str) -> lsp_server::Notification {
    let params = serde_json::to_value(params).expect("could not serialize");
    lsp_server::Notification {
        method: method.to_string(),
        params,
    }
}

fn create_didsave_params(uri: Uri, text: Option<String>) -> DidSaveTextDocumentParams {
    DidSaveTextDocumentParams {
        text_document: TextDocumentIdentifier { uri },
        text,
    }
}

#[tokio::test]
async fn handles_didsave_correctly() {
    LazyLock::force(&TEST_TRACING);
    let (uri, text) = test_doc_1();
    let state = handler_tests_state().await;

    let r = state.get_read().unwrap();

    let agent_cache_before = r
        .agents
        .as_ref()
        .unwrap()
        .get_agent_ref(AgentID::Global)
        .unwrap()
        .cache
        .clone();
    drop(r);

    let buffer_op_channel = test_buff_op_channel();

    let params = create_didsave_params(uri, Some(text));
    let noti = into_lsp_notification(params, "textDocument/didSave");

    handle_didSave(noti, state.clone(), buffer_op_channel.sender.clone())
        .await
        .unwrap();

    let r = state.get_read().unwrap();
    let agent_cache_len_after = r
        .agents
        .as_ref()
        .unwrap()
        .get_agent_ref(AgentID::Global)
        .unwrap()
        .cache
        .len();
    drop(r);

    assert_eq!(agent_cache_before.len() + 1, agent_cache_len_after);

    let (uri, _) = test_doc_1();
    let text = String::new();
    let params = create_didsave_params(uri, Some(text));
    let noti = into_lsp_notification(params, "textDocument/didSave");

    handle_didSave(noti, state.clone(), buffer_op_channel.sender)
        .await
        .unwrap();
    let r = state.get_read().unwrap();

    warn!("agent cache before: {agent_cache_before:#?}",);
    let agent_cache_after = r
        .agents
        .as_ref()
        .unwrap()
        .get_agent_ref(AgentID::Global)
        .unwrap()
        .cache
        .clone();
    warn!("agent cache after: {agent_cache_after:#?}",);
    assert_eq!(agent_cache_after.len(), agent_cache_before.len());
}
