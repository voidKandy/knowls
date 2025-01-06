use super::{
    execution::InteractDocumentInfo,
    logic::{
        InteractArg, InteractLspNotification, InteractLspRequest, InteractVar, LspMessageInteract,
    },
    parsing::{
        comments::ParsedComment,
        language_ext_from_uri,
        lexer::Lexer,
        tokens::{vec::TokenVec, Token},
    },
    Interact,
};
use crate::{
    agents::{message_stack_into_marked_string, AgentID, Agents},
    other_err,
    server::buffer_operations::BufferOperation,
    state::LspState,
    util::Diff,
    MainErr, MainResult,
};
use espionox::{
    agents::{memory::OtherRoleTo, Agent},
    language_models::completions::streaming::CompletionStreamStatus,
    prelude::{Message, MessageRole},
};
use lsp_server::RequestId;
use lsp_types::{
    ApplyWorkspaceEditParams, Diagnostic, DiagnosticSeverity, HoverContents, MessageType, Range,
    ShowMessageParams, TextEdit, Uri, WorkspaceEdit,
};
use std::collections::HashMap;
use tokio::sync::RwLockWriteGuard;
use tracing::warn;

pub(crate) struct AgentInteractExArgs<'i> {
    user_input: AgentInteractUserInput,
    lsp_state: AgentInteractLspState<'i>,
}

pub(crate) struct AgentInteractLspState<'i> {
    agent: &'i mut Agent,
    document_state: TokenVec<'i>,
    uri: &'i Uri,
}

pub(super) struct AgentInteractUserInput {
    content: String,
    range: Range,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AgentInteract {
    Push,
    Prompt,
    RagPrompt,
}

impl AgentInteract {
    const PUSH: char = '+';
    const PROMPT: char = '@';
    const RAG_PROMPT: char = '$';
}

impl TryFrom<char> for AgentInteract {
    type Error = MainErr;
    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            Self::PUSH => Ok(Self::Push),
            Self::PROMPT => Ok(Self::Prompt),
            Self::RAG_PROMPT => Ok(Self::RagPrompt),
            _ => Err(std::io::Error::other(format!(
                "could not create agent interact from {value}"
            ))
            .into()),
        }
    }
}

pub fn push_interact_role(uri: &Uri, token_idx: usize) -> MessageRole {
    MessageRole::Other {
        alias: format!("{}:{token_idx}", uri.to_string()),
        coerce_to: OtherRoleTo::User,
    }
}

impl AgentInteract {
    pub fn push_interact_diff_handle<'i>(
        &self,
        agent: &mut Agent,
        diff: &Diff<Token<'i>>,
        info: InteractDocumentInfo<'i>,
    ) -> MainResult<()> {
        match self {
            Self::Push => match diff {
                Diff::Insert(i, _) => {
                    let role = push_interact_role(info.uri, *i);
                    if let Some(Token::Block(next_block)) = info.tokens.get(i + 1) {
                        agent.cache.push(Message {
                            role: role.clone(),
                            content: next_block.to_string(),
                        });
                    }
                }

                d @ Diff::Change(i, _) | d @ Diff::Delete(i) => {
                    let mut should_delete = true;

                    if let Diff::Change(_, t) = d {
                        if let Token::Comment(ParsedComment {
                            interact: Some(interact),
                            ..
                        }) = t
                        {
                            if interact.variant == InteractVar::Agent(*self) {
                                warn!("Some trivial change must have occurred, will not delete");
                                should_delete = false;
                            }
                        }
                    }

                    if should_delete {
                        let role = push_interact_role(info.uri, *i);
                        agent.cache.mut_filter_by(&role, false);
                    }
                }
            },
            _ => {}
        }
        Ok(())
    }
}

