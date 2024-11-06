use super::{
    execution::InteractDocumentInfo,
    logic::{
        InteractArg, InteractLspNotification, InteractLspRequest, InteractVar, LspMessageInteract,
    },
    parsing::{
        comments::ParsedComment,
        language_ext_from_uri,
        lexer::Lexer,
        tokens::{Token, TokenVec},
    },
};
use crate::{
    agents::{message_stack_into_marked_string, AgentID, Agents},
    handle::{
        buffer_operations::BufferOperation,
        error::{HandleError, HandleResult},
    },
    state::LspState,
};
use espionox::{
    agents::{memory::OtherRoleTo, Agent},
    language_models::completions::streaming::CompletionStreamStatus,
    prelude::{Message, MessageRole},
};
use lsp_server::RequestId;
use lsp_types::{
    ApplyWorkspaceEditParams, HoverContents, MessageType, Range, ShowMessageParams, TextEdit, Uri,
    WorkspaceEdit,
};
use std::collections::HashMap;
use tokio::sync::RwLockWriteGuard;
use tracing::warn;

pub(super) struct AgentInteractExArgs<'i> {
    user_input: AgentInteractUserInput,
    lsp_state: AgentInteractLspState<'i>,
}

pub(super) struct AgentInteractLspState<'i> {
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
    type Error = anyhow::Error;
    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            Self::PUSH => Ok(Self::Push),
            Self::PROMPT => Ok(Self::Prompt),
            Self::RAG_PROMPT => Ok(Self::RagPrompt),
            _ => Err(anyhow::anyhow!(
                "could not create agent interact from {value}"
            )),
        }
    }
}

pub fn uri_agent_role(uri: &Uri) -> MessageRole {
    MessageRole::Other {
        alias: uri.to_string(),
        coerce_to: OtherRoleTo::User,
    }
}

impl<'i> AgentInteractExArgs<'i> {
    fn update_agent_memory_from_new_text(&mut self, new_text: &str) {
        let mut lexer = Lexer::new(new_text, language_ext_from_uri(&self.lsp_state.uri));
        let new_tokens = lexer.lex_input();

        let prev_push_interacts: Vec<(usize, &ParsedComment<'_>)> = self
            .lsp_state
            .document_state
            .into_iter()
            .filter_map(|(i, c)| {
                if let Some(ref int) = c.interact {
                    if let InteractVar::AGENT_PUSH = int.variant {
                        return Some((i, c));
                    }
                }
                None
            })
            .collect();

        for (i, comment) in prev_push_interacts {
            if new_tokens.get(i).is_some_and(|t| {
                if let Token::Comment(c) = t {
                    c != comment
                } else {
                    true
                }
            }) {
                self.lsp_state
                    .agent
                    .cache
                    .mut_filter_by(&uri_agent_role(self.lsp_state.uri), false)
            }
        }
    }
}

impl<'i> LspMessageInteract<'i, AgentInteractExArgs<'i>> for AgentInteract {
    async fn execute_request(
        &self,
        args: AgentInteractExArgs<'i>,
        rq_id: RequestId,
        params: impl Into<InteractLspRequest>,
        sender: &mut crate::handle::buffer_operations::BufferOpChannelSender,
    ) -> HandleResult<()> {
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
                        // let (range_of_text, text_for_interact) =
                        //     comment.text_for_interact().unwrap();
                        // if text_for_interact.trim().is_empty() {
                        //     return Ok(());
                        // }

                        let mut changes = HashMap::new();

                        changes.insert(
                            uri.clone(),
                            vec![TextEdit {
                                range: args.user_input.range,
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
                            match stream_handler.receive(args.lsp_state.agent).await {
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
                    Self::RagPrompt => {}
                }
            }
            InteractLspRequest::Hover(hover) => {
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
        sender: &mut crate::handle::buffer_operations::BufferOpChannelSender,
    ) -> HandleResult<()> {
        match Into::<InteractLspNotification>::into(noti) {
            InteractLspNotification::Save(did_save) => {
                match self {
                    //
                    Self::Push => {
                        args.lsp_state.agent.cache.push(Message {
                            role: uri_agent_role(&did_save.text_document.uri),
                            content: args.user_input.content,
                        });
                    }
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
    fn n_expected_args(&self) -> usize {
        match self {
            AgentInteract::Push => 1,
            AgentInteract::Prompt | AgentInteract::RagPrompt => 2,
        }
    }
    #[tracing::instrument("get ex args", skip(w))]
    fn get_execution_args(
        &self,
        w: &'i mut RwLockWriteGuard<'_, LspState<'static>>,
        interact_comment: &'i ParsedComment<'_>,
        doc_info: InteractDocumentInfo<'i>,
        args: &Vec<InteractArg>,
    ) -> Option<AgentInteractExArgs<'i>> {
        assert!(
            args.len() <= self.n_expected_args(),
            "Got too many arguments"
        );
        warn!("args: {args:?}");

        let agent_char = args[0].as_char().expect("first arg is not char");
        let agent_id = AgentID::from(*agent_char);
        let document_state = w.documents.get(&doc_info.uri).unwrap().to_owned();

        if let Some(agents) = w.agents.as_mut() {
            let agent = agents
                .get_agent_mut(&agent_id)
                .expect("No agent matching given id");
            let content = match self {
                Self::Prompt | Self::RagPrompt => args
                    .into_iter()
                    .nth(1)
                    .expect("too little args")
                    .as_string()
                    .expect("second arg not string")
                    .to_string(),

                Self::Push => doc_info
                    .tokens
                    .get(doc_info.my_pos + 1)
                    .expect("No token after push command comment")
                    .to_string(),
            };

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

            return Some(ex_args);
        }
        None
    }
}
