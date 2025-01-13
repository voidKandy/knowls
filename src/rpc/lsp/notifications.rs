use super::{
    buffer_operations::{BufferOpChannelHandler, BufferOpChannelSender},
    diagnostics::LspDiagnostic,
    show_notification_err, BufferOpChannelJoinHandle,
};
use crate::{
    agents::AgentID,
    interact::{
        agent::{AgentInteract, AgentInteractExArgs},
        execution::InteractDocumentInfo,
        Interact, InteractLspNotification, InteractVar,
    },
    knowledge::{
        parsing::{language_ext_from_uri, lexer::Lexer, tokens::Token},
        uri_to_surreal_id,
    },
    other_err,
    server::SharedState,
    util::Diff,
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
    let text_document_changes: DidChangeTextDocumentParams =
        serde_json::from_value(noti.params).expect("could not parse changes from notification");
    let uri = text_document_changes.text_document.uri;
    if text_document_changes.content_changes.len() > 1 {
        warn!("more than a single change recieved in notification");
    }
    let text = &text_document_changes.content_changes.first().unwrap().text;

    // warn!("change text: {text}");
    // let mut w = state.0.try_write()?;

    // w.update_doc_and_agents_from_text(uri.clone(), &text)
    //     .expect("failed to update doc and agent");

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
        serde_json::from_value::<DidSaveTextDocumentParams>(noti.params)
            .expect("could not get didSave params");
    let text = params
        .text
        .as_ref()
        .ok_or(other_err!("No text in didSave noti"))?
        .to_owned();
    let uri = params.text_document.uri.clone();

    let mut w = state.try_write().expect("failed to get write lock");
    let ext = language_ext_from_uri(&uri);
    let mut lexer = Lexer::new(&text, ext);
    let new_tokens = lexer.lex_input();
    let knowledge_id = uri_to_surreal_id(&uri);

    if let Some(crate::knowledge::Knowledge::Document(old_tokens)) =
        w.knowledge.remove(&knowledge_id)
    {
        let diff = Diff::get_diffs(&old_tokens, &new_tokens);
        for d in diff.iter() {
            let idx = match d {
                Diff::Delete(idx) => idx,
                Diff::Insert(idx, _) => idx,
                Diff::Change(idx, _) => idx,
            };

            if let Some(interact) = old_tokens.get(*idx).as_ref().and_then(|t| {
                if let Token::Comment(c) = t {
                    Interact::try_from_str(&c.content)
                } else {
                    None
                }
            }) {
                let doc_info = InteractDocumentInfo {
                    tokens: &old_tokens,
                    my_pos: *idx,
                    uri: &uri,
                };
                match interact.variant {
                    InteractVar::DB(_) => {}
                    InteractVar::Agent(int) => {
                        let agent_id = interact.parsed_args.first().and_then(|arg| {
                            arg.as_char()
                                .and_then(|ch| Some(AgentID::from((&uri, *ch))))
                        });
                        if let Some(agent) = w.agents.get_agent_mut(agent_id.unwrap()) {
                            AgentInteract::push_interact_diff_handle(&int, agent, d, doc_info)?;
                        }
                    }
                };
            }
        }
    }

    w.knowledge.insert(
        knowledge_id,
        crate::knowledge::Knowledge::Document(new_tokens),
    );

    // let notification = Into::<InteractLspNotification>::into(params);

    // if w.database.is_some() {
    //     w.save_docs_to_database()
    //         .await
    //         .expect("failed to save docs to database");
    //     w.save_agent_memories_to_database()
    //         .await
    //         .expect("failed to save agent memories");
    // }

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

    // let mut w = state.0.try_write()?;

    // if let Some(tokens) = w.documents.get(&uri) {}
    // w.update_doc_and_agents_from_text(uri.clone(), &text)?;
    // let mut w = state.0.try_write()?;
    // this causes a crash?
    // w.update_doc_and_agents_from_text(uri.clone(), text)?;

    // sender
    //     .send_operation(LspDiagnostic::diagnose_document(uri, &mut w)?.into())
    //     .await?;
    Ok(())
}
