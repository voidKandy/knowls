use super::{
    buffer_operations::{BufferOpChannelHandler, BufferOpChannelSender},
    error::HandleResult,
    BufferOpChannelJoinHandle,
};
use crate::{
    handle::{diagnostics::LspDiagnostic, error::HandleError},
    state::SharedState,
};
use anyhow::anyhow;
use lsp_server::Notification;
use lsp_types::{DidChangeTextDocumentParams, DidSaveTextDocumentParams, TextDocumentItem};
use tracing::{debug, warn};

#[derive(serde::Deserialize, Debug)]
struct TextDocumentOpen {
    #[serde(rename = "textDocument")]
    text_document: TextDocumentItem,
}

#[tracing::instrument(name = "handle notification", skip_all)]
pub async fn handle_notification(
    noti: Notification,
    state: SharedState,
) -> HandleResult<BufferOpChannelHandler> {
    let handle = BufferOpChannelHandler::new();

    let mut task_sender = handle.sender.clone();
    let _: BufferOpChannelJoinHandle = tokio::spawn(async move {
        let method = noti.method.clone();
        match match method.as_str() {
            "textDocument/didChange" => handle_didChange(noti, state, task_sender.clone()).await,
            "textDocument/didSave" => handle_didSave(noti, state, task_sender.clone()).await,
            "textDocument/didOpen" => handle_didOpen(noti, state, task_sender.clone()).await,
            s => {
                debug!("unhandled notification: {:?}", s);
                Ok(())
            }
        } {
            Ok(_) => {
                task_sender
                    .send_finish()
                    .await
                    .map_err(|err| HandleError::from(err))?;
                Ok(())
            }
            Err(err) => {
                err.notification_err(&mut task_sender).await?;
                Ok(())
            }
        }
    });
    return Ok(handle);
}

#[allow(non_snake_case)]
#[tracing::instrument(name = "didChange", skip_all)]
async fn handle_didChange(
    noti: Notification,
    mut state: SharedState,
    mut sender: BufferOpChannelSender,
) -> HandleResult<()> {
    let text_document_changes: DidChangeTextDocumentParams = serde_json::from_value(noti.params)?;
    let uri = text_document_changes.text_document.uri;
    //BAD!
    let ext = uri.clone().as_str().to_string();
    let ext = ext.rsplit_once('.').unwrap().1;

    let mut w = state.get_write()?;
    if text_document_changes.content_changes.len() > 1 {
        warn!("more than a single change recieved in notification");
    }

    // sender
    //     .send_operation(LspDiagnostic::diagnose_document(uri, &mut w.store)?.into())
    //     .await?;
    Ok(())
}

#[allow(non_snake_case)]
#[tracing::instrument(name = "didSave", skip_all)]
pub async fn handle_didSave(
    noti: Notification,
    mut state: SharedState,
    mut sender: BufferOpChannelSender,
) -> HandleResult<()> {
    let saved_text_doc: DidSaveTextDocumentParams =
        serde_json::from_value::<DidSaveTextDocumentParams>(noti.params)?;
    let text = saved_text_doc
        .text
        .ok_or(HandleError::Undefined(anyhow!("No text on didSave noti")))?;
    let uri = saved_text_doc.text_document.uri;

    let mut w = state.get_write()?;
    warn!("updating");
    w.update_doc_and_agents_from_text(uri.clone(), text)?;
    warn!("done updating");

    if w.database.is_some() {
        w.save_docs_to_database().await?;
        w.save_agent_memories_to_database().await?;
    }

    // let role = MessageRole::Other {
    //     alias: uri.to_string(),
    //     coerce_to: OtherRoleTo::User,
    // };
    // agent.cache.mut_filter_by(&role, false);

    sender
        .send_work_done_report(Some("Updated Document Tokens"), None)
        .await?;

    sender
        .send_operation(LspDiagnostic::diagnose_document(uri, &mut w)?.into())
        .await?;
    Ok(())
}

#[allow(non_snake_case)]
#[tracing::instrument(name = "didOpen", skip_all)]
async fn handle_didOpen(
    noti: Notification,
    mut state: SharedState,
    mut sender: BufferOpChannelSender,
) -> HandleResult<()> {
    let text_doc_item = serde_json::from_value::<TextDocumentOpen>(noti.params)?;
    let text = text_doc_item.text_document.text;
    let uri = text_doc_item.text_document.uri;

    // let mut w = state.get_write()?;
    // this causes a crash?
    // w.update_doc_and_agents_from_text(uri.clone(), text)?;
    Ok(())
}