impl<'i, 'g> LspMessageInteract<'i, 'g, AgentInteractExArgs<'i>> for AgentInteract {
    fn diagnostics(&self, args: AgentInteractExArgs<'i>) -> Vec<Diagnostic> {
        let mut all_diagnostics = vec![];
        let severity = Some(DiagnosticSeverity::HINT);
        let str = match self {
            Self::Push => "PUSH",
            Self::Prompt => "PROMPT",
            Self::RagPrompt => "RAG_PROMPT",
        };
        all_diagnostics.push(Diagnostic {
            range: args.user_input.range,
            severity,
            message: format!("{str}"),
            ..Default::default()
        });

        all_diagnostics
    }

    async fn execute_request(
        &self,
        args: AgentInteractExArgs<'i>,
        rq_id: RequestId,
        params: impl Into<InteractLspRequest>,
        sender: &mut crate::server::buffer_operations::BufferOpChannelSender,
    ) -> MainResult<()> {
        match Into::<InteractLspRequest>::into(params) {
            InteractLspRequest::GotoDef(goto) => {
                let uri = goto.text_document_position_params.text_document.uri;

                match self {
                    Self::Push => {
                        let message = ShowMessageParams {
                            typ: MessageType::INFO,
                            message: format!("Push command has no GOTO function"),
                        };

                        sender.send_operation(message.into()).await?;
                    }
                    Self::Prompt => {
                        if args.user_input.content.trim().is_empty() {
                            sender
                                .send_work_done_report(Some("No messages to send"), None)
                                .await?;

                            return Ok(());
                        }

                        let mut changes = HashMap::new();
                        let mut change_range = args.user_input.range;
                        change_range.start.character += 2;

                        changes.insert(
                            uri.clone(),
                            vec![TextEdit {
                                range: change_range,
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

                        let message = Message::new_user(&args.user_input.content);
                        args.lsp_state.agent.cache.push(message);

                        let mut stream_handler = args.lsp_state.agent.stream_completion().await?;

                        sender
                            .send_work_done_report(
                                Some("Started Receiving Streamed Completion"),
                                None,
                            )
                            .await?;

                        let mut whole_message = String::new();
                        loop {
                            match stream_handler.receive(Some(args.lsp_state.agent)).await {
                                Ok(status) => {
                                    warn!("STATUS: {status:?}");
                                    match status {
                                        Some(CompletionStreamStatus::Working(token)) => {
                                            warn!("got completion token: {}", token);
                                            whole_message.push_str(&token);
                                            sender
                                                .send_work_done_report(Some(&token), None)
                                                .await?;
                                        }
                                        Some(CompletionStreamStatus::Finished) => {
                                            warn!("finished");
                                            break;
                                        }
                                        None => break,
                                    }
                                }
                                Err(err) => return Err(other_err!("{err:#?}")),
                            }
                        }
                        sender.send_work_done_end(Some("Finished")).await?;

                        if !whole_message.trim().is_empty() {
                            warn!("whole message: {whole_message}");

                            let message = ShowMessageParams {
                                typ: MessageType::INFO,
                                message: whole_message.clone(),
                            };

                            sender.send_operation(message.into()).await?;
                        }
                    }
                    Self::RagPrompt => {}
                }
            }
            InteractLspRequest::Hover(_hover) => {
                let stack = Agents::get_last_n_messages(&args.lsp_state.agent, 5);
                let contents = HoverContents::Scalar(message_stack_into_marked_string(stack));
                sender
                    .send_operation(BufferOperation::HoverResponse {
                        id: rq_id,
                        contents,
                    })
                    .await?;
            }
            InteractLspRequest::Diagnostic(diag) => {}
        }
        Ok(())
    }

    async fn execute_notification(
        &self,
        args: AgentInteractExArgs<'i>,
        noti: impl Into<InteractLspNotification>,
        sender: &mut crate::server::buffer_operations::BufferOpChannelSender,
    ) -> MainResult<()> {
        match Into::<InteractLspNotification>::into(noti) {
            InteractLspNotification::Save(did_save) => {
                match self {
                    //
                    // Self::Push => {
                    //     args.lsp_state.agent.cache.push(Message {
                    //         role: push_interact_role(&did_save.text_document.uri),
                    //         content: args.user_input.content,
                    //     });
                    // }
                    _ => {}
                }
            }
            InteractLspNotification::Change(did_change) => {
                if let Some(change) = did_change.content_changes.first() {}
            }
            InteractLspNotification::Open(did_open) => {}
        }

        Ok(())
    }

    #[tracing::instrument("get ex args", skip(w))]
    // THis function should return a result
    fn get_execution_args(
        &self,
        w: &'i mut RwLockWriteGuard<'g, LspState<'static>>,
        interact_comment: &'i ParsedComment<'_>,
        doc_info: InteractDocumentInfo<'i>,
        args: &Vec<InteractArg>,
    ) -> Option<AgentInteractExArgs<'i>> {
        warn!("args: {args:?}");

        let mut first_arg_is_string = false;
        let agent_char = match &args[0] {
            InteractArg::Char(c) => *c,
            InteractArg::String(s) => {
                first_arg_is_string = true;
                s.chars()
                    .next()
                    .expect("empty string as first interact arg")
            }
        };

        let agent_id = AgentID::from((doc_info.uri, agent_char));
        let document_state = w.documents.get(&doc_info.uri).unwrap().to_owned();

        let agent = w
            .agents
            .get_agent_mut(&agent_id)
            .expect("No agent matching given id");
        let content = match self {
            Self::Prompt | Self::RagPrompt => args.into_iter().fold(String::new(), |str, arg| {
                if let Some(arg_str) = arg
                    .as_string()
                    .and_then(|str| Some(str.to_string()))
                    .or(arg.as_char().and_then(|ch| Some(ch.to_string())))
                {
                    if first_arg_is_string {
                        format!("{str} {}", &arg_str[1..])
                    } else {
                        format!("{str} {arg_str}",)
                    }
                } else {
                    str
                }
            }),

            Self::Push => {
                if let Some(tok) = doc_info.tokens.get(doc_info.my_pos + 1) {
                    match tok {
                        Token::Block(content) => {
                            warn!("Push interact should add {content} to agent context");
                            content.to_string()
                        }
                        _ => {
                            warn!(
                                "Push interact's next token is not a Token::Block, got {}",
                                tok.variant_display()
                            );
                            String::new()
                        }
                    }
                } else {
                    warn!("No token after Push interact");
                    String::new()
                }
            }
        };

        // this might be BAD
        if content.is_empty() {
            warn!("passing empty user input to agent interact args");
        }

        let ex_args = AgentInteractExArgs {
            lsp_state: AgentInteractLspState {
                agent,
                document_state,
                uri: doc_info.uri,
            },
            user_input: AgentInteractUserInput {
                content,
                range: interact_comment.range,
            },
        };

        warn!("got execution args");

        Some(ex_args)
    }
}
