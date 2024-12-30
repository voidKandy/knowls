use super::{
    execution::InteractDocumentInfo,
    logic::{InteractArg, LspMessageInteract},
    parsing::comments::ParsedComment,
    InteractLspRequest,
};
use crate::{server::buffer_operations::BufferOperation, state::LspState, MainErr, MainResult};
use lsp_server::RequestId;
use lsp_types::{
    Diagnostic, DiagnosticSeverity, HoverContents, MessageType, Range, ShowMessageParams,
};
use tokio::sync::RwLockWriteGuard;
use tracing::warn;

pub struct DBInteractExArgs<'i, 'g> {
    range: Range,
    // if present, database is present
    state_guard: Option<&'i mut RwLockWriteGuard<'g, LspState<'static>>>,
    typ: DBInteractTyp,
}

enum DBInteractTyp {
    Status,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DBInteract;

impl DBInteract {
    const DATABASE: char = '%';
}

impl TryFrom<char> for DBInteract {
    type Error = MainErr;
    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            Self::DATABASE => Ok(Self),
            _ => Err(std::io::Error::other(format!(
                "could not create agent interact from {value}"
            ))
            .into()),
        }
    }
}

impl<'i, 'g> LspMessageInteract<'i, 'g, DBInteractExArgs<'i, 'g>> for DBInteract {
    fn diagnostics(&self, args: DBInteractExArgs<'i, 'g>) -> Vec<Diagnostic> {
        let severity = Some(DiagnosticSeverity::HINT);
        let str = match args.typ {
            DBInteractTyp::Status => "Status",
        };
        vec![Diagnostic {
            range: args.range,
            severity,
            message: format!("{str}"),
            ..Default::default()
        }]
    }

    async fn execute_notification(
        &self,
        args: DBInteractExArgs<'i, 'g>,
        noti: impl Into<super::InteractLspNotification>,
        sender: &mut crate::server::buffer_operations::BufferOpChannelSender,
    ) -> MainResult<()> {
        Ok(())
    }

    async fn execute_request(
        &self,
        args: DBInteractExArgs<'i, 'g>,
        rq_id: RequestId,
        params: impl Into<super::InteractLspRequest>,
        sender: &mut crate::server::buffer_operations::BufferOpChannelSender,
    ) -> MainResult<()> {
        match Into::<InteractLspRequest>::into(params) {
            InteractLspRequest::Hover(_hover) => {
                let content = match args.typ {
                    DBInteractTyp::Status => {
                        match args.state_guard.and_then(|w| w.database.as_ref()) {
                            None => "No Database Connected".to_string(),
                            Some(db) => {
                                let config = db.config();
                                format!(
                                    r#"
----- Database Status -----
namespace: {}
database: {}
user: {}
pass: {}
port: {}
---------------------------"#,
                                    config.namespace,
                                    config.database,
                                    config.user,
                                    config.pass,
                                    config.port,
                                )
                            }
                        }
                    }
                };

                let contents = HoverContents::Scalar(lsp_types::MarkedString::String(content));
                sender
                    .send_operation(BufferOperation::HoverResponse {
                        id: rq_id,
                        contents,
                    })
                    .await?;
            }

            InteractLspRequest::GotoDef(_goto) => {
                warn!("activating gotodef for database interact");
                match args.typ {
                    DBInteractTyp::Status => {
                        let message = ShowMessageParams {
                            typ: MessageType::INFO,
                            message: format!("Status command has no GOTO function"),
                        };

                        sender.send_operation(message.into()).await?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    #[tracing::instrument("get ex args", skip(w))]
    fn get_execution_args(
        &self,
        w: &'i mut RwLockWriteGuard<'g, LspState<'static>>,
        interact_comment: &'i ParsedComment<'_>,
        doc_info: InteractDocumentInfo<'i>,
        args: &Vec<InteractArg>,
    ) -> Option<DBInteractExArgs<'i, 'g>> {
        warn!("args: {args:?}");

        let state_guard = if w.database.as_ref().is_some() {
            Some(w)
        } else {
            None
        };

        let command_char = args[0]
            .as_char()
            .and_then(|c| Some(c.to_ascii_lowercase()))
            .or_else(|| {
                tracing::error!(
                    "expected to get a char as first argument, instead got: {:#?}",
                    args[0]
                );
                None
            })?;

        let typ = match command_char {
            's' | _ => {
                if command_char != 's' {
                    warn!("unrecognized command char: {command_char}. Defaulting to 'status' behavior");
                }
                DBInteractTyp::Status
            }
        };

        Some(DBInteractExArgs {
            state_guard,
            typ,
            range: interact_comment.range,
        })
    }
}
