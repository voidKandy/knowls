use super::{
    buffer_operations::{BufferOpChannelHandler, BufferOpChannelSender},
    diagnostics::LspDiagnostic,
    show_notification_err, BufferOpChannelJoinHandle,
};
use crate::{
    interact::{execution::InteractDocumentInfo, InteractLspNotification},
    other_err,
    state::SharedState,
    MainResult,
};
use lsp_server::Notification;
use lsp_types::{
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
};
use tracing::{debug, warn};

#[tracing::instrument(name = "handle notification", skip_all)]
pub async fn handle_notification(
    noti: Notification,
    state: SharedState<'static>,
) -> MainResult<BufferOpChannelHandler> {
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
                    .map_err(|err| other_err!("{err:#?}"))?;
                Ok(())
            }
            Err(err) => {
                show_notification_err(&err, &mut task_sender).await?;
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
    state: SharedState<'static>,
    sender: BufferOpChannelSender,
) -> MainResult<()> {
    let text_document_changes: DidChangeTextDocumentParams = serde_json::from_value(noti.params)?;
    let uri = text_document_changes.text_document.uri;
    if text_document_changes.content_changes.len() > 1 {
        warn!("more than a single change recieved in notification");
    }
    let text = &text_document_changes.content_changes.first().unwrap().text;

    let mut w = state.0.try_write()?;

    w.update_doc_and_agents_from_text(uri.clone(), &text)?;

    Ok(())
}

#[allow(non_snake_case)]
#[tracing::instrument(name = "didSave", skip_all)]
pub async fn handle_didSave<'s>(
    noti: Notification,
    state: SharedState<'static>,
    mut sender: BufferOpChannelSender,
) -> MainResult<()> {
    let params: DidSaveTextDocumentParams =
        serde_json::from_value::<DidSaveTextDocumentParams>(noti.params)?;
    let text = params
        .text
        .as_ref()
        .ok_or(other_err!("No text on didSave noti"))?
        .to_owned();
    let uri = params.text_document.uri.clone();

    let mut w = state.0.try_write()?;
    w.update_doc_and_agents_from_text(uri.clone(), &text)?;

    let notification = Into::<InteractLspNotification>::into(params);

    if let Some(tokens) = w.documents.get(&uri).cloned() {
        for (pos, parsed_comment) in tokens.into_iter() {
            let doc_info = InteractDocumentInfo {
                tokens: &tokens,
                my_pos: pos,
                uri: &uri,
            };
            parsed_comment
                .execute_from_lsp_message(&mut w, &mut sender, notification.clone(), doc_info)
                .await?;
        }
    }

    if w.database.is_some() {
        w.save_docs_to_database().await?;
        w.save_agent_memories_to_database().await?;
    }

    sender
        .send_work_done_end(Some("Updated Document Tokens"))
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
    state: SharedState<'static>,
    mut sender: BufferOpChannelSender,
) -> MainResult<()> {
    let text_doc_item = serde_json::from_value::<DidOpenTextDocumentParams>(noti.params)?;
    let text = text_doc_item.text_document.text;
    let uri = text_doc_item.text_document.uri;

    let mut w = state.0.try_write()?;
    w.update_doc_and_agents_from_text(uri.clone(), &text)?;
    // let mut w = state.0.try_write()?;
    // this causes a crash?
    // w.update_doc_and_agents_from_text(uri.clone(), text)?;

    sender
        .send_operation(LspDiagnostic::diagnose_document(uri, &mut w)?.into())
        .await?;
    Ok(())
}
