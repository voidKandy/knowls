use super::{
    messages::InteractLspNotification, messages::InteractLspRequest, Interact, InteractCtx,
    InteractParams, InteractReadCtx, InteractWriteCtx, ServerStateReadGuard, ServerStateWriteGuard,
};
use crate::{
    database::Database,
    knowledge::parsing::tokens::Token,
    other_err,
    rpc::lsp::buffer_operations::{BufferOpChannelSender, BufferOperation},
    MainErr, MainResult,
};
use lsp_server::RequestId;
use lsp_types::{DiagnosticSeverity, HoverContents, Range};

#[derive(Debug, Clone)]
pub enum DBInteractVar {
    Status,
}

#[derive(Debug)]
pub struct DBReadCtx<'i> {
    db: Option<&'i Database>,
}
impl<'i> InteractReadCtx<'i> for DBReadCtx<'i> {}

#[derive(Debug)]
pub struct DBWriteCtx<'i> {
    db: Option<&'i mut Database>,
}
impl<'i> InteractWriteCtx<'i> for DBWriteCtx<'i> {}

#[derive(Debug, Clone)]
pub struct DBInteract {
    range: Range,
    var: DBInteractVar,
}

impl DBInteract {
    const TRIGGER: char = '%';
}

impl<'t> TryFrom<InteractParams<'t>> for DBInteract {
    type Error = MainErr;
    fn try_from((toks, tok, idx): InteractParams<'t>) -> Result<Self, Self::Error> {
        if let Token::Comment(parsed) = tok {
            let mut chars = parsed.content.chars();

            match chars.next().ok_or(other_err!("empty content"))? {
                Self::TRIGGER => {
                    return Ok(Self {
                        range: parsed.range,
                        var: DBInteractVar::Status,
                    });
                }
                c => return Err(other_err!("{c} is not valid for an database interact")),
            }
        } else {
            return Err(other_err!("Wrong token Variant"));
        }
    }
}

impl<'i> Interact<'i> for DBInteract {
    type ReadContext = DBReadCtx<'i>;
    type WriteContext = DBWriteCtx<'i>;

    async fn handle_noti(
        &self,
        noti: impl Into<InteractLspNotification>,
        _ctx: &InteractCtx<'i, Self>,
        _sender: &mut BufferOpChannelSender,
    ) -> MainResult<()> {
        match Into::<InteractLspNotification>::into(noti) {
            InteractLspNotification::Save(_save) => {}
            InteractLspNotification::Change(_change) => {}
            InteractLspNotification::Open(_open) => {}
        }
        Ok(())
    }

    async fn handle_req(
        &self,
        req: impl Into<InteractLspRequest>,
        rq_id: RequestId,
        ctx: &InteractCtx<'i, Self>,
        sender: &mut BufferOpChannelSender,
    ) -> MainResult<()> {
        match Into::<InteractLspRequest>::into(req) {
            InteractLspRequest::Hover(_hover) => {
                let content = match self.var {
                    DBInteractVar::Status => {
                        match match ctx {
                            InteractCtx::Read(r) => {
                                r.db.as_ref().and_then(|db| Some(db.config().clone()))
                            }
                            InteractCtx::Write(w) => {
                                w.db.as_ref().and_then(|db| Some(db.config().clone()))
                            }
                        } {
                            None => "No Database Connected".to_string(),
                            Some(db_config) => {
                                format!(
                                    r#"
                    ----- Database Status -----
                    namespace: {}
                    database: {}
                    user: {}
                    pass: {}
                    port: {}
                    ---------------------------"#,
                                    db_config.namespace,
                                    db_config.database,
                                    db_config.user,
                                    db_config.pass,
                                    db_config.port,
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
            InteractLspRequest::GotoDef(_goto) => {}
            InteractLspRequest::Diagnostic(_diag) => {}
        };
        Ok(())
    }

    fn diagnostics(&self, _wctx: Self::ReadContext) -> Vec<lsp_types::Diagnostic> {
        let mut all_diagnostics = vec![];
        let severity = Some(DiagnosticSeverity::HINT);
        let message = match self.var {
            DBInteractVar::Status => "Database Status",
        }
        .to_string();
        all_diagnostics.push(lsp_types::Diagnostic {
            range: self.range,
            severity,
            message,
            ..Default::default()
        });

        all_diagnostics
    }

    fn get_read_context(&self, r: &'i ServerStateReadGuard) -> MainResult<Self::ReadContext> {
        Ok(DBReadCtx { db: r.db.as_ref() })
    }

    fn get_write_context(
        &self,
        w: &'i mut ServerStateWriteGuard,
    ) -> MainResult<Self::WriteContext> {
        Ok(DBWriteCtx { db: w.db.as_mut() })
    }
}
