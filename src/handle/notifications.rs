use super::{
    buffer_operations::{BufferOpChannelHandler, BufferOpChannelSender},
    error::HandleResult,
    BufferOpChannelJoinHandle,
};
use crate::{
    handle::{diagnostics::LspDiagnostic, error::HandleError},
    interact::{parsing::tokens::Token, InteractLspNotification},
    state::SharedState,
};
use anyhow::anyhow;
use lsp_server::Notification;
use lsp_types::{
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    TextDocumentItem,
};
use tracing::{debug, warn};

// #[derive(serde::Deserialize, Debug)]
// pub struct TextDocumentOpen {
//     #[serde(rename = "textDocument")]
//     text_document: TextDocumentItem,
// }

#[tracing::instrument(name = "handle notification", skip_all)]
pub async fn handle_notification(
    noti: Notification,
    state: SharedState<'static>,
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
    mut state: SharedState<'static>,
    sender: BufferOpChannelSender,
) -> HandleResult<()> {
    let text_document_changes: DidChangeTextDocumentParams = serde_json::from_value(noti.params)?;
    let uri = text_document_changes.text_document.uri;
    //BAD!
    let ext = uri.clone().as_str().to_string();
    let ext = ext.rsplit_once('.').unwrap().1;

    let w = state.0.try_write()?;
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
    mut state: SharedState<'static>,
    mut sender: BufferOpChannelSender,
) -> HandleResult<()> {
    let params: DidSaveTextDocumentParams =
        serde_json::from_value::<DidSaveTextDocumentParams>(noti.params)?;
    let text = params
        .text
        .as_ref()
        .ok_or(HandleError::Undefined(anyhow!("No text on didSave noti")))?;
    let uri = &params.text_document.uri;

    let mut w = state.0.try_write()?;
    let doc_tokens = w
        .documents
        .get(&uri)
        .ok_or(anyhow!("document not present"))?
        .clone();

    let notification = Into::<InteractLspNotification>::into(params);
    for cmt_idx in doc_tokens.comment_indices().iter() {
        if let Token::Comment(parsed_comment) =
            doc_tokens.get(*cmt_idx).expect("Should be something here")
        {
            parsed_comment
                .execute_from_lsp_message(
                    &mut w,
                    &mut sender,
                    notification.clone(),
                    &doc_tokens,
                    *cmt_idx,
                )
                .await?;
        }
    }

    // w.update_doc_and_agents_from_text(uri.clone(), text)?;
    // warn!("done updating");

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

    // sender
    //     .send_operation(LspDiagnostic::diagnose_document(uri.clone(), &mut w)?.into())
    //     .await?;
    Ok(())
}

#[allow(non_snake_case)]
#[tracing::instrument(name = "didOpen", skip_all)]
async fn handle_didOpen(
    noti: Notification,
    state: SharedState<'static>,
    sender: BufferOpChannelSender,
) -> HandleResult<()> {
    let text_doc_item = serde_json::from_value::<DidOpenTextDocumentParams>(noti.params)?;
    let text = text_doc_item.text_document.text;
    let uri = text_doc_item.text_document.uri;

    // let mut w = state.0.try_write()?;
    // this causes a crash?
    // w.update_doc_and_agents_from_text(uri.clone(), text)?;
    Ok(())
}
