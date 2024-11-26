use super::{
    execution::InteractDocumentInfo,
    logic::{InteractArg, InteractVar, LspMessageInteract},
    parsing::{comments::ParsedComment, tokens::TokenVec},
    InteractLspRequest,
};
use crate::{
    config::database::DatabaseConfig,
    database::Database,
    handle::{buffer_operations::BufferOperation, error::HandleResult},
    state::{LspState, SharedState},
};
use color_eyre::owo_colors::OwoColorize;
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
    Init,
    Kill,
    Status,
}

// impl<'i> From<&'i Database> for DBInfo<'i> {
//     fn from(value: &'i Database) -> Self {
//         Self {
//             config: &value.config,
//             running: value.thread.is_some(),
//         }
//     }
// }

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DBInteract;

impl DBInteract {
    const DATABASE: char = '%';
}

impl TryFrom<char> for DBInteract {
    type Error = anyhow::Error;
    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            Self::DATABASE => Ok(Self),
            _ => Err(anyhow::anyhow!(
                "could not create agent interact from {value}"
            )),
        }
    }
}

impl<'i, 'g> LspMessageInteract<'i, 'g, DBInteractExArgs<'i, 'g>> for DBInteract {
    fn diagnostics(&self, args: DBInteractExArgs<'i, 'g>) -> Vec<Diagnostic> {
        let severity = Some(DiagnosticSeverity::HINT);
        let str = match args.typ {
            DBInteractTyp::Init => "Init",
            DBInteractTyp::Kill => "Kill",
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
        sender: &mut crate::handle::buffer_operations::BufferOpChannelSender,
    ) -> HandleResult<()> {
        Ok(())
    }

    async fn execute_request(
        &self,
        args: DBInteractExArgs<'i, 'g>,
        rq_id: RequestId,
        params: impl Into<super::InteractLspRequest>,
        sender: &mut crate::handle::buffer_operations::BufferOpChannelSender,
    ) -> HandleResult<()> {
        match Into::<InteractLspRequest>::into(params) {
            InteractLspRequest::Hover(_hover) => {
                let content =
                    match args.typ {
                        DBInteractTyp::Kill => if args.state_guard.as_ref().is_some_and(|w| {
                            w.database.as_ref().is_some_and(|db| db.thread.is_some())
                        }) {
                            "Goto Def to Kill database"
                        } else {
                            "Database is not running"
                        }
                        .to_string(),

                        DBInteractTyp::Init => if args.state_guard.as_ref().is_some_and(|w| {
                            w.database.as_ref().is_some_and(|db| db.thread.is_some())
                        }) {
                            "Database is already running"
                        } else {
                            "Goto Def to Init database"
                        }
                        .to_string(),

                        DBInteractTyp::Status => {
                            match args.state_guard.and_then(|w| w.database.as_ref()) {
                                None => "No Database Connected".to_string(),
                                Some(db) => {
                                    format!(
                                        r#"
----- Database Status -----
Database {}
namespace: {}
database: {}
user: {}
pass: {}
port: {}
---------------------------"#,
                                        if db.thread.is_some() {
                                            "Is Running"
                                        } else {
                                            "Is Not Running"
                                        },
                                        db.config.namespace,
                                        db.config.database,
                                        db.config.user,
                                        db.config.pass,
                                        db.config.port,
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
            InteractLspRequest::GotoDef(_goto) => match args.typ {
                t @ DBInteractTyp::Init | t @ DBInteractTyp::Kill => match args.state_guard {
                    None => {
                        let message = ShowMessageParams {
                            typ: MessageType::WARNING,
                            message: format!(
                                "This command does nothing when there isn't a DB present"
                            ),
                        };

                        sender.send_operation(message.into()).await?;
                    }
                    Some(w) => {
                        if let Some(db) = w.database.as_mut() {
                            match t {
                                DBInteractTyp::Init => {
                                    if db.thread.is_none() {
                                        let message = match db.init_thread().await {
                                            Ok(_) => "Successfully initialized Database thread!"
                                                .to_string(),
                                            Err(e) => {
                                                format!(
                                                    "Failed to initialize database thread: {e:#?}"
                                                )
                                            }
                                        };
                                        let message = ShowMessageParams {
                                            typ: MessageType::INFO,
                                            message,
                                        };

                                        sender.send_operation(message.into()).await?;
                                    }
                                }
                                DBInteractTyp::Kill => {
                                    if let Some(th) = db.thread.take() {
                                        let message = match th.kill().await {
                                            Ok(_) => {
                                                "Successfully killed Database thread!".to_string()
                                            }
                                            Err(e) => {
                                                format!("Failed to kill database thread: {e:#?}")
                                            }
                                        };
                                        let message = ShowMessageParams {
                                            typ: MessageType::INFO,
                                            message,
                                        };

                                        sender.send_operation(message.into()).await?;
                                    }
                                }
                                _ => unreachable!(),
                            }
                        }
                    }
                },

                DBInteractTyp::Status => {
                    let message = ShowMessageParams {
                        typ: MessageType::INFO,
                        message: format!("Status command has no GOTO function"),
                    };

                    sender.send_operation(message.into()).await?;
                }
            },
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
            'i' => DBInteractTyp::Init,
            'k' => DBInteractTyp::Kill,
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
