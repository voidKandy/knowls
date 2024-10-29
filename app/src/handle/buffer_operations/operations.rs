use std::sync::Arc;

use super::BufferOpChannelResult;
use crate::handle::diagnostics::LspDiagnostic;
use crossbeam_channel::Sender;
use lsp_server::{Message, Notification, RequestId, Response};
use lsp_types::{
    ApplyWorkspaceEditParams, GotoDefinitionResponse, HoverContents, ProgressParams,
    ProgressParamsValue, ProgressToken, PublishDiagnosticsParams, ShowMessageParams,
    WorkDoneProgress,
};
use tokio::{io::AsyncWriteExt, net::UnixStream, sync::RwLock};
use tracing::{debug, error};

#[derive(Debug, Clone)]
pub enum BufferOperation {
    Diagnostics(LspDiagnostic),
    WorkDone(WorkDoneProgress),
    ShowMessage(ShowMessageParams),
    WorkspaceEdit(ApplyWorkspaceEditParams),
    GotoFile {
        id: RequestId,
        response: GotoDefinitionResponse,
    },
    HoverResponse {
        id: RequestId,
        contents: HoverContents,
    },
}

impl From<WorkDoneProgress> for BufferOperation {
    fn from(value: WorkDoneProgress) -> Self {
        Self::WorkDone(value)
    }
}

impl From<LspDiagnostic> for BufferOperation {
    fn from(value: LspDiagnostic) -> Self {
        Self::Diagnostics(value)
    }
}

impl From<ShowMessageParams> for BufferOperation {
    fn from(value: ShowMessageParams) -> Self {
        Self::ShowMessage(value)
    }
}

impl From<ApplyWorkspaceEditParams> for BufferOperation {
    fn from(value: ApplyWorkspaceEditParams) -> Self {
        Self::WorkspaceEdit(value)
    }
}

impl BufferOperation {
    pub async fn do_operation(self, stream: Arc<RwLock<UnixStream>>) -> BufferOpChannelResult<()> {
        let msg = match self {
            BufferOperation::WorkDone(work) => {
                let method = match work {
                    WorkDoneProgress::Begin(_) => "window/workDoneProgress/create",
                    WorkDoneProgress::Report(_) | WorkDoneProgress::End(_) => "$/progress",
                };

                Message::Notification(Notification {
                    method: method.to_string(),
                    params: serde_json::to_value(ProgressParams {
                        token: ProgressToken::Number(0),
                        value: ProgressParamsValue::WorkDone(work),
                    })?,
                })
            }
            BufferOperation::WorkspaceEdit(edit) => Message::Notification(Notification {
                method: "workspace/applyEdit".to_string(),
                params: serde_json::to_value(edit)?,
            }),
            BufferOperation::ShowMessage(message_params) => Message::Notification(Notification {
                method: "window/showMessage".to_string(),
                params: serde_json::to_value(message_params)?,
            }),

            BufferOperation::GotoFile { id, response } => {
                let result = serde_json::to_value(response).ok();
                debug!("SENDING GOTO FILE RESPONSE");

                Message::Response(Response {
                    id,
                    result,
                    error: None,
                })
            }

            BufferOperation::HoverResponse { contents, id } => {
                let result = match serde_json::to_value(&lsp_types::Hover {
                    contents,
                    range: None,
                }) {
                    Ok(jsn) => Some(jsn),
                    Err(err) => {
                        error!("Fail to parse hover_response: {:?}", err);
                        None
                    }
                };
                debug!("SENDING HOVER RESPONSE. ID: {:?}", id);
                Message::Response(Response {
                    id,
                    result,
                    error: None,
                })
            }

            BufferOperation::Diagnostics(diag) => match diag {
                LspDiagnostic::Publish(diag_params) => {
                    debug!("PUBLISHING DIAGNOSTICS: {:?}", diag_params);
                    if let Some(params) = serde_json::to_value(diag_params).ok() {
                        Message::Notification(Notification {
                            method: "textDocument/publishDiagnostics".to_string(),
                            params,
                        })
                    } else {
                        return Ok(());
                    }
                }

                LspDiagnostic::ClearDiagnostics(uri) => {
                    debug!("CLEARING DIAGNOSTICS");
                    let diag_params = PublishDiagnosticsParams {
                        uri,
                        diagnostics: vec![],
                        version: None,
                    };
                    if let Some(params) = serde_json::to_value(diag_params).ok() {
                        Message::Notification(Notification {
                            method: "textDocument/publishDiagnostics".to_string(),
                            params,
                        })
                    } else {
                        return Ok(());
                    }
                }
            },
        };

        let json: serde_json::Value = serde_json::to_value(msg).unwrap();
        let mut w = stream.write().await;
        if let Some(str) = serde_json::to_string(&json).ok() {
            let str = &format!("{str}\n");
            let bytes: &[u8] = str.as_bytes();

            w.write_all(bytes).await.unwrap();
            w.flush().await.unwrap();
        }
        Ok(())

        // return Ok(sender);
    }
}
