use super::{messages::InteractLspMessage, Interact, InteractWrapper};
use crate::{
    knowledge::parsing::{comments::ParsedComment, tokens::vec::TokenVec},
    other_err,
    rpc::lsp::buffer_operations::BufferOpChannelSender,
    server::ServerState,
    MainResult,
};
use lsp_types::{Diagnostic, Uri};
use tokio::sync::RwLockWriteGuard;

#[derive(Debug)]
pub struct InteractDocumentInfo<'i> {
    pub tokens: &'i TokenVec,
    pub my_pos: usize,
    pub uri: &'i Uri,
}

impl ParsedComment {
    pub async fn execute_from_lsp_message<'i>(
        &self,
        w: &'_ mut RwLockWriteGuard<'_, ServerState>,
        sender: &mut BufferOpChannelSender,
        message: impl Into<InteractLspMessage>,
        doc_info: InteractDocumentInfo<'i>,
    ) -> MainResult<()> {
        if let Some(interact) = InteractWrapper::try_from_str(&self.content) {
            let message = Into::<InteractLspMessage>::into(message);

            // let report = format!(
            //     "Triggered LSP {} with {:?}",
            //     match message {
            //         InteractLspMessage::Req { .. } => "request",
            //         InteractLspMessage::Noti { .. } => "notification",
            //     },
            //     interact.variant
            // );

            // sender
            //     .send_work_done_end(Some(&report))
            //     .await
            //     .expect("failed to send work done");

            match interact.variant {
                InteractVar::DB(db_int) => {
                    if let Some(args) =
                        db_int.get_execution_args(w, self, doc_info, &interact.parsed_args)
                    {
                        match message {
                            InteractLspMessage::Req { body, id } => {
                                db_int
                                    .execute_request(args, id, body, sender)
                                    .await
                                    .expect("failed to execute db interact from request");
                            }
                            InteractLspMessage::Noti(noti) => {
                                db_int
                                    .execute_notification(args, noti, sender)
                                    .await
                                    .expect("failed to execute db interact from notification");
                            }
                        }
                    } else {
                        return Err(other_err!("failed to get Execution args"));
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
                                    .await
                                    .expect("failed to execute agent interact from request");
                            }
                            InteractLspMessage::Noti(noti) => {
                                agent_interact
                                    .execute_notification(args, noti, sender)
                                    .await
                                    .expect("failed to execute agent interact from notification");
                            }
                        }
                    } else {
                        return Err(other_err!("failed to get Execution args"));
                    }
                }
            }
        }
        Ok(())
    }
}
