use super::{
    buffer_operations::{BufferOpChannelHandler, BufferOpChannelSender},
    error::{HandleError, HandleResult},
};
use crate::{
    handle::BufferOpChannelJoinHandle,
    interact::{execution::InteractDocumentInfo, InteractLspRequest},
    state::SharedState,
};
use anyhow::anyhow;
use lsp_server::Request;
use lsp_types::{DocumentDiagnosticParams, GotoDefinitionParams, HoverParams};
use tracing::{debug, warn};

#[tracing::instrument(name = "handle request", skip_all)]
pub async fn handle_request(
    req: Request,
    state: SharedState<'static>,
) -> HandleResult<BufferOpChannelHandler> {
    let handle = BufferOpChannelHandler::new();
    let mut task_sender = handle.sender.clone();
    let _: BufferOpChannelJoinHandle = tokio::spawn(async move {
        let method = req.method.clone();
        match match method.as_str() {
            "textDocument/definition" => {
                handle_goto_definition(req, state, task_sender.clone()).await
            }
            "textDocument/hover" => handle_hover(req, state, task_sender.clone()).await,
            "textDocument/diagnostic" => handle_diagnostics(req, state, task_sender.clone()).await,
            "shutdown" => handle_shutdown(state, task_sender.clone()).await,
            _ => {
                warn!("unhandled request method: {}", req.method);
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
                err.request_err(&mut task_sender).await?;
                Ok(())
            }
        }
    });
    return Ok(handle);
}

#[tracing::instrument(name = "goto def", skip_all)]
pub async fn handle_goto_definition(
    req: Request,
    state: SharedState<'static>,
    mut sender: BufferOpChannelSender,
) -> HandleResult<()> {
    let params = serde_json::from_value::<GotoDefinitionParams>(req.params)?;

    let uri = params
        .text_document_position_params
        .text_document
        .uri
        .clone();
    let position = params.text_document_position_params.position;

    warn!("Gotodef Position: {position:?}");

    let mut w = state.0.try_write()?;

    let doc_tokens = w
        .documents
        .get(&uri)
        .ok_or(anyhow!("document not present"))?
        .clone();

    let (comment, idx) = match doc_tokens.comment_in_position(&position) {
        Some((com, i)) => (com.clone(), i),
        None => return Ok(()),
    };

    let request = Into::<InteractLspRequest>::into(params);

    let doc_info = InteractDocumentInfo {
        tokens: &doc_tokens,
        my_pos: idx,
        uri: &uri,
    };
    comment
        .execute_from_lsp_message(&mut w, &mut sender, (request, req.id), doc_info)
        .await?;

    Ok(())
}

#[tracing::instrument(name = "hover", skip_all)]
pub async fn handle_hover(
    req: Request,
    state: SharedState<'static>,
    mut sender: BufferOpChannelSender,
) -> HandleResult<()> {
    let params = serde_json::from_value::<HoverParams>(req.params)?;
    let uri = params
        .text_document_position_params
        .text_document
        .uri
        .clone();
    let position = params.text_document_position_params.position;

    let mut w = state.0.try_write()?;

    let doc_tokens = w
        .documents
        .get(&uri)
        .ok_or(anyhow!("document not present"))?
        .clone();
    let (comment, idx) = match doc_tokens.comment_in_position(&position) {
        Some((com, i)) => (com.clone(), i),
        None => {
            return Err(anyhow!("no comment at gotodef position").into());
        }
    };

    let request = Into::<InteractLspRequest>::into(params);

    let doc_info = InteractDocumentInfo {
        tokens: &doc_tokens,
        my_pos: idx,
        uri: &uri,
    };
    comment
        .execute_from_lsp_message(&mut w, &mut sender, (request, req.id), doc_info)
        .await?;

    Ok(())
}

async fn handle_diagnostics(
    req: Request,
    mut state: SharedState<'static>,
    sender: BufferOpChannelSender,
) -> HandleResult<()> {
    let params: DocumentDiagnosticParams =
        serde_json::from_value::<DocumentDiagnosticParams>(req.params)?;
    let w = state.0.try_write()?;
    // sender
    //     .send_operation(
    //         LspDiagnostic::diagnose_document(params.text_document.uri, &mut w.store)?.into(),
    //     )
    // .await?;
    Ok(())
}

async fn handle_shutdown(
    state: SharedState<'static>,
    sender: BufferOpChannelSender,
) -> HandleResult<()> {
    warn!("shutting down server");
    // sender.start_work_done(Some("Shutting down server")).await?;
    // let mut w = state.0.try_write()?;
    // if let Some(_db) = w.database.take() {
    //     sender
    //         .send_work_done_report(Some("Database present, Saving state..."), None)
    //         .await?;
    //     warn!("saving current state to database");
    //
    //     match w.try_update_database().await {
    //         Ok(_) => debug!("succesfully updated database"),
    //         Err(err) => warn!("problem updating database: {:?}", err),
    //     };
    //     sender
    //         .send_work_done_report(Some("Finished saving state, shutting down database"), None)
    //         .await?;
    //
    //     warn!("shutting down database");
    // }
    // sender
    //     .send_work_done_end(Some("Finished Server shutdown"))
    //     .await?;
    Ok(())
}
