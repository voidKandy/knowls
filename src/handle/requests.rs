use super::{
    buffer_operations::{BufferOpChannelHandler, BufferOpChannelSender, BufferOperation},
    error::{HandleError, HandleResult},
};
use crate::{
    agents::{message_stack_into_marked_string, AgentID, Agents},
    handle::BufferOpChannelJoinHandle,
    interact::id::{human_readable_int, DOCUMENT_ID, GLOBAL_ID, PROMPT_ID, PUSH_ID, RAG_PUSH_ID},
    state::SharedState,
};
use anyhow::anyhow;
use espionox::{
    language_models::completions::streaming::CompletionStreamStatus,
    prelude::{stream_completion, ListenerTrigger, Message},
};
use lsp_server::Request;
use lsp_types::{
    ApplyWorkspaceEditParams, DocumentDiagnosticParams, GotoDefinitionParams, HoverContents,
    HoverParams, MessageType, ShowMessageParams, TextEdit, WorkspaceEdit,
};
use std::collections::HashMap;
use tracing::{debug, warn};

#[tracing::instrument(name = "handle request", skip_all)]
pub async fn handle_request(
    req: Request,
    state: SharedState,
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
    mut state: SharedState,
    mut sender: BufferOpChannelSender,
) -> HandleResult<()> {
    let params = serde_json::from_value::<GotoDefinitionParams>(req.params)?;

    let uri = params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    warn!("Gotodef Position: {position:?}");

    // let message = ShowMessageParams {
    //     typ: MessageType::INFO,
    //     message: format!("Triggered GotoDef at position {position:?}",),
    // };

    // sender.send_operation(message.into()).await?;
    let mut w = state.get_write()?;

    let doc_tokens = w
        .documents
        .get(&uri)
        .ok_or(anyhow!("document not present"))?;

    let (comment, idx) = match doc_tokens.comment_in_position(&position) {
        Some((com, i)) => (com.clone(), i),
        None => {
            return Err(anyhow!("no comment at gotodef position").into());
        }
    };

    if comment.try_get_interact_integer().is_err() {
        return Err(anyhow!("no interact at gotodef position").into());
    }

    let integer = comment.try_get_interact_integer()?;
    let (command, scope) = w.registry.interract_tuple(integer)?;

    let message = ShowMessageParams {
        typ: MessageType::INFO,
        message: format!("Triggered GotoDef with {}", human_readable_int(integer)),
    };

    sender.send_operation(message.into()).await?;

    let agent = match w.agents.as_mut() {
        Some(agents) => {
            let id = match scope {
                _ if scope == GLOBAL_ID => AgentID::Global,
                _ if scope == DOCUMENT_ID => AgentID::from(&uri),
                _ => unreachable!(),
            };
            agents.get_agent_mut(id).expect("Could not get agent")
        }
        None => {
            warn!("no agents");
            return Ok(());
        }
    };

    // let role = MessageRole::Other {
    //     alias: uri.to_string(),
    //     coerce_to: OtherRoleTo::User,
    // };

    match command {
        PUSH_ID => {
            let message = ShowMessageParams {
                typ: MessageType::INFO,
                message: format!("Push command has no GOTO function"),
            };

            sender.send_operation(message.into()).await?;
        }

        RAG_PUSH_ID => {
            let (_range, text_for_interact) = comment.text_for_interact().unwrap();
            if text_for_interact.trim().is_empty() {
                return Ok(());
            }

            // let embedded = embeddings::get_passage_embeddings(vec![&text_for_interact])?;
        }

        PROMPT_ID => {
            let (range_of_text, text_for_interact) = comment.text_for_interact().unwrap();
            if text_for_interact.trim().is_empty() {
                return Ok(());
            }

            let mut changes = HashMap::new();

            changes.insert(
                uri.clone(),
                vec![TextEdit {
                    range: range_of_text,
                    new_text: String::new(),
                }],
            );

            let edit_params = ApplyWorkspaceEditParams {
                label: None,
                edit: WorkspaceEdit {
                    changes: Some(changes),
                    ..Default::default()
                },
            };

            sender.send_operation(edit_params.into()).await?;

            let message = Message::new_user(&text_for_interact);
            agent.cache.push(message);

            let mut stream_handler = agent
                .do_action(stream_completion, (), Option::<ListenerTrigger>::None)
                .await?;

            sender
                .send_work_done_report(Some("Started Receiving Streamed Completion"), None)
                .await?;

            let mut whole_message = String::new();
            warn!("starting inference response loop");
            loop {
                match stream_handler.receive(agent).await {
                    Ok(status) => {
                        warn!("STATUS: {status:?}");
                        match status {
                            Some(CompletionStreamStatus::Working(token)) => {
                                warn!("got completion token: {}", token);
                                whole_message.push_str(&token);
                                sender.send_work_done_report(Some(&token), None).await?;
                            }
                            Some(CompletionStreamStatus::Finished) => {
                                warn!("finished");
                                sender.send_work_done_end(Some("Finished")).await?;
                                break;
                            }
                            None => break,
                        }
                    }
                    Err(err) => return Err(HandleError::from(err)),
                }
            }

            if !whole_message.trim().is_empty() {
                warn!("whole message: {whole_message}");

                let message = ShowMessageParams {
                    typ: MessageType::INFO,
                    message: whole_message.clone(),
                };

                sender.send_operation(message.into()).await?;
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}

#[tracing::instrument(name = "hover", skip_all)]
pub async fn handle_hover(
    req: Request,
    state: SharedState,
    mut sender: BufferOpChannelSender,
) -> HandleResult<()> {
    let params = serde_json::from_value::<HoverParams>(req.params)?;
    let uri = params.text_document_position_params.text_document.uri;
    let position = params.text_document_position_params.position;

    debug!(
        "got hover request on doc: {:?} as position: {:?}",
        uri.as_str(),
        position
    );

    let r = state.get_read()?;

    let doc_tokens = r
        .documents
        .get(&uri)
        .ok_or(anyhow!("document not present"))?;

    if let Some((comment, _)) = doc_tokens.comment_in_position(&position) {
        if let Some(integer) = comment.try_get_interact_integer().ok() {
            let (_command, scope) = r.registry.interract_tuple(integer)?;
            let agent = match r.agents.as_ref() {
                Some(agents) => {
                    let id = match scope {
                        _ if scope == GLOBAL_ID => AgentID::Global,
                        _ if scope == DOCUMENT_ID => AgentID::from(&uri),
                        _ => unreachable!(),
                    };
                    agents.get_agent_ref(id).expect("Could not get agent")
                }
                None => {
                    warn!("no agents");
                    return Ok(());
                }
            };
            let stack = Agents::get_last_n_messages(agent, 5);
            let contents = HoverContents::Scalar(message_stack_into_marked_string(stack));

            sender
                .send_operation(BufferOperation::HoverResponse {
                    id: req.id,
                    contents,
                })
                .await?;
        }
    }
    // if let Some(burns_on_doc) = r.store.burns.read_burns_on_doc(&uri) {
    //     if let Some(burn) = burns_on_doc
    //         .iter()
    //         .find(|b| b.activation.is_in_position(&position))
    //     {
    //         if let Some(contents) = &burn.hover_contents {
    //             sender
    //                 .send_operation(BufferOperation::HoverResponse {
    //                     contents: contents.clone(),
    //                     id: req.id,
    //                 })
    //                 .await?;
    //         }
    //     }
    // }

    Ok(())
}

async fn handle_diagnostics(
    req: Request,
    mut state: SharedState,
    sender: BufferOpChannelSender,
) -> HandleResult<()> {
    let params: DocumentDiagnosticParams =
        serde_json::from_value::<DocumentDiagnosticParams>(req.params)?;
    let w = state.get_write()?;
    // sender
    //     .send_operation(
    //         LspDiagnostic::diagnose_document(params.text_document.uri, &mut w.store)?.into(),
    //     )
    // .await?;
    Ok(())
}

async fn handle_shutdown(
    state: SharedState,
    sender: BufferOpChannelSender,
) -> HandleResult<()> {
    warn!("shutting down server");
    // sender.start_work_done(Some("Shutting down server")).await?;
    // let mut w = state.get_write()?;
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
