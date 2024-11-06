use super::{
    logic::{IntoInteractVar, LspMessageInteract},
    parsing::{comments::ParsedComment, tokens::TokenVec},
    InteractLspMessage, InteractVar,
};
use crate::{
    handle::{buffer_operations::BufferOpChannelSender, error::HandleResult},
    state::LspState,
};
use lsp_types::{MessageType, ShowMessageParams, Uri};
use tokio::sync::RwLockWriteGuard;

#[derive(Debug)]
pub struct InteractDocumentInfo<'i> {
    pub tokens: &'i TokenVec<'i>,
    pub my_pos: usize,
    pub uri: &'i Uri,
}

impl<'i> ParsedComment<'i> {
    pub async fn execute_from_lsp_message(
        &self,
        w: &'_ mut RwLockWriteGuard<'_, LspState<'static>>,
        sender: &mut BufferOpChannelSender,
        message: impl Into<InteractLspMessage>,
        doc_info: InteractDocumentInfo<'i>,
    ) -> HandleResult<()> {
        if let Some(interact) = &self.interact {
            let message = Into::<InteractLspMessage>::into(message);
            let showmessage = ShowMessageParams {
                typ: MessageType::INFO,
                message: format!("Triggered {message:#?} with {:#?}", interact.variant),
            };

            sender.send_operation(showmessage.into()).await?;

            match interact.variant {
                InteractVar::State(state_interact) => {
                    if let Some(args) =
                        state_interact.get_execution_args(w, self, doc_info, &interact.parsed_args)
                    {
                        match message {
                            InteractLspMessage::Req { body, id } => {
                                state_interact
                                    .execute_request(args, id, body, sender)
                                    .await?;
                            }
                            InteractLspMessage::Noti(noti) => {
                                state_interact
                                    .execute_notification(args, noti, sender)
                                    .await?;
                            }
                        }
                    }
                }

                InteractVar::Agent(agent_interact) => {
                    if let Some(args) =
                        agent_interact.get_execution_args(w, self, doc_info, &interact.parsed_args)
                    {
                        match message {
                            InteractLspMessage::Req { body, id } => {
                                agent_interact
                                    .execute_request(args, id, body, sender)
                                    .await?;
                            }
                            InteractLspMessage::Noti(noti) => {
                                agent_interact
                                    .execute_notification(args, noti, sender)
                                    .await?;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
