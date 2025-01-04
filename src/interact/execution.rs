use super::{
    logic::LspMessageInteract,
    parsing::{comments::ParsedComment, tokens::vec::TokenVec},
    InteractLspMessage, InteractVar,
};
use crate::{server::buffer_operations::BufferOpChannelSender, state::LspState, MainResult};
use lsp_types::{Diagnostic, Uri};
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
    ) -> MainResult<()> {
        if let Some(interact) = &self.interact {
            let message = Into::<InteractLspMessage>::into(message);

            let report = format!(
                "Triggered LSP {} with {:#?}",
                match message {
                    InteractLspMessage::Req { .. } => "request",
                    InteractLspMessage::Noti { .. } => "notification",
                },
                interact.variant
            );

            sender.send_work_done_end(Some(&report)).await?;

            match interact.variant {
                InteractVar::DB(db_int) => {
                    if let Some(args) =
                        db_int.get_execution_args(w, self, doc_info, &interact.parsed_args)
                    {
                        match message {
                            InteractLspMessage::Req { body, id } => {
                                db_int.execute_request(args, id, body, sender).await?;
                            }
                            InteractLspMessage::Noti(noti) => {
                                db_int.execute_notification(args, noti, sender).await?;
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
