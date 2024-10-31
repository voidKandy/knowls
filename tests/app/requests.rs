use crate::helpers::{handler_tests_state, test_buff_op_channel, TEST_TRACING};
use espx_lsp_server::handle::{
    buffer_operations::{BufferOpChannelHandler, BufferOpChannelStatus, BufferOperation},
    requests::handle_goto_definition,
};
use lsp_server::RequestId;
use lsp_types::{
    GotoDefinitionParams, HoverParams, PartialResultParams, Position, TextDocumentIdentifier,
    TextDocumentPositionParams, Uri, WorkDoneProgressParams,
};
use serde::Serialize;
use std::{str::FromStr, sync::LazyLock};
use tracing_log::log::warn;

#[derive(Debug)]
enum BufferOpType {
    Diagnostics,
    WorkDone,
    ShowMessage,
    WorkspaceEdit,
    GotoFile,
    HoverResponse,
}

#[derive(Debug)]
struct BufferOpTypeVec(Vec<(BufferOpType, Option<usize>)>);

impl BufferOpType {
    fn same_as(&self, op: &BufferOperation) -> bool {
        match op {
            BufferOperation::WorkDone(_) => {
                if let Self::WorkDone = self {
                    return true;
                }
            }
            BufferOperation::WorkspaceEdit(_) => {
                if let Self::WorkspaceEdit = self {
                    return true;
                }
            }
            BufferOperation::Diagnostics(_) => {
                if let Self::Diagnostics = self {
                    return true;
                }
            }
            BufferOperation::ShowMessage(_) => {
                if let Self::ShowMessage = self {
                    return true;
                }
            }
            BufferOperation::GotoFile { .. } => {
                if let Self::GotoFile = self {
                    return true;
                }
            }
            BufferOperation::HoverResponse { .. } => {
                if let Self::HoverResponse = self {
                    return true;
                }
            }
        }
        false
    }
}

impl BufferOpTypeVec {
    fn validate_buffer_ops(&self, vec: Vec<BufferOperation>) -> bool {
        let mut total_gone_through = 0;
        for (typ, amt_opt) in &self.0 {
            match amt_opt {
                Some(amt) => {
                    let chunk = vec.iter().skip(total_gone_through).take(*amt);
                    total_gone_through += amt;
                    for op in chunk {
                        if !typ.same_as(op) {
                            return false;
                        }
                    }
                }
                None => {
                    let chunk = vec
                        .iter()
                        .skip(total_gone_through)
                        .take_while(|op| typ.same_as(op));

                    total_gone_through += chunk.count();
                }
            }
        }
        true
    }
}

pub fn into_lsp_request<P: Serialize>(
    params: P,
    id: impl Into<RequestId>,
    method: &str,
) -> lsp_server::Request {
    let params = serde_json::to_value(params).expect("could not serialize");
    lsp_server::Request {
        id: id.into(),
        method: method.to_string(),
        params,
    }
}

#[tracing::instrument(name = "buffer op drain", skip_all)]
async fn poll_into_vec(handler: &mut BufferOpChannelHandler) -> Vec<BufferOperation> {
    let mut all = vec![];
    while let Some(status) = handler.receiver.recv().await {
        match status.unwrap() {
            BufferOpChannelStatus::Finished => break,
            BufferOpChannelStatus::Working(buffer_op) => {
                all.push(buffer_op);
            }
        }
    }
    all
}

pub fn create_gotodef_params(position: Position, doc_uri: Uri) -> GotoDefinitionParams {
    let partial_result_params = PartialResultParams {
        partial_result_token: None,
    };

    let work_done_progress_params = WorkDoneProgressParams {
        work_done_token: None,
    };

    let text_document_position_params = TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: doc_uri },
        position,
    };

    GotoDefinitionParams {
        text_document_position_params,
        work_done_progress_params,
        partial_result_params,
    }
}

fn create_hover_params(position: Position, doc_uri: Uri) -> HoverParams {
    let work_done_progress_params = WorkDoneProgressParams {
        work_done_token: None,
    };

    let text_document_position_params = TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: doc_uri },
        position,
    };

    HoverParams {
        text_document_position_params,
        work_done_progress_params,
    }
}

#[ignore]
#[tokio::test]
async fn handles_prompt_gotodef_correctly() {
    LazyLock::force(&TEST_TRACING);
    let state = handler_tests_state().await;

    let mut buffer_op_channel = test_buff_op_channel();
    let test_doc_1_uri = Uri::from_str("test_doc_1.rs").unwrap();

    let prompt_pos = Position {
        line: 3,
        character: 4,
    };

    let prompt_request_params = create_gotodef_params(prompt_pos, test_doc_1_uri.clone());

    let req = into_lsp_request(prompt_request_params, 1, "textDocument/definition");
    warn!("prompt req: {req:?}");
    handle_goto_definition(req, state.clone(), buffer_op_channel.sender.clone())
        .await
        .expect("failed handle goto def for prompt");
    buffer_op_channel.sender.send_finish().await.unwrap();

    let expected_ops = BufferOpTypeVec(vec![
        (BufferOpType::ShowMessage, Some(2)),
        (BufferOpType::WorkspaceEdit, Some(1)),
        (BufferOpType::WorkDone, None),
        (BufferOpType::ShowMessage, Some(1)),
    ]);
    let all = poll_into_vec(&mut buffer_op_channel).await;
    warn!("All for prompt:\n{all:#?}");

    assert!(expected_ops.validate_buffer_ops(all));
}
