use super::{
    logic::LspMessageInteract,
    parsing::{comments::ParsedComment, tokens::TokenVec},
    InteractLspMessage, InteractVar,
};
use crate::{
    handle::{buffer_operations::BufferOpChannelSender, error::HandleResult},
    state::LspState,
};
use lsp_types::{Diagnostic, MessageType, ShowMessageParams, Uri};
use tokio::sync::RwLockWriteGuard;

#[derive(Debug)]
pub struct InteractDocumentInfo<'i> {
    pub tokens: &'i TokenVec<'i>,
    pub my_pos: usize,
    pub uri: &'i Uri,
}

impl<'i> ParsedComment<'i> {
    pub fn get_diagnostics(
        &self,
        w: &'_ mut RwLockWriteGuard<'_, LspState<'static>>,
        doc_info: InteractDocumentInfo<'i>,
    ) -> Vec<Diagnostic> {
        let mut all_diagnostics = vec![];

        if let Some(interact) = self.interact.as_ref() {
            match interact.variant {
                InteractVar::Agent(i) => {
                    if let Some(args) =
                        i.get_execution_args(w, &self, doc_info, &interact.parsed_args)
                    {
                        all_diagnostics.append(&mut i.diagnostics(args));
                    }
                }
                InteractVar::DB(i) => {
                    if let Some(args) =
                        i.get_execution_args(w, &self, doc_info, &interact.parsed_args)
                    {
                        all_diagnostics.append(&mut i.diagnostics(args));
                    }
                }
            }
        }
        all_diagnostics
    }

    pub async fn execute_from_lsp_message(
        &self,
        w: &'_ mut RwLockWriteGuard<'_, LspState<'static>>,
        sender: &mut BufferOpChannelSender,
        message: impl Into<InteractLspMessage>,
        doc_info: InteractDocumentInfo<'i>,
    ) -> HandleResult<()> {
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

            sender.send_work_done_report(Some(&report), None).await?;

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
